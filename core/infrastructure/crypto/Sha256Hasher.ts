import { Hasher } from "../../domain/shared/Hasher";

// WebCrypto による SHA-256。Cloudflare Workers / Tauri webview / Node18+ で動く。
// ローカル・クラウドでハッシュ計算を分岐させないため、この実装1本を両方で使う。
export class Sha256Hasher implements Hasher {
  async sha256Hex(input: string): Promise<string> {
    const bytes = new TextEncoder().encode(input);
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    return [...new Uint8Array(digest)]
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
  }
}
