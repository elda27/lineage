// @tauri-apps/plugin-sql の Database と構造的に互換な最小インターフェース。
// core 層が Tauri パッケージへ直接依存しないよう、ここで形だけ定義する。
// LocalAppClient が実際の Database インスタンス（互換）を渡す。
export interface SqlDatabase {
  execute(query: string, bindValues?: unknown[]): Promise<unknown>;
  select<T>(query: string, bindValues?: unknown[]): Promise<T>;
}

// BEGIN/COMMIT で複数文を原子的に確定する。失敗時は ROLLBACK。
export async function runInTransaction(
  db: SqlDatabase,
  work: () => Promise<void>
): Promise<void> {
  await db.execute("BEGIN");
  try {
    await work();
    await db.execute("COMMIT");
  } catch (e) {
    await db.execute("ROLLBACK");
    throw e;
  }
}
