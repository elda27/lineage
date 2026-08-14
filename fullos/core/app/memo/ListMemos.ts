import { compareMemosForList, type Memo } from "../../domain/memo/Memo";
import { DEFAULT_MEMO_STATE, isTrashed } from "../../domain/memo/MemoState";
import type { MemoRepository } from "../../domain/ports/MemoRepository";
import type { MemoStateRepository } from "../../domain/ports/MemoStateRepository";

/** 一覧の既定件数。ダッシュボードと検索画面はこの範囲から絞り込む。 */
export const DEFAULT_MEMO_LIMIT = 200;

/**
 * minos が記録したメモを、組み込みタグの状態を添えて読み出すユースケース。
 *
 * ゴミ箱の記録は落とし、組み込みタグの付いた記録を先に並べる。アーカイブ済みは
 * 落とさずに返す（検索したときだけ出す判断は画面側が持つ）。
 *
 * 参照だけなので lineage(link) は追記しない。
 */
export class ListMemos {
  constructor(
    private readonly memos: MemoRepository,
    private readonly states: MemoStateRepository,
  ) {}

  async execute(workspaceId: string, limit: number = DEFAULT_MEMO_LIMIT): Promise<Memo[]> {
    const [memos, states] = await Promise.all([
      this.memos.list(workspaceId, limit),
      this.states.all(workspaceId),
    ]);
    const byDocument = new Map(states.map((record) => [record.documentId, record.state]));

    return memos
      .map((memo) => ({ ...memo, state: byDocument.get(memo.id) ?? DEFAULT_MEMO_STATE }))
      .filter((memo) => !isTrashed(memo.state))
      .sort(compareMemosForList);
  }
}
