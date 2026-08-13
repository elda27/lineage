import type { Memo } from "../../core/domain/memo/Memo";

/**
 * UI が依存する唯一のインターフェース。
 *
 * 実装はローカル接続（LocalAppClient / SQLite）とクラウド接続（HttpAppClient / D1）の2つ。
 * UI はどちらに繋がっているかを知らない（docs/concept/MINIMAL_ARCHITECTURE.md 1.）。
 */
export interface ApplicationPort {
  /** 記録を新しい順に取得する。 */
  listMemos(limit?: number): Promise<Memo[]>;
}
