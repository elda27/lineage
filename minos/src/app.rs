//! composition root。
//!
//! 具体的な実装（SQLite / SHA-256 / システム時計）をユースケースに注入し、
//! features 層（画面）には「何ができるか」だけを見せる。
//! features はここより内側の実装を直接知らない。

use std::rc::Rc;

use anyhow::Result;

use lineage_core::app::capture::{CaptureMemo, CaptureMemoInput, CaptureMemoOutput};
use lineage_core::app::meta::CompleteMetaTag;
use lineage_core::app::settings::{LoadSettings, SaveSettings};
use lineage_core::app::lineage::VerifyLineage;
use lineage_core::domain::capture::CaptureContext;
use lineage_core::domain::lineage::VerifyResult;
use lineage_core::domain::meta::{MetaAssignment, MetaSuggestion};
use lineage_core::domain::settings::Settings;
use lineage_core::infra::clock::{SystemClock, UuidGenerator};
use lineage_core::infra::crypto::Sha256Hasher;
use lineage_core::infra::sqlite::Database;

/// minos は単一利用者なので、既定のワークスペースは1つ固定。
/// クラウド接続に切り替えるときは、ここが利用者ごとの workspace になる。
const DEFAULT_WORKSPACE_ID: &str = "local";
const DEFAULT_WORKSPACE_NAME: &str = "minos";

pub struct Services {
    database: Database,
    clock: SystemClock,
    ids: UuidGenerator,
    hasher: Sha256Hasher,
    workspace_id: String,
}

impl Services {
    pub fn new(database: Database) -> Rc<Self> {
        Rc::new(Self {
            database,
            clock: SystemClock,
            ids: UuidGenerator,
            hasher: Sha256Hasher,
            workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
        })
    }

    /// 入力を1件確定する（document + lineage を同一トランザクションで保存）。
    pub fn capture(
        &self,
        body: String,
        metas: Vec<MetaAssignment>,
        context: Option<CaptureContext>,
    ) -> Result<CaptureMemoOutput> {
        CaptureMemo::new(&self.database, &self.clock, &self.ids, &self.hasher).execute(
            CaptureMemoInput {
                workspace_id: self.workspace_id.clone(),
                workspace_name: DEFAULT_WORKSPACE_NAME.to_string(),
                body,
                metas,
                context,
            },
        )
    }

    /// `#` の入力補完候補。
    pub fn suggest_meta_tags(&self, query: &str, limit: usize) -> Result<Vec<MetaSuggestion>> {
        CompleteMetaTag::new(&self.database).execute(&self.workspace_id, query, limit)
    }

    /// 保存されている設定（未保存なら既定値）。
    pub fn load_settings(&self) -> Result<Settings> {
        LoadSettings::new(&self.database).execute(&self.workspace_id)
    }

    /// 設定を保存する。fullos の設定画面からも同じ行を編集する。
    pub fn save_settings(&self, settings: Settings) -> Result<()> {
        SaveSettings::new(&self.database, &self.clock).execute(&self.workspace_id, settings)
    }

    /// hash-chain の検証。
    pub fn verify_lineage(&self) -> Result<VerifyResult> {
        VerifyLineage::new(&self.database, &self.hasher).execute(&self.workspace_id)
    }
}
