import type { MemoStateRepository } from "../../domain/ports/MemoStateRepository";

/**
 * 記録をアーカイブする／戻すユースケース（組み込みタグの `archive` 機能）。
 *
 * アーカイブしても記録は消えない。一覧から外れ、検索したときだけ出るようになる。
 */
export class ArchiveMemo {
  constructor(private readonly states: MemoStateRepository) {}

  execute(workspaceId: string, memoId: string, archived: boolean, now: Date): Promise<void> {
    return this.states.setArchived(workspaceId, memoId, archived ? now.toISOString() : null);
  }
}
