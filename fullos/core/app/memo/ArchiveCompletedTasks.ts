import { builtinTagLabels } from "../../domain/memo/BuiltinTag";
import type { MemoStateRepository } from "../../domain/ports/MemoStateRepository";

/**
 * 完了したタスクをまとめてアーカイブするユースケース。
 *
 * fullos を閉じるときに1回だけ呼ぶ（docs/ui.md「組み込みタグ」）。チェックした
 * 瞬間にアーカイブしないのは、閉じるまでは取り消せるようにしておくためで、
 * 「開いている間に見えていたものが消えない」ほうが誤操作に気付ける。
 */
export class ArchiveCompletedTasks {
  constructor(private readonly states: MemoStateRepository) {}

  execute(workspaceId: string, now: Date): Promise<void> {
    return this.states.archiveDone(workspaceId, builtinTagLabels("task"), now.toISOString());
  }
}
