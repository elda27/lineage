// 時刻取得。ISO 8601 文字列でそろえる（schema の created_at / updated_at は TEXT）。

export function nowIso(): string {
  return new Date().toISOString();
}
