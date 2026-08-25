/**
 * `@tauri-apps/plugin-sql` の Database のうち、リポジトリ実装で必要な部分だけ。
 *
 * 直接プラグインの型に依存しないので、テストでは差し替えられる。
 */
export interface SqlHandle {
  select<T>(query: string, bindValues?: unknown[]): Promise<T>;
}

/**
 * minos を一度も起動していないと DB にテーブルが無い。
 * 「まだ記録が無い」だけなので空一覧として扱う。
 */
export async function selectOrEmpty<T>(
  db: SqlHandle,
  query: string,
  bindValues: unknown[],
): Promise<T[]> {
  try {
    return await db.select<T[]>(query, bindValues);
  } catch (error) {
    if (String(error).includes("no such table")) return [];
    throw error;
  }
}
