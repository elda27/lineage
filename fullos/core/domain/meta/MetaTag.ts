/**
 * メタ情報（`#タグ`）の学習結果と、その入力補完のルール。
 *
 * docs/ui.md「minos」2.／「検索画面」に対応する。minos 側の同じ規則
 * （minos/src/domain/meta.rs）を TypeScript へ写したもので、両アプリで
 * 候補の並び順が食い違わないようにここを唯一の定義とする。
 *
 * - 候補は「過去にユーザが入力したメタ情報」（meta_tags）から作る
 * - 短縮文字列(shorthand)を定義すると、その先頭一致でも候補に出る
 *   （`#タスク` に `task` を設定 → `#t` で候補、`#ta` でさらに絞り込み）
 */

/** 学習済みのメタ情報タグ。補完候補の母集合になる。 */
export type MetaTag = {
  id: string;
  workspaceId: string;
  label: string;
  /** fullos の設定画面で定義する短縮文字列。未設定なら undefined。 */
  shorthand?: string;
  usageCount: number;
  lastUsedAt?: string;
};

/**
 * 補完候補が「どう一致したか」。並び順の決定と、候補一覧の説明表示に使う。
 *
 * 配列の順序がそのまま優先順位（前ほど上位）。
 */
export const MATCH_ORDER = ["labelPrefix", "shorthandPrefix", "labelContains"] as const;

export type MatchKind = (typeof MATCH_ORDER)[number];

/** 並び替え済みの補完候補。 */
export type MetaSuggestion = {
  label: string;
  shorthand?: string;
  usageCount: number;
  matched: MatchKind;
};

/** カーソル位置から遡って見つけた、補完対象の `#` トークン。 */
export type TagToken = {
  /** `#` の位置（JS の文字列インデックス）。 */
  start: number;
  /** `#` を除いた入力文字列。`#` を打った直後は空文字。 */
  query: string;
};

/** `#タグ` を終端する文字。空白のほか、読点や `#` でも区切る。 */
export function isTagTerminator(character: string): boolean {
  return /\s/.test(character) || character === "#" || character === "," || character === "、";
}

/**
 * カーソル位置から遡って、補完対象になっている `#` トークンを探す。
 *
 * カーソルが `#` トークンの中にいない場合は null。
 */
export function findActiveTagToken(textBeforeCursor: string): TagToken | null {
  for (let index = textBeforeCursor.length - 1; index >= 0; index -= 1) {
    const character = textBeforeCursor[index];
    if (character === "#") {
      return { start: index, query: textBeforeCursor.slice(index + 1) };
    }
    if (isTagTerminator(character)) return null;
  }
  return null;
}

/**
 * 学習済みタグから、クエリに対する補完候補を順位付きで返す。
 *
 * `query` は `#` を除いた入力（空文字なら「よく使う順」）。
 */
export function rankCandidates(tags: MetaTag[], query: string, limit: number): MetaSuggestion[] {
  const needle = query.trim().toLowerCase();

  return tags
    .flatMap((tag) => {
      const matched = matchKind(tag, needle);
      if (!matched) return [];
      return [{ label: tag.label, shorthand: tag.shorthand, usageCount: tag.usageCount, matched }];
    })
    .sort(
      (a, b) =>
        MATCH_ORDER.indexOf(a.matched) - MATCH_ORDER.indexOf(b.matched) ||
        b.usageCount - a.usageCount ||
        a.label.localeCompare(b.label),
    )
    .slice(0, limit);
}

function matchKind(tag: MetaTag, needle: string): MatchKind | null {
  if (needle === "") return "labelPrefix";

  const label = tag.label.toLowerCase();
  if (label.startsWith(needle)) return "labelPrefix";
  if (tag.shorthand?.toLowerCase().startsWith(needle)) return "shorthandPrefix";
  if (label.includes(needle)) return "labelContains";
  return null;
}
