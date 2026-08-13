import { rankCandidates, type MetaSuggestion } from "../domain/meta/MetaTag";
import type { MetaTagRepository } from "../domain/ports/MetaTagRepository";

/** 補完候補として読み出す学習済みタグの上限（minos の TAG_POOL_LIMIT と同じ）。 */
const TAG_POOL_LIMIT = 500;

/** 一度に返す候補の既定上限。 */
export const DEFAULT_SUGGESTION_LIMIT = 12;

/**
 * 検索バーで `#` を打ったときの補完候補を返すユースケース。
 *
 * 並び順のルールは domain（`rankCandidates`）に置き、ここは「読み出して並べる」だけ。
 * 参照だけなので lineage(link) は追記しない。
 */
export class SuggestMetaTags {
  constructor(private readonly tags: MetaTagRepository) {}

  /** `query` は `#` を除いた入力文字列。空なら「よく使う順」。 */
  async execute(
    workspaceId: string,
    query: string,
    limit: number = DEFAULT_SUGGESTION_LIMIT,
  ): Promise<MetaSuggestion[]> {
    const tags = await this.tags.all(workspaceId, TAG_POOL_LIMIT);
    return rankCandidates(tags, query, limit);
  }
}
