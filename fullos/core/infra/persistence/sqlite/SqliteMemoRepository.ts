import { DOCUMENT_TYPE_MEMO, type Memo, type MetaAssignment } from "../../../domain/memo/Memo";
import { DEFAULT_MEMO_STATE } from "../../../domain/memo/MemoState";
import type { MemoRepository } from "../../../domain/ports/MemoRepository";
import { selectOrEmpty, type SqlHandle } from "./SqlHandle";

type MemoRow = {
  id: string;
  workspace_id: string;
  title: string;
  body_text: string | null;
  created_at: string;
  updated_at: string;
  label: string | null;
  value: string | null;
};

/**
 * ローカル SQLite（minos が書いた `%LOCALAPPDATA%\minos\lineage.db`）から記録を読む。
 *
 * SQL は D1 版と共通にできる形にしてある（実行ハンドルだけが違う）。
 */
export class SqliteMemoRepository implements MemoRepository {
  constructor(private readonly db: SqlHandle) {}

  async list(workspaceId: string, limit: number): Promise<Memo[]> {
    // 件数を絞ってから document_meta を結合する（LIMIT がメタ情報の行数に食われないように）。
    const rows = await selectOrEmpty<MemoRow>(
      this.db,
      `SELECT d.id AS id, d.workspace_id AS workspace_id, d.title AS title,
              d.body_text AS body_text, d.created_at AS created_at, d.updated_at AS updated_at,
              m.label AS label, m.value AS value
       FROM (
         SELECT id, workspace_id, title, body_text, created_at, updated_at
         FROM documents
         WHERE workspace_id = $1 AND document_type = $2
         ORDER BY created_at DESC
         LIMIT $3
       ) d
       LEFT JOIN document_meta m ON m.document_id = d.id
       ORDER BY d.created_at DESC, m.label ASC`,
      [workspaceId, DOCUMENT_TYPE_MEMO, limit],
    );

    return groupByDocument(rows);
  }
}

function groupByDocument(rows: MemoRow[]): Memo[] {
  const memos = new Map<string, Memo>();

  for (const row of rows) {
    let memo = memos.get(row.id);
    if (!memo) {
      memo = {
        id: row.id,
        workspaceId: row.workspace_id,
        title: row.title,
        bodyText: row.body_text ?? "",
        metas: [],
        // 組み込みタグの状態は document_states 側にあり、
        // ListMemos が MemoStateRepository の結果を重ねる。
        state: DEFAULT_MEMO_STATE,
        createdAt: row.created_at,
        updatedAt: row.updated_at,
      };
      memos.set(row.id, memo);
    }
    if (row.label !== null) {
      memo.metas.push(toMetaTag(row));
    }
  }

  return [...memos.values()];
}

function toMetaTag(row: MemoRow): MetaAssignment {
  const label = row.label as string;
  return row.value === null ? { label } : { label, value: row.value };
}
