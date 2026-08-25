import { selectOrEmpty, type SqlHandle } from "./SqlHandle";

type SettingRow = { value: string };

/** `settings` テーブルの読み出し。書き込みは Rust の mutation API を通す。 */
export class SqliteSettingsRepository {
  constructor(private readonly db: SqlHandle) {}

  async get(workspaceId: string, key: string): Promise<string | null> {
    const rows = await selectOrEmpty<SettingRow>(
      this.db,
      "SELECT value FROM settings WHERE workspace_id = $1 AND key = $2",
      [workspaceId, key],
    );
    return rows[0]?.value ?? null;
  }
}
