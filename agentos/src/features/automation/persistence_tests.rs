use super::commit_result::CommitAutomationResult;
use lineage_core::domain::automation::{AutomationRun, BackendKind, RunStatus};
use lineage_core::domain::document::DocumentAsset;
use lineage_core::domain::lineage::{LineageInput, LineageLedger, VerifyResult};
use lineage_core::domain::meta::{DocumentMetadata, MetaAssignment};
use lineage_store::features::document::*;
use lineage_store::infra::clock::SequentialIds;
use lineage_store::infra::crypto::Sha256Hasher;
use lineage_store::infra::sqlite::Database;
use lineage_store::ports::{AutomationRunStore, DocumentQuery, LineageQuery};

const NOW: &str = "2026-09-13T00:00:00Z";

fn input(id: &str) -> SaveDocumentInput {
    SaveDocumentInput {
        workspace_name: "Workspace".into(),
        mode: WriteMode::Insert,
        document: DocumentAsset {
            id: id.into(),
            workspace_id: "ws".into(),
            title: "Caller supplied title".into(),
            body_text: "A different first line\n".into(),
            blob_uri: None,
            document_type: "memo".into(),
            created_at: NOW.into(),
            updated_at: NOW.into(),
        },
        lineage: LineageInput {
            workspace_id: "ws".into(),
            source_kind: "import".into(),
            source_id: "source".into(),
            target_kind: "document".into(),
            target_id: id.into(),
            relation_type: "derived_from".into(),
            actor: "caller".into(),
            created_at: NOW.into(),
        },
        metas: vec![MetaAssignment::user("topic", None)],
        metadata: vec![DocumentMetadata {
            key: "origin".into(),
            value: "file".into(),
            source: "auto".into(),
        }],
        attachments: vec![],
    }
}

fn attachment() -> LinkedDocument {
    let mut image = input("image");
    image.document.document_type = "image".into();
    image.document.blob_uri = Some("/attachments/image.png".into());
    image.lineage.source_kind = "document".into();
    image.lineage.source_id = "image".into();
    image.lineage.target_id = "note".into();
    image.lineage.relation_type = "attachment_for".into();
    LinkedDocument {
        document: image.document,
        lineage: image.lineage,
    }
}

#[test]
fn saves_the_canonical_input_without_applying_capture_rules() {
    let db = Database::open_in_memory().unwrap();
    let ids = SequentialIds::new();
    let mut request = input("note");
    request.attachments.push(attachment());
    let saved = SaveDocument::new(&db, &ids, &Sha256Hasher)
        .execute(request)
        .unwrap();
    let document = DocumentQuery::get(&db, "ws", "note").unwrap().unwrap();
    assert_eq!(document.title, "Caller supplied title");
    assert_eq!(document.body_text, "A different first line\n");
    assert_eq!(document.metas, vec![MetaAssignment::user("topic", None)]);
    let links = LineageQuery::list(&db, "ws").unwrap();
    assert_eq!(saved.seq, 2);
    assert_eq!(links[0].source_kind, "import");
    assert_eq!(links[0].actor, "caller");
    assert_eq!(
        LineageLedger::new(&Sha256Hasher).verify(&links),
        VerifyResult::Ok { checked: 2 }
    );
}

#[test]
fn attachment_failure_rolls_back_the_entire_capture_transaction() {
    let db = Database::open_in_memory().unwrap();
    db.connection_for_test()
        .execute_batch(
            "CREATE TRIGGER fail_attachment BEFORE INSERT ON documents
         WHEN NEW.document_type='image' BEGIN SELECT RAISE(ABORT, 'attachment failure'); END;",
        )
        .unwrap();
    let mut request = input("note");
    request.attachments.push(attachment());
    assert!(
        SaveDocument::new(&db, &SequentialIds::new(), &Sha256Hasher)
            .execute(request)
            .is_err()
    );
    // This fails after the parent document, metadata, tags and first link have been inserted.
    let conn = db.connection_for_test();
    for table in [
        "workspaces",
        "documents",
        "document_metadata",
        "document_meta",
        "meta_tags",
        "tag_definitions",
        "tag_assignments",
        "links",
    ] {
        let count: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM {table} WHERE {}",
                    if table == "tag_definitions" {
                        "managed=0"
                    } else {
                        "1=1"
                    }
                ),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "partial write in {table}");
    }
}

