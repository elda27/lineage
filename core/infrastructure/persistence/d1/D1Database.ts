// Cloudflare D1 と構造的に互換な最小インターフェース。
// core 層が @cloudflare/workers-types へ直接依存しないよう形だけ定義する。
// worker は実際の D1Database（互換）を渡す。
export interface D1PreparedStatement {
  bind(...values: unknown[]): D1PreparedStatement;
  run(): Promise<unknown>;
  all<T>(): Promise<{ results: T[] }>;
  first<T>(): Promise<T | null>;
}

export interface D1Database {
  prepare(query: string): D1PreparedStatement;
  batch(statements: D1PreparedStatement[]): Promise<unknown>;
}
