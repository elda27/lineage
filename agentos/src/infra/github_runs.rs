//! Runner-only durable dispatch journal, separate from the shared Note database.
use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dispatch {
    pub execution_id: String,
    pub workspace: String,
    pub note: String,
    pub repo: String,
    pub workflow: String,
    pub git_ref: String,
    pub inputs: serde_json::Value,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct TrackedRun {
    pub request: Dispatch,
    pub source_fingerprint: String,
    pub run_id: Option<u64>,
    pub url: Option<String>,
    pub status: String,
    pub conclusion: Option<String>,
    pub result_note: Option<String>,
}
pub struct RunJournal(Connection);
impl RunJournal {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(
            version == 0 || version == 1,
            "unsupported Runner journal version"
        );
        if version == 0 {
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE github_runs (execution_id TEXT PRIMARY KEY, payload TEXT NOT NULL); PRAGMA user_version=1; COMMIT;")?;
        }
        Ok(Self(connection))
    }
    pub fn get(&self, id: &str) -> Result<Option<TrackedRun>> {
        let text: Option<String> = self
            .0
            .query_row(
                "SELECT payload FROM github_runs WHERE execution_id=?",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        text.map(|v| serde_json::from_str(&v).map_err(Into::into))
            .transpose()
    }
    /// Reserve before sending to prevent concurrent/repeated dispatches.
    pub fn reserve(&self, run: &TrackedRun) -> Result<bool> {
        Ok(self.0.execute(
            "INSERT OR IGNORE INTO github_runs(execution_id,payload) VALUES (?,?)",
            params![run.request.execution_id, serde_json::to_string(run)?],
        )? == 1)
    }
    pub fn update(&self, run: &TrackedRun) -> Result<()> {
        ensure!(
            self.0.execute(
                "UPDATE github_runs SET payload=? WHERE execution_id=?",
                params![serde_json::to_string(run)?, run.request.execution_id]
            )? == 1,
            "execution not found"
        );
        Ok(())
    }
}
