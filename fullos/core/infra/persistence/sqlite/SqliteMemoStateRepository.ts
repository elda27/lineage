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

/**
 * 組み込みタグの状態（document_states）の読み書き。
 *
 * lineage(links) を生まない行なので fullos の webview から直接書く。
 * SQL は D1 版と共通にできる形にしてある（実行ハンドルだけが違う）。
 */
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

  async setDone(workspaceId: string, documentId: string, done: boolean, at: string): Promise<void> {
    await this.write(
      `INSERT INTO document_states (document_id, workspace_id, done, done_at, updated_at)
       VALUES ($1, $2, $3, $4, $5)
       ON CONFLICT(document_id) DO UPDATE
         SET done = excluded.done, done_at = excluded.done_at, updated_at = excluded.updated_at`,
      [documentId, workspaceId, done ? 1 : 0, done ? at : null, at],
    );
  }

  async setArchived(workspaceId: string, documentId: string, at: string | null): Promise<void> {
    // 戻すときも「いつ操作したか」は残したいので、updated_at には現在時刻を入れる。
    const updatedAt = at ?? new Date().toISOString();
    await this.write(
      `INSERT INTO document_states (document_id, workspace_id, archived_at, updated_at)
       VALUES ($1, $2, $3, $4)
       ON CONFLICT(document_id) DO UPDATE
         SET archived_at = excluded.archived_at, updated_at = excluded.updated_at`,
      [documentId, workspaceId, at, updatedAt],
    );
  }

  async trash(workspaceId: string, documentId: string, at: string): Promise<void> {
    await this.write(
      `INSERT INTO document_states (document_id, workspace_id, deleted_at, updated_at)
       VALUES ($1, $2, $3, $4)
       ON CONFLICT(document_id) DO UPDATE
         SET deleted_at = excluded.deleted_at, updated_at = excluded.updated_at`,
      [documentId, workspaceId, at, at],
    );
  }

  async archiveDone(workspaceId: string, labels: string[], at: string): Promise<void> {
    if (labels.length === 0) return;

    // ラベルの表記ゆれは domain 側で吸収済み。ここは大文字小文字だけ合わせる。
    const placeholders = labels.map((_, index) => `$${index + 3}`).join(", ");
    await this.write(
      `UPDATE document_states
       SET archived_at = $1, updated_at = $1
       WHERE workspace_id = $2
         AND done = 1
         AND archived_at IS NULL
         AND deleted_at IS NULL
         AND document_id IN (
           SELECT document_id FROM document_meta WHERE lower(label) IN (${placeholders})
         )`,
      [at, workspaceId, ...labels.map((label) => label.toLowerCase())],
    );
  }

  /**
   * 書き込み。
   *
   * minos を一度も起動していない DB にはテーブルが無い（スキーマを適用するのは
   * lineage-core 側で、fullos の webview は読み書きするだけ）。読み出しは
   * `selectOrEmpty` が空として扱えるが、書き込みは黙って捨てるわけにいかないので、
   * 何をすれば直るかが分かる文言にして投げ直す。
   */
  private async write(query: string, bindValues: unknown[]): Promise<void> {
    try {
      await this.db.execute(query, bindValues);
    } catch (error) {
      if (String(error).includes("no such table")) {
        throw new Error(
          "記録の状態を保存できませんでした。minos を一度起動してデータベースを最新にしてください。",
        );
      }
      throw error;
    }
  }
}
