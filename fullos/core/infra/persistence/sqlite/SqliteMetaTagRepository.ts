import type { MetaTag } from "../../../domain/meta/MetaTag";
import type { MetaTagRepository } from "../../../domain/ports/MetaTagRepository";
import { selectOrEmpty, type SqlHandle } from "./SqlHandle";

type MetaTagRow = {
  id: string;
  workspace_id: string;
  label: string;
  shorthand: string | null;
  usage_count: number;
  last_used_at: string | null;
};

/**
 * ローカル SQLite の meta_tags（minos が入力のたびに学習している）から補完候補の母集合を読む。
 *
 * SQL は D1 版と共通にできる形にしてある（実行ハンドルだけが違う）。
 * 読み出し順は minos 側の MetaTagQuery 実装と揃えてある。
 */
export class SqliteMetaTagRepository implements MetaTagRepository {
  constructor(private readonly db: SqlHandle) {}

  async all(workspaceId: string, limit: number): Promise<MetaTag[]> {
    const rows = await selectOrEmpty<MetaTagRow>(
      this.db,
      `SELECT id, workspace_id, label, shorthand, usage_count, last_used_at
       FROM meta_tags
       WHERE workspace_id = $1
       ORDER BY usage_count DESC, last_used_at DESC
       LIMIT $2`,
      [workspaceId, limit],
    );

    return rows.map(toMetaTag);
  }
}

function toMetaTag(row: MetaTagRow): MetaTag {
  return {
    id: row.id,
    workspaceId: row.workspace_id,
    label: row.label,
    shorthand: row.shorthand ?? undefined,
    usageCount: row.usage_count,
    lastUsedAt: row.last_used_at ?? undefined,
  };
}
