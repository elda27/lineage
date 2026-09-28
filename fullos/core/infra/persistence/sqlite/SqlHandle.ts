/**
 * Rustのread-only query境界でリポジトリ実装が必要とする部分。
 *
 * 直接プラグインの型に依存しないので、テストでは差し替えられる。
 */
export interface SqlHandle {
  select<T>(query: string, bindValues?: unknown[]): Promise<T>;
}

/**
 * Rust側で初期化後に問い合わせる。schema不整合は呼び出し元へ通知する。
 */
export async function selectOrEmpty<T>(
  db: SqlHandle,
  query: string,
  bindValues: unknown[],
): Promise<T[]> {
  return db.select<T[]>(query, bindValues);
}
