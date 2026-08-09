//! Repository インターフェース。実装は infrastructure 側に置く。
//!
//! 依存方向は presentation/infrastructure → application → domain。
//! ここには SQL も Win32 も現れない。

use anyhow::Result;

use crate::domain::capture::DocumentAsset;
use crate::domain::lineage::LineageRecord;
use crate::domain::meta::{MetaAssignment, MetaTag};

/// 書き込みのトランザクション境界。
///
/// document の insert と link の append は必ず同一トランザクションで確定させる必要がある
/// （hash-chain を切らないため）。そのため、書き込み系はこのポート越しにまとめて行う。
pub trait CaptureStore {
    fn transact(&self, work: &mut dyn FnMut(&mut dyn CaptureTx) -> Result<()>) -> Result<()>;
}

/// トランザクション内で使える操作。
pub trait CaptureTx {
    /// 未作成なら workspace を作る。
    fn ensure_workspace(&mut self, id: &str, name: &str, now: &str) -> Result<()>;

    fn insert_document(&mut self, document: &DocumentAsset) -> Result<()>;

    fn insert_document_meta(
        &mut self,
        id: &str,
        document_id: &str,
        meta: &MetaAssignment,
        now: &str,
    ) -> Result<()>;

    /// メタ情報タグの学習。未登録なら作成し、使用回数と最終使用日時を更新する。
    fn learn_meta_tag(&mut self, id: &str, workspace_id: &str, label: &str, now: &str)
    -> Result<()>;

    /// 鎖の末尾（`seq` 最大）を返す。
    fn last_link(&mut self, workspace_id: &str) -> Result<Option<LineageRecord>>;

    /// 鎖に1件追記する。append-only なので更新・削除の口は用意しない。
    fn append_link(&mut self, link: &LineageRecord) -> Result<()>;
}

/// 補完候補の母集合を読み出す。
pub trait MetaTagQuery {
    fn all(&self, workspace_id: &str, limit: usize) -> Result<Vec<MetaTag>>;
}

/// 台帳を seq 昇順で読み出す（検証用）。
pub trait LineageQuery {
    fn list(&self, workspace_id: &str) -> Result<Vec<LineageRecord>>;
}

/// 利用者の設定の読み書き。
pub trait SettingsRepository {
    fn all(&self, workspace_id: &str) -> Result<Vec<(String, String)>>;
    fn set(&self, workspace_id: &str, key: &str, value: &str, now: &str) -> Result<()>;
}
