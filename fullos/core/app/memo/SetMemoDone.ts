import type { MemoStateRepository } from "../../domain/ports/MemoStateRepository";

/**
 * 完了フラグを切り替えるユースケース（組み込みタグの `complete` 機能）。
 *
 * ここではアーカイブしない。完了のまま fullos を閉じたときに
 * ArchiveCompletedTasks がまとめてアーカイブする（docs/ui.md「組み込みタグ」）。
 */
export class SetMemoDone {
  constructor(private readonly states: MemoStateRepository) {}

  execute(workspaceId: string, memoId: string, done: boolean, now: Date): Promise<void> {
    return this.states.setDone(workspaceId, memoId, done, now.toISOString());
  }
}
