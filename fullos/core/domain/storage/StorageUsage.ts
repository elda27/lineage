/**
 * ワークスペースに割り当てられたストレージの使用量。
 *
 * 割り当て上限(quota)を持つのはクラウド接続だけである。
 * ローカル接続は利用者のディスクへ直接書くので上限という概念が無く、
 * ApplicationPort は null を返す（docs/concept/MINIMAL_ARCHITECTURE.md 1.）。
 * ここは domain なので DB / Tauri / fetch には一切依存しない。
 */
export type StorageUsage = {
  usedBytes: number;
  /** 割り当て上限。 */
  quotaBytes: number;
};

/** 使用率（0〜1）。上限が未設定・超過のときも表示に使える範囲へ丸める。 */
export function usageRatio(usage: StorageUsage): number {
  if (usage.quotaBytes <= 0) return 0;
  return Math.min(Math.max(usage.usedBytes, 0) / usage.quotaBytes, 1);
}
