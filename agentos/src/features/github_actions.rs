//! Durable Actions dispatch and reconciliation, entirely owned by Runner.
use crate::infra::{
    github::{GitHub, validate_repo},
    github_runs::{Dispatch, RunJournal, TrackedRun},
};
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use lineage_core::domain::{
    document::{DOCUMENT_TYPE_MEMO, DocumentAsset},
    lineage::{LineageInput, relation},
    shared::Hasher,
};
use lineage_store::{
    features::document::{SaveDocument, SaveDocumentInput, WriteMode},
    infra::{
        clock::{SystemClock, UuidGenerator},
        crypto::Sha256Hasher,
        sqlite::Database,
    },
    ports::{Clock, DocumentQuery},
};
use reqwest::Method;
use serde_json::{Value, json};

#[derive(Subcommand)]
pub enum ActionsCommand {
    /// 完了した実行に外部結果を紐付ける。同じ内容は重複登録しない。
    Record {
        #[arg(long)]
        execution_id: String,
        #[arg(long)]
        result_file: String,
    },
    /// 終了まで状態を確認する。タイムアウト時も次回再開できる。
    Watch {
        #[arg(long)]
        execution_id: String,
        #[arg(long, default_value_t = 600)]
        timeout_seconds: u64,
    },
    /// 起動要求JSONを読み、executionIdで重複実行を防ぐ。
    Dispatch {
        #[arg(long)]
        request_file: String,
    },
    /// 状態を再取得する。終端なら結果Noteを一度だけ登録する。
    Status {
        #[arg(long)]
        execution_id: String,
    },
}
fn read(path: &str) -> Result<String> {
    Ok(if path == "-" {
        std::io::read_to_string(std::io::stdin())?
    } else {
        std::fs::read_to_string(path)?
    })
}

