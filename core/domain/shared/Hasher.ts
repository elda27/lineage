// ハッシュ計算の抽象。実装は infrastructure/crypto（WebCrypto）。
// domain はこのインターフェースだけを知る。

export interface Hasher {
  sha256Hex(input: string): Promise<string>;
}
