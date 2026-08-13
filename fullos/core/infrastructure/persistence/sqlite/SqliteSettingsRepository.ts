import { selectOrEmpty, type SqlHandle } from "./SqlHandle";

type SettingRow = { value: string };

/**
 * `settings` テーブルの読み書き。
 *
 * minos が書いた値と同じ行を fullos からも編集する（docs/ui.md「fullos」4.）。
 * lineage を生まない設定なので、fullos が直接書いてよい。
 */
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

  async set(workspaceId: string, key: string, value: string): Promise<void> {
    await this.db.execute(
      `INSERT INTO settings (workspace_id, key, value, updated_at)
       VALUES ($1, $2, $3, $4)
       ON CONFLICT(workspace_id, key)
       DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at`,
      [workspaceId, key, value, new Date().toISOString()],
    );
  }
}
