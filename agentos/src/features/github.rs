//! GitHub Issues integration belongs to Runner.
use crate::infra::github::{GitHub, Issue};
use anyhow::{Result, ensure};
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

#[derive(Subcommand)]
pub enum GitHubCommand {
    #[command(subcommand)]
    Actions(super::github_actions::ActionsCommand),
    #[command(subcommand)]
    Issue(IssueCommand),
}
#[derive(Subcommand)]
pub enum IssueCommand {
    Get {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        number: u64,
    },
    Import {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        number: u64,
        /// Explicitly replace an existing note with changed remote content.
        #[arg(long)]
        overwrite: bool,
    },
    Create {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        request_file: String,
    },
    Update {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        number: u64,
        #[arg(long)]
        request_file: String,
    },
}

pub fn execute(
    db: &Database,
    workspace: &str,
    command: &GitHubCommand,
) -> Result<serde_json::Value> {
    let api = GitHub::authenticated()?;
    match command {
        GitHubCommand::Actions(_) => anyhow::bail!("Actions requires a Runner journal"),
        GitHubCommand::Issue(command) => match command {
            IssueCommand::Get { repo, number } => {
                Ok(serde_json::to_value(api.issue(repo, *number)?)?)
            }
            IssueCommand::Import {
                repo,
                number,
                overwrite,
            } => {
                let issue = api.issue(repo, *number)?;
                let id = import(db, workspace, &issue, *overwrite)?;
                Ok(serde_json::json!({"noteId":id,"issueUrl":issue.html_url}))
            }
            IssueCommand::Create { repo, request_file }
            | IssueCommand::Update {
                repo, request_file, ..
            } => {
                let text = if request_file == "-" {
                    std::io::read_to_string(std::io::stdin())?
                } else {
                    std::fs::read_to_string(request_file)?
                };
                let number = match command {
                    IssueCommand::Update { number, .. } => Some(*number),
                    _ => None,
                };
                Ok(serde_json::to_value(api.write_issue(
                    repo,
                    number,
                    serde_json::from_str(&text)?,
                )?)?)
            }
        },
    }
}

pub fn import(db: &Database, workspace: &str, issue: &Issue, overwrite: bool) -> Result<String> {
    ensure!(issue.pull_request.is_none(), "cannot import pull request");
    let id = format!(
        "github-{}",
        Sha256Hasher.sha256_hex(&serde_json::to_string(&(workspace, issue.id))?)
    );
    let previous = DocumentQuery::get(db, workspace, &id)?;
    let body = issue.body.as_deref().unwrap_or("");
    if let Some(previous) = &previous {
        if previous.title == issue.title && previous.body_text == body {
            return Ok(id);
        }
        ensure!(
            overwrite,
            "linked note differs; review both versions and use --overwrite to replace it"
        );
    }
    let now = SystemClock.now_rfc3339();
    SaveDocument::new(db, &UuidGenerator, &Sha256Hasher).execute(SaveDocumentInput {
        workspace_name: workspace.into(),
        mode: if previous.is_some() {
            WriteMode::Update
        } else {
            WriteMode::Insert
        },
        document: DocumentAsset {
            id: id.clone(),
            workspace_id: workspace.into(),
            title: issue.title.clone(),
            body_text: body.into(),
            blob_uri: None,
            document_type: DOCUMENT_TYPE_MEMO.into(),
            created_at: previous
                .as_ref()
                .map(|v| v.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now.clone(),
        },
        lineage: LineageInput {
            workspace_id: workspace.into(),
            source_kind: "github_issue".into(),
            source_id: format!("{}#updated_at={}", issue.html_url, issue.updated_at),
            target_kind: "document".into(),
            target_id: id.clone(),
            relation_type: relation::DERIVED_FROM.into(),
            actor: "automation:github-issues".into(),
            created_at: now,
        },
        metas: previous.map(|v| v.metas).unwrap_or_default(),
        metadata: vec![],
        attachments: vec![],
    })?;
    Ok(id)
}
#[cfg(test)]
mod tests {
    use super::*;
    use lineage_store::ports::LineageQuery;
    #[test]
    fn imports_are_idempotent_and_changes_require_explicit_overwrite() {
        let db = Database::open_in_memory().unwrap();
        let mut issue = Issue {
            id: 42,
            number: 1,
            title: "Issue".into(),
            body: Some("first".into()),
            html_url: "https://github.com/o/r/issues/1".into(),
            updated_at: "today".into(),
            pull_request: None,
        };
        let id = import(&db, "local", &issue, false).unwrap();
        assert_eq!(id, import(&db, "local", &issue, false).unwrap());
        assert_eq!(LineageQuery::list(&db, "local").unwrap().len(), 1);
        issue.body = Some("second".into());
        assert!(import(&db, "local", &issue, false).is_err());
        assert_eq!(
            DocumentQuery::get(&db, "local", &id)
                .unwrap()
                .unwrap()
                .body_text,
            "first"
        );
        import(&db, "local", &issue, true).unwrap();
        let links = LineageQuery::list(&db, "local").unwrap();
        assert_eq!(links.len(), 2);
        assert!(
            lineage_core::domain::lineage::LineageLedger::new(&Sha256Hasher)
                .verify(&links)
                .is_ok()
        );
        assert_ne!(id, import(&db, "other", &issue, false).unwrap());
    }
}
