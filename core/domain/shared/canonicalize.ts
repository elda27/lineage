// 正規化 JSON 文字列化。キー順を固定し、同じ内容なら常に同じ文字列になる。
// hash-chain の content_hash はこの出力に対して計算する。
// ローカル/クラウドで分岐させず、必ずこの1本を通すこと。

export function canonicalize(value: unknown): string {
  return stringify(value);
}

function stringify(value: unknown): string {
  if (value === null) return "null";
  if (value === undefined) return "null";

  const t = typeof value;
  if (t === "number") {
    if (!Number.isFinite(value as number)) {
      throw new Error("canonicalize: non-finite number");
    }
    return JSON.stringify(value);
  }
  if (t === "boolean" || t === "string") {
    return JSON.stringify(value);
  }

  if (Array.isArray(value)) {
    return "[" + value.map((v) => stringify(v)).join(",") + "]";
  }

  if (t === "object") {
    const obj = value as Record<string, unknown>;
    const keys = Object.keys(obj).sort();
    const entries = keys.map(
      (k) => JSON.stringify(k) + ":" + stringify(obj[k])
    );
    return "{" + entries.join(",") + "}";
  }

  throw new Error(`canonicalize: unsupported type ${t}`);
}