pub fn execute(
    db: &Database,
    journal: &RunJournal,
    workspace: &str,
    command: &ActionsCommand,
) -> Result<Value> {
    if let ActionsCommand::Watch {
        execution_id,
        timeout_seconds,
    } = command
    {
        ensure!(
            *timeout_seconds > 0 && *timeout_seconds <= 86400,
            "timeout_seconds must be 1..86400"
        );
        let start = std::time::Instant::now();
        loop {
            let result = execute(
                db,
                journal,
                workspace,
                &ActionsCommand::Status {
                    execution_id: execution_id.clone(),
                },
            )?;
            if result["status"] == "completed" {
                return Ok(result);
            }
            ensure!(
                start.elapsed().as_secs() < *timeout_seconds,
                "watch timed out; resume with status/watch"
            );
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    }
    if let ActionsCommand::Record {
        execution_id,
        result_file,
    } = command
    {
        let tracked = journal.get(execution_id)?.context("execution not found")?;
        ensure!(
            tracked.request.workspace == workspace && tracked.status == "completed",
            "poll completed execution in this workspace before recording output"
        );
        let content = read(result_file)?;
        let id = record_result(db, &tracked, Some(&content))?;
        return Ok(json!({"resultNote":id,"executionId":execution_id}));
    }
    let api = GitHub::authenticated()?;
    execute_api(db, journal, workspace, command, &api)
}
fn execute_api(
    db: &Database,
    journal: &RunJournal,
    workspace: &str,
    command: &ActionsCommand,
    api: &GitHub,
) -> Result<Value> {
    match command {
        ActionsCommand::Watch { .. } | ActionsCommand::Record { .. } => unreachable!(),
        ActionsCommand::Dispatch { request_file } => {
            let request: Dispatch = serde_json::from_str(&read(request_file)?)?;
            ensure!(
                request.workspace == workspace,
                "request workspace must match --workspace"
            );
            validate_repo(&request.repo)?;
            ensure!(
                !request.execution_id.trim().is_empty(),
                "execution_id is required"
            );
            ensure!(!request.git_ref.trim().is_empty(), "git_ref is required");
            ensure!(
                !request.workflow.is_empty()
                    && request
                        .workflow
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
                "workflow must be filename or ID"
            );
            ensure!(request.inputs.is_object(), "inputs must be an object");
            ensure!(
                DocumentQuery::get(db, workspace, &request.note)?.is_some(),
                "source note not found"
            );
            let mut tracked = TrackedRun {
                source_fingerprint: Sha256Hasher.sha256_hex(&serde_json::to_string(
                    &DocumentQuery::get(db, workspace, &request.note)?
                        .context("source note not found")?,
                )?),
                request,
                run_id: None,
                url: None,
                status: "dispatching".into(),
                conclusion: None,
                result_note: None,
            };
            if !journal.reserve(&tracked)? {
                let previous = journal
                    .get(&tracked.request.execution_id)?
                    .context("execution disappeared")?;
                ensure!(
                    serde_json::to_value(&previous.request)?
                        == serde_json::to_value(&tracked.request)?,
                    "execution_id already used for a different request"
                );
                ensure!(
                    previous.run_id.is_some(),
                    "dispatch outcome unknown: inspect GitHub; this execution will not be dispatched again"
                );
                return Ok(serde_json::to_value(previous)?);
            }
            let response=api.request::<Value>(Method::POST,&format!("/repos/{}/actions/workflows/{}/dispatches",tracked.request.repo,tracked.request.workflow),Some(&json!({"ref":tracked.request.git_ref,"inputs":tracked.request.inputs,"return_run_details":true})));
            let details = match response {
                Ok(v) => v,
                Err(error) => {
                    tracked.status = "unknown".into();
                    journal.update(&tracked)?;
                    return Err(error);
                }
            };
            // Never infer a run from 'latest': dispatch returns the exact run ID.
            let Some(run_id) = details.get("workflow_run_id").and_then(Value::as_u64) else {
                tracked.status = "unknown".into();
                journal.update(&tracked)?;
                anyhow::bail!("dispatch returned no run ID; inspect GitHub before proceeding");
            };
            tracked.run_id = Some(run_id);
            tracked.url = details
                .get("html_url")
                .and_then(Value::as_str)
                .map(str::to_owned);
            tracked.status = "queued".into();
            journal.update(&tracked)?;
            Ok(serde_json::to_value(tracked)?)
        }
        ActionsCommand::Status { execution_id } => {
            let mut tracked = journal.get(execution_id)?.context("execution not found")?;
            ensure!(
                tracked.request.workspace == workspace,
                "execution belongs to another workspace"
            );
            let id = tracked
                .run_id
                .context("dispatch outcome unknown; no run ID recorded")?;
            let run: Value = api.request(
                Method::GET,
                &format!("/repos/{}/actions/runs/{id}", tracked.request.repo),
                None,
            )?;
            ensure!(
                run.get("id").and_then(Value::as_u64) == Some(id),
                "GitHub run ID mismatch"
            );
            tracked.status = run
                .get("status")
                .and_then(Value::as_str)
                .context("missing run status")?
                .into();
            tracked.conclusion = run
                .get("conclusion")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if tracked.status == "completed" {
                ensure!(
                    tracked.conclusion.is_some(),
                    "completed run has no conclusion"
                );
                tracked.result_note = Some(record_result(db, &tracked, None)?);
            }
            journal.update(&tracked)?;
            Ok(serde_json::to_value(tracked)?)
        }
    }
}
fn record_result(db: &Database, run: &TrackedRun, output: Option<&str>) -> Result<String> {
    let request = &run.request;

    let id = format!(
        "github-action-{}",
        Sha256Hasher.sha256_hex(&serde_json::to_string(&(
            &request.workspace,
            &request.execution_id
        ))?)
    );
    let title = format!("GitHub Actions: {}", request.execution_id);
    let body = serde_json::to_string_pretty(
        &json!({"executionId":request.execution_id,"repository":request.repo,"workflow":request.workflow,"ref":request.git_ref,"runId":run.run_id,"url":run.url,"conclusion":run.conclusion,"sourceNote":request.note,"sourceFingerprint":run.source_fingerprint,"output":output}),
    )?;
    if let Some(previous) = DocumentQuery::get(db, &request.workspace, &id)? {
        ensure!(
            previous.body_text == body,
            "result differs from previously registered run; do not overwrite history"
        );
        return Ok(id);
    }
    let now = SystemClock.now_rfc3339();
    SaveDocument::new(db, &UuidGenerator, &Sha256Hasher).execute(SaveDocumentInput {
        workspace_name: request.workspace.clone(),
        mode: WriteMode::Insert,
        document: DocumentAsset {
            id: id.clone(),
            workspace_id: request.workspace.clone(),
            title,
            body_text: body,
            blob_uri: None,
            document_type: DOCUMENT_TYPE_MEMO.into(),
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        lineage: LineageInput {
            workspace_id: request.workspace.clone(),
            source_kind: "document".into(),
            source_id: request.note.clone(),
            target_kind: "document".into(),
            target_id: id.clone(),
            relation_type: relation::DERIVED_FROM.into(),
            actor: "automation:github-actions".into(),
            created_at: now,
        },
        metas: vec![],
        metadata: vec![],
        attachments: vec![],
    })?;
    Ok(id)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reservation_and_result_registration_are_idempotent() {
        let path = std::env::temp_dir().join(format!("lineage-actions-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = Database::open_in_memory().unwrap();
        let note = crate::features::notes::put(
            &db,
            "local",
            crate::features::notes::PutNote {
                id: None,
                title: "source".into(),
                body_text: "text".into(),
            },
        )
        .unwrap();
        let run = TrackedRun {
            request: Dispatch {
                execution_id: "execution".into(),
                workspace: "local".into(),
                note,
                repo: "o/r".into(),
                workflow: "work.yml".into(),
                git_ref: "main".into(),
                inputs: json!({}),
            },
            source_fingerprint: "source-hash".into(),
            run_id: Some(42),
            url: Some("https://github.com/o/r/actions/runs/42".into()),
            status: "completed".into(),
            conclusion: Some("success".into()),
            result_note: None,
        };
        {
            let journal = RunJournal::open(&path).unwrap();
            assert!(journal.reserve(&run).unwrap());
            assert!(!journal.reserve(&run).unwrap());
        }
        let journal = RunJournal::open(&path).unwrap();
        assert_eq!(journal.get("execution").unwrap().unwrap().run_id, Some(42));
        let first = record_result(&db, &run, None).unwrap();
        assert_eq!(first, record_result(&db, &run, None).unwrap());
        assert_eq!(DocumentQuery::recent(&db, "local", 50).unwrap().len(), 2);
        drop(journal);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod api_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    #[test]
    fn dispatch_tracks_exact_run_and_replay_does_not_send_another_request() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let api = GitHub::new(
            format!("http://{}", listener.local_addr().unwrap()),
            "test".into(),
        )
        .unwrap();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            loop {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..i]).to_lowercase();
                    let size = header
                        .lines()
                        .find_map(|s| s.strip_prefix("content-length: "))
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    if bytes.len() >= i + 4 + size {
                        break;
                    }
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.starts_with("POST /repos/o/r/actions/workflows/work.yml/dispatches "));
            let body: Value =
                serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            assert_eq!(body["return_run_details"], true);
            let body =
                r#"{"workflow_run_id":123,"html_url":"https://github.com/o/r/actions/runs/123"}"#;
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let directory =
            std::env::temp_dir().join(format!("lineage-dispatch-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let journal = RunJournal::open(&directory.join("journal.db")).unwrap();
        let db = Database::open_in_memory().unwrap();
        let note = crate::features::notes::put(
            &db,
            "local",
            crate::features::notes::PutNote {
                id: None,
                title: "source".into(),
                body_text: "text".into(),
            },
        )
        .unwrap();
        let path = directory.join("request.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&Dispatch {
                execution_id: "one".into(),
                workspace: "local".into(),
                note,
                repo: "o/r".into(),
                workflow: "work.yml".into(),
                git_ref: "main".into(),
                inputs: json!({}),
            })
            .unwrap(),
        )
        .unwrap();
        let command = ActionsCommand::Dispatch {
            request_file: path.to_str().unwrap().into(),
        };
        assert_eq!(
            execute_api(&db, &journal, "local", &command, &api).unwrap()["run_id"],
            123
        );
        server.join().unwrap();
        // Listener is closed: replay can only succeed without a network request.
        assert_eq!(
            execute_api(&db, &journal, "local", &command, &api).unwrap()["run_id"],
            123
        );
        drop(journal);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
