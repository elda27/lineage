import type { MemoStateRecord } from "../memo/MemoState";

/** 組み込みタグが付ける状態の参照ポート。書き込みは Rust の mutation API を通す。 */
export interface MemoStateRepository {
  /** workspace 内で状態を持つ記録すべて。行の無い記録は既定の状態として扱う。 */
  all(workspaceId: string): Promise<MemoStateRecord[]>;
}
