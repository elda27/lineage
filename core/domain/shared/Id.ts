// 一意 ID 生成。domain は外部ライブラリに依存しないため WebCrypto の randomUUID を使う
// （Workers / Tauri webview / Node18+ いずれでも利用可能）。

export function newId(): string {
  return crypto.randomUUID();
}
