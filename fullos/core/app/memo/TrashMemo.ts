import type { MemoStateRepository } from "../../domain/ports/MemoStateRepository";

/**
 * 記録をゴミ箱へ入れるユースケース（組み込みタグの `trash` 機能）。
 *
 * documents の行は消さない。links が指す先を消すと hash-chain の辿れない link が
 * 残るため、削除は deleted_at を立てる論理削除で表す
 * （docs/concept/MINIMAL_ARCHITECTURE.md 4.）。
 */
export class TrashMemo {
  constructor(private readonly states: MemoStateRepository) {}

  execute(workspaceId: string, memoId: string, now: Date): Promise<void> {
    return this.states.trash(workspaceId, memoId, now.toISOString());
  }
}
