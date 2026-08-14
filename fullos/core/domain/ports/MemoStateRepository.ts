import type { MemoStateRecord } from "../memo/MemoState";

/**
 * 組み込みタグが付ける状態の読み書きポート。実装は infrastructure 側（SQLite / D1）。
 *
 * 状態は lineage(links) を生まない（記録そのものは変わらず、見せ方だけが変わる）ので、
 * fullos の webview から直接書いてよい行にあたる
 * （docs/concept/MINIMAL_ARCHITECTURE.md 2.「fullos の webview は……」）。
 */
export interface MemoStateRepository {
  /** workspace 内で状態を持つ記録すべて。行の無い記録は既定の状態として扱う。 */
  all(workspaceId: string): Promise<MemoStateRecord[]>;

  /** 完了フラグを立てる/降ろす。 */
  setDone(workspaceId: string, documentId: string, done: boolean, at: string): Promise<void>;

  /** アーカイブする／戻す。`at` が null なら戻す。 */
  setArchived(workspaceId: string, documentId: string, at: string | null): Promise<void>;

  /** ゴミ箱へ入れる（論理削除）。 */
  trash(workspaceId: string, documentId: string, at: string): Promise<void>;

  /**
   * 完了フラグの立った記録をまとめてアーカイブする。
   *
   * 対象は `labels` のいずれかを持つ記録に限る。どのラベルが組み込みタグかは
   * domain の定義（core/domain/memo/BuiltinTag.ts）で決まるので、SQL 側には持たせない。
   */
  archiveDone(workspaceId: string, labels: string[], at: string): Promise<void>;
}
