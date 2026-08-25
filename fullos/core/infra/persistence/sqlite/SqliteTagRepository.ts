import type { TagRepository } from "../../../domain/ports/TagRepository";
import type { TagDefinition } from "../../../domain/tag/TagDefinition";
import type { SqlHandle } from "./SqlHandle";
type Row = {
  id: string;
  workspace_id: string;
  kind: TagDefinition["kind"];
  display_name: string;
  shorthand: string | null;
  usage_count: number;
  last_used_at: string | null;
  enabled: number;
  managed: number;
  deleted_at: string | null;
  view_id: string | null;
  recipe_name: string | null;
  ownership: string | null;
};
export class SqliteTagRepository implements TagRepository {
  constructor(private db: SqlHandle) {}
  async all(workspaceId: string) {
    const rows = await this.db.select<Row[]>(
      `SELECT t.*,v.view_id,a.recipe_name,a.ownership FROM tag_definitions t LEFT JOIN view_bindings v ON v.tag_id=t.id LEFT JOIN automation_bindings a ON a.tag_id=t.id WHERE (t.workspace_id=? OR t.workspace_id='local') AND t.deleted_at IS NULL`,
      [workspaceId],
    );
    return rows.map(map);
  }
  async get(id: string) {
    const rows = await this.db.select<Row[]>(
      `SELECT t.*,v.view_id,a.recipe_name,a.ownership FROM tag_definitions t LEFT JOIN view_bindings v ON v.tag_id=t.id LEFT JOIN automation_bindings a ON a.tag_id=t.id WHERE t.id=?`,
      [id],
    );
    return rows[0] ? map(rows[0]) : null;
  }
}
const map = (r: Row): TagDefinition => ({
  id: r.id,
  workspaceId: r.workspace_id,
  kind: r.kind,
  displayName: r.display_name,
  shorthand: r.shorthand,
  usageCount: r.usage_count,
  lastUsedAt: r.last_used_at,
  enabled: !!r.enabled,
  managed: !!r.managed,
  deletedAt: r.deleted_at,
  view: r.view_id,
  recipe: r.recipe_name,
  recipeManaged: r.ownership === "managed",
});