#[test]
fn failed_update_restores_the_old_body_and_tags() {
    let db = Database::open_in_memory().unwrap();
    let ids = SequentialIds::new();
    let save = SaveDocument::new(&db, &ids, &Sha256Hasher);
    save.execute(input("note")).unwrap();
    db.connection_for_test().execute_batch(
        "CREATE TRIGGER fail_link BEFORE INSERT ON links BEGIN SELECT RAISE(ABORT, 'link failure'); END;"
    ).unwrap();
    let mut request = input("note");
    request.mode = WriteMode::Update;
    request.document.body_text = "Updated body".into();
    request.metas = vec![MetaAssignment::user("replacement", None)];
    assert!(save.execute(request).is_err());
    let document = DocumentQuery::get(&db, "ws", "note").unwrap().unwrap();
    assert_eq!(document.body_text, "A different first line\n");
    assert_eq!(document.metas, vec![MetaAssignment::user("topic", None)]);
    assert_eq!(LineageQuery::list(&db, "ws").unwrap().len(), 1);
}

#[test]
fn rejects_cross_workspace_or_wrong_target_lineage() {
    for wrong_workspace in [true, false] {
        let db = Database::open_in_memory().unwrap();
        let mut request = input("note");
        if wrong_workspace {
            request.lineage.workspace_id = "other".into();
        } else {
            request.lineage.target_id = "other".into();
        }
        assert!(
            SaveDocument::new(&db, &SequentialIds::new(), &Sha256Hasher)
                .execute(request)
                .is_err()
        );
        assert!(DocumentQuery::get(&db, "ws", "note").unwrap().is_none());
    }
}

#[test]
fn a_run_update_failure_rolls_back_result_and_lineage() {
    let db = Database::open_in_memory().unwrap();
    let mut run = AutomationRun {
        id: "run".into(),
        workspace_id: "ws".into(),
        rule_id: "rule".into(),
        source_document_id: "source".into(),
        result_document_id: None,
        status: RunStatus::Running,
        backend: BackendKind::ApiKey,
        error: None,
        started_at: NOW.into(),
        finished_at: None,
    };
    AutomationRunStore::start(&db, &run).unwrap();
    db.connection_for_test().execute_batch(
        "CREATE TRIGGER fail_run BEFORE UPDATE ON automation_runs BEGIN SELECT RAISE(ABORT, 'run failure'); END;"
    ).unwrap();
    let mut result = input("result");
    result.document.document_type = "automation_result".into();
    result.lineage.source_kind = "document".into();
    run.status = RunStatus::Succeeded;
    run.result_document_id = Some("result".into());
    run.finished_at = Some(NOW.into());
    let ids = SequentialIds::new();
    let commit = CommitAutomationResult {
        store: &db,
        ids: &ids,
        hasher: &Sha256Hasher,
    };
    assert!(
        commit
            .execute(
                &run,
                Some(LinkedDocument {
                    document: result.document,
                    lineage: result.lineage
                })
            )
            .is_err()
    );
    assert!(LineageQuery::list(&db, "ws").unwrap().is_empty());
    let conn = db.connection_for_test();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
    let status: String = conn
        .query_row(
            "SELECT status FROM automation_runs WHERE id='run'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "running");
}

#[test]
fn canonical_updates_are_not_restricted_to_capture_memos() {
    let db = Database::open_in_memory().unwrap();
    let ids = SequentialIds::new();
    let save = SaveDocument::new(&db, &ids, &Sha256Hasher);
    let mut initial = input("report");
    initial.document.document_type = "report".into();
    initial.document.blob_uri = Some("/content/v1".into());
    save.execute(initial).unwrap();
    let mut update = input("report");
    update.mode = WriteMode::Update;
    update.document.document_type = "report".into();
    update.document.blob_uri = Some("/content/v2".into());
    update.document.title = "Revised report".into();
    save.execute(update).unwrap();
    let conn = db.connection_for_test();
    let (title, uri): (String, String) = conn
        .query_row(
            "SELECT title,blob_uri FROM documents WHERE id='report'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(title, "Revised report");
    assert_eq!(uri, "/content/v2");
}

#[test]
fn missing_run_cannot_leave_a_successful_result_without_execution_history() {
    let db = Database::open_in_memory().unwrap();
    let mut result = input("result");
    result.lineage.source_kind = "document".into();
    let run = AutomationRun {
        id: "missing".into(),
        workspace_id: "ws".into(),
        rule_id: "rule".into(),
        source_document_id: "source".into(),
        result_document_id: Some("result".into()),
        status: RunStatus::Succeeded,
        backend: BackendKind::ApiKey,
        error: None,
        started_at: NOW.into(),
        finished_at: Some(NOW.into()),
    };
    let ids = SequentialIds::new();
    let commit = CommitAutomationResult {
        store: &db,
        ids: &ids,
        hasher: &Sha256Hasher,
    };
    assert!(
        commit
            .execute(
                &run,
                Some(LinkedDocument {
                    document: result.document,
                    lineage: result.lineage
                })
            )
            .is_err()
    );
    assert!(LineageQuery::list(&db, "ws").unwrap().is_empty());
    assert!(DocumentQuery::get(&db, "ws", "result").unwrap().is_none());
}
