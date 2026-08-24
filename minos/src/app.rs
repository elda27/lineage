//! composition root。
//!
//! 具体的な実装（SQLite / SHA-256 / システム時計）をユースケースに注入し、
//! features 層（画面）には「何ができるか」だけを見せる。
//! features はここより内側の実装を直接知らない。

use std::rc::Rc;
use std::thread;

use anyhow::Result;

use lineage_core::app::capture::{CaptureMemo, CaptureMemoInput, CaptureMemoOutput};
use lineage_core::app::lineage::VerifyLineage;
use lineage_core::app::meta::CompleteMetaTag;
use lineage_core::app::settings::{LoadSettings, SaveSettings};
use lineage_core::domain::automation::MemoSnapshot;
use lineage_core::domain::capture::CaptureContext;
use lineage_core::domain::lineage::VerifyResult;
use lineage_core::domain::meta::{MetaAssignment, MetaSuggestion};
use lineage_core::domain::ports::MemoQuery;
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
        document_id: Option<String>,
    ) -> Result<CaptureMemoOutput> {
        CaptureMemo::new(&self.database, &self.clock, &self.ids, &self.hasher).execute(
            CaptureMemoInput {
                workspace_id: self.workspace_id.clone(),
                workspace_name: DEFAULT_WORKSPACE_NAME.to_string(),
                body,
                document_id,
                metas,
                context,
            },
        )
    }

    /// 入力画面を待たせず、専用スレッドで1件を保存する。
    ///
    /// SQLite の接続は作成したスレッドで開く。UI が所有する接続を別スレッドへ
    /// 渡さないことで、画面を先に閉じても保存のトランザクションは安全に継続できる。
    pub fn capture_in_background(
        body: String,
        metas: Vec<MetaAssignment>,
        context: Option<CaptureContext>,
        document_id: Option<String>,
    ) -> async_channel::Receiver<Result<CaptureMemoOutput>> {
        let (sender, receiver) = async_channel::bounded(1);
        thread::spawn(move || {
            let result = Database::open_default().and_then(|database| {
                CaptureMemo::new(&database, &SystemClock, &UuidGenerator, &Sha256Hasher).execute(
                    CaptureMemoInput {
                        workspace_id: DEFAULT_WORKSPACE_ID.to_string(),
                        workspace_name: DEFAULT_WORKSPACE_NAME.to_string(),
                        body,
                        document_id,
                        metas,
                        context,
                    },
                )
            });
            _ = sender.send_blocking(result);
        });
        receiver
    }

    pub fn recent_memos(&self, limit: usize) -> Result<Vec<MemoSnapshot>> {
        self.database.recent(&self.workspace_id, limit)
    }

    pub fn memo(&self, document_id: &str) -> Result<Option<MemoSnapshot>> {
        self.database.get(&self.workspace_id, document_id)
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
