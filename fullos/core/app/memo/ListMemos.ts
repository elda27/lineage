import type { Memo } from "../../domain/memo/Memo";
import type { MemoRepository } from "../../domain/ports/MemoRepository";

/** 一覧の既定件数。ダッシュボードと検索画面はこの範囲から絞り込む。 */
export const DEFAULT_MEMO_LIMIT = 200;

/**
 * minos が記録したメモを新しい順に読み出すユースケース。
 *
 * 参照だけなので lineage(link) は追記しない。
 */
export class ListMemos {
  constructor(private readonly memos: MemoRepository) {}

  execute(workspaceId: string, limit: number = DEFAULT_MEMO_LIMIT): Promise<Memo[]> {
    return this.memos.list(workspaceId, limit);
  }
}
