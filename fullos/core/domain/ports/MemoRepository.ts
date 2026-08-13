import type { Memo } from "../memo/Memo";

/**
 * 記録の参照ポート。実装は infrastructure 側（SQLite / D1）に置く。
 */
export interface MemoRepository {
  /** 新しい順に取得する。 */
  list(workspaceId: string, limit: number): Promise<Memo[]>;
}
