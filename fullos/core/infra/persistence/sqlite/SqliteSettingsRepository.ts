import { invoke } from "@tauri-apps/api/core";

import { selectOrEmpty, type SqlHandle } from "./SqlHandle";

type SettingRow = { value: string };

/**
 * `settings` テーブルの repository。
 *
 * 読み取りは WebView の plugin-sql、mutation は Rust command に委ねる（ADR-0004）。
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
    await invoke<void>("setting_set", {
      workspaceId,
      key,
      value,
      at: new Date().toISOString(),
    });
  }
}
