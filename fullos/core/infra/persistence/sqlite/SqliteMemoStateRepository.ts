import type { MemoStateRecord } from "../../../domain/memo/MemoState";
import type { MemoStateRepository } from "../../../domain/ports/MemoStateRepository";
import { selectOrEmpty, type SqlHandle } from "./SqlHandle";

type StateRow = {
  document_id: string;
  done: number;
  done_at: string | null;
  archived_at: string | null;
  deleted_at: string | null;
};

/** 組み込みタグの状態（document_states）の読み出し。 */
export class SqliteMemoStateRepository implements MemoStateRepository {
  constructor(private readonly db: SqlHandle) {}

  async all(workspaceId: string): Promise<MemoStateRecord[]> {
    const rows = await selectOrEmpty<StateRow>(
      this.db,
      `SELECT document_id, done, done_at, archived_at, deleted_at
       FROM document_states
       WHERE workspace_id = $1`,
      [workspaceId],
    );

    return rows.map((row) => ({
      documentId: row.document_id,
      state: {
        done: row.done !== 0,
        doneAt: row.done_at,
        archivedAt: row.archived_at,
        deletedAt: row.deleted_at,
      },
    }));
  }
}
