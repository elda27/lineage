//! Repository インターフェース。実装は infrastructure 側に置く。
//!
//! 依存方向は composition root → features / infra → domain。
//! ここには SQL も Win32 も現れない。

use anyhow::Result;

use lineage_core::domain::automation::{AutomationRule, AutomationRun};
use lineage_core::domain::document::{DocumentAsset, DocumentSnapshot};
use lineage_core::domain::lineage::LineageRecord;
use lineage_core::domain::meta::{DocumentMetadata, MetaAssignment, MetaTag};
use lineage_core::domain::mutation::{MutationRequest, MutationResult};
use lineage_core::domain::tag::TagDefinition;

/// タグの参照境界。変更は `MutationStore` の typed delta を通す。
pub trait TagRepository {
    fn list(&self, workspace_id: &str, include_deleted: bool) -> Result<Vec<TagDefinition>>;
    fn get(&self, id: &str) -> Result<Option<TagDefinition>>;
}

/// document と link を同一トランザクションで確定させるための最小の口。
///
/// この2つが別トランザクションになると、片方だけ commit されたときに hash-chain が
/// 切れる。記録の取り込み（minos）でも自動化の結果保存でも要件は同じなので、
/// 共通の親トレイトとして切り出してある。
pub trait LedgerTx {
    fn insert_document(&mut self, document: &DocumentAsset) -> Result<()>;
    fn update_document(&mut self, document: &DocumentAsset) -> Result<()>;

    /// 鎖の末尾（`seq` 最大）を返す。
    fn last_link(&mut self, workspace_id: &str) -> Result<Option<LineageRecord>>;

    /// 鎖に1件追記する。append-only なので更新・削除の口は用意しない。
    fn append_link(&mut self, link: &LineageRecord) -> Result<()>;
}

/// 書き込みのトランザクション境界。
///
/// document の insert と link の append は必ず同一トランザクションで確定させる必要がある
/// （hash-chain を切らないため）。そのため、書き込み系はこのポート越しにまとめて行う。
pub trait DocumentStore {
    fn transact(&self, work: &mut dyn FnMut(&mut dyn DocumentTx) -> Result<()>) -> Result<()>;
}

/// 記録の取り込みでトランザクション内から使える操作。
pub trait DocumentTx: LedgerTx {
    /// 未作成なら workspace を作る。
    fn ensure_workspace(&mut self, id: &str, name: &str, now: &str) -> Result<()>;
    fn clear_document_metas(&mut self, document_id: &str) -> Result<()>;

    fn insert_document_meta(
        &mut self,
        id: &str,
        document_id: &str,
        meta: &MetaAssignment,
        now: &str,
    ) -> Result<()>;
    fn insert_document_metadata(
        &mut self,
        id: &str,
        document_id: &str,
        metadata: &DocumentMetadata,
        now: &str,
    ) -> Result<()>;

    /// メタ情報タグの学習。未登録なら作成し、使用回数と最終使用日時を更新する。
    fn learn_meta_tag(
        &mut self,
        id: &str,
        workspace_id: &str,
        label: &str,
        now: &str,
    ) -> Result<()>;
}

/// 自動化の結果を確定させるトランザクション境界。
pub trait AutomationStore {
    fn transact(&self, work: &mut dyn FnMut(&mut dyn AutomationTx) -> Result<()>) -> Result<()>;
}

/// 自動化の結果保存でトランザクション内から使える操作。
///
/// 結果 document・link・run の確定を1つの tx に収める。run だけ先に成功と記録して
/// document の insert が失敗する、といった食い違いを作らないため。
pub trait AutomationTx: LedgerTx {
    /// 実行結果（成否・結果 document・エラー）を確定する。
    fn finish_run(&mut self, run: &AutomationRun) -> Result<()>;
}

/// 自動化ルールの読み出し。
///
/// 書き込み（作成・差分更新・削除）はこの query port に含めず、Rust の
/// `MutationStore` へ集約する。FullOS の WebView は SQL を直接実行しない。
pub trait AutomationRuleQuery {
    /// workspace のルールをすべて返す（無効なものも含む。絞り込みは domain の役目）。
    fn all(&self, workspace_id: &str) -> Result<Vec<AutomationRule>>;

    fn get(&self, id: &str) -> Result<Option<AutomationRule>>;
}

/// 実行履歴の読み書き。
pub trait AutomationRunStore {
    /// 実行開始を `running` として記録する。
    fn start(&self, run: &AutomationRun) -> Result<()>;

    fn recent(&self, workspace_id: &str, limit: usize) -> Result<Vec<AutomationRun>>;

    /// このルールがまだ処理していない記録を新しい順に返す。
    ///
    /// 「処理済み」は成功済みか実行中の run があること。失敗した記録は再び返るので、
    /// 一時的な失敗（鍵の未登録・通信断）は次の poll で自然に再試行される。
    ///
    /// メタ情報での絞り込みはここでは行わない。条件の解釈は domain の `matches` 1本に
    /// 寄せたいので、SQL は「未処理か」だけを見て、絞り込みは呼び出し側で行う。
    fn unprocessed_memos(
        &self,
        workspace_id: &str,
        rule_id: &str,
        scan_limit: usize,
    ) -> Result<Vec<DocumentSnapshot>>;

    /// このルールが最後に実行を開始した時刻。スケジュールの発火判定に使う。
    fn last_started_at(&self, rule_id: &str) -> Result<Option<String>>;
}

/// 本文とタグの参照境界。入力の再編集と自動化のどちらでも使う。
pub trait DocumentQuery {
    fn get(&self, workspace_id: &str, document_id: &str) -> Result<Option<DocumentSnapshot>>;
    fn recent(&self, workspace_id: &str, limit: usize) -> Result<Vec<DocumentSnapshot>>;
}

/// 補完候補の母集合を読み出す。
pub trait MetaTagQuery {
    fn all(&self, workspace_id: &str, limit: usize) -> Result<Vec<MetaTag>>;
}

/// 台帳を seq 昇順で読み出す（検証用）。
pub trait LineageQuery {
    fn list(&self, workspace_id: &str) -> Result<Vec<LineageRecord>>;
}

/// 利用者の設定の参照境界。変更は `MutationStore` の typed delta を通す。
pub trait SettingsRepository {
    fn all(&self, workspace_id: &str) -> Result<Vec<(String, String)>>;
}

/// 差分 mutation を冪等かつトランザクショナルに適用する書き込み境界。
///
/// UI や transport は SQL を知らず、この契約だけをローカル writer / 将来の remote API
/// へ渡す。`base_revision` の比較、revision 採番、mutation log への記録は store が同じ
/// transaction 内で行う。
pub trait MutationStore {
    fn apply_mutation(
        &self,
        request: &MutationRequest,
        recorded_at: &str,
    ) -> Result<MutationResult>;
}

/// 現在時刻。テストから差し替えられるようにトレイトにする。
pub trait Clock {
    /// RFC3339 (UTC) 文字列を返す。DB にはこの形式で保存する。
    fn now_rfc3339(&self) -> String;
}

/// 一意な ID を生成する。
pub trait IdGenerator {
    fn new_id(&self) -> String;
}
