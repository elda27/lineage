import type { MetaTag } from "../meta/MetaTag";

/**
 * 学習済みメタ情報タグの参照ポート。実装は infrastructure 側（SQLite / D1）に置く。
 */
export interface MetaTagRepository {
  /** よく使う順に取得する。 */
  all(workspaceId: string, limit: number): Promise<MetaTag[]>;
}
