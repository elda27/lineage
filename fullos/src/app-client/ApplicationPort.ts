import type { Account } from "../../core/domain/account/Account";
import type { MetaSuggestion } from "../../core/domain/meta/MetaTag";
import type { Memo } from "../../core/domain/memo/Memo";
import type { StorageUsage } from "../../core/domain/storage/StorageUsage";

/**
 * UI が依存する唯一のインターフェース。
 *
 * 実装はローカル接続（LocalAppClient / SQLite）とクラウド接続（HttpAppClient / D1）の2つ。
 * UI はどちらに繋がっているかを知らない（docs/concept/MINIMAL_ARCHITECTURE.md 1.）。
 */
export interface ApplicationPort {
  /** 記録を新しい順に取得する。 */
  listMemos(limit?: number): Promise<Memo[]>;

  /**
   * 検索バーで `#` を打ったときのメタ情報の補完候補。
   * `query` は `#` を除いた入力文字列（空なら「よく使う順」）。
   */
  suggestMetaTags(query: string, limit?: number): Promise<MetaSuggestion[]>;

  /**
   * ストレージ使用量。割り当て上限を持たない接続では null を返す。
   * UI は null のとき使用量を表示しない。
   */
  storageUsage(): Promise<StorageUsage | null>;

  /**
   * ログイン中のアカウント。認証を持たない接続では null を返す。
   * UI は null のときアカウント欄を表示しない。
   */
  currentAccount(): Promise<Account | null>;
}
