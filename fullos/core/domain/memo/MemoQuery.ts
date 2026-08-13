/**
 * 検索バーの入力を「本文キーワード」と「メタ情報の条件」に分けて解釈する。
 *
 * docs/ui.md「検索画面」の
 * 「メタ情報の検索は、テキスト入力によるインテントの他に、`#` キーを押すことで、
 * 　登録されたメタ情報を補完することができる」に対応する。
 *
 * 例: `損切り #タスク #銘柄=SOXL`
 *   → キーワード "損切り" かつ `#タスク` を持ち `#銘柄` が SOXL の記録。
 */

import { isTagTerminator } from "../meta/MetaTag";
import { metaText, type MetaAssignment } from "./Memo";

/** `#ラベル` または `#ラベル=値` 1件分の絞り込み条件。 */
export type MetaCondition = {
  label: string;
  /** `#ラベル=値` の値。ラベルだけの条件では undefined。 */
  value?: string;
};

/** 解析済みの検索条件。 */
export type MemoQuery = {
  /** `#` トークンを除いたキーワード。空なら絞り込まない。 */
  text: string;
  /** すべて満たす必要のあるメタ情報の条件（AND）。 */
  metas: MetaCondition[];
};

/** 照合に必要な記録の断片だけ。表示用の型からも渡せるようにしてある。 */
export type SearchableMemo = {
  title: string;
  bodyText: string;
  metas: MetaAssignment[];
};

/**
 * 入力文字列を検索条件へ分解する。
 *
 * `#` トークンの終端規則は minos の入力補完と同じ（空白・`#`・読点）。
 * ラベルの無い裸の `#` は条件にならず、キーワードにも混ぜない
 * （`#` を打った直後に結果が全部消えないようにするため）。
 */
export function parseMemoQuery(input: string): MemoQuery {
  const metas: MetaCondition[] = [];
  let text = "";
  let index = 0;

  while (index < input.length) {
    if (input[index] !== "#") {
      text += input[index];
      index += 1;
      continue;
    }

    let end = index + 1;
    while (end < input.length && !isTagTerminator(input[end])) end += 1;

    const condition = toCondition(input.slice(index + 1, end));
    index = end;
    if (
      condition &&
      !metas.some((m) => m.label === condition.label && m.value === condition.value)
    ) {
      metas.push(condition);
    }
  }

  return { text: text.trim().replace(/\s+/g, " "), metas };
}

/** 記録が検索条件に一致するか。 */
export function matchesMemoQuery(memo: SearchableMemo, query: MemoQuery): boolean {
  return (
    query.metas.every((condition) =>
      memo.metas.some((meta) => matchesCondition(meta, condition)),
    ) && matchesText(memo, query.text)
  );
}

/**
 * ラベルは前方一致で見る。補完を使わず `#タス` まで打った時点でも絞り込めるようにするため。
 * 値は部分一致（`#app=chrome` で `chrome.exe` に当てたい）。
 */
function matchesCondition(meta: MetaAssignment, condition: MetaCondition): boolean {
  if (!meta.label.toLowerCase().startsWith(condition.label.toLowerCase())) return false;
  if (condition.value === undefined) return true;
  return (meta.value ?? "").toLowerCase().includes(condition.value.toLowerCase());
}

function matchesText(memo: SearchableMemo, text: string): boolean {
  if (text === "") return true;
  const haystack = [memo.title, memo.bodyText, ...memo.metas.map(metaText)].join(" ").toLowerCase();
  return haystack.includes(text.toLowerCase());
}

function toCondition(token: string): MetaCondition | null {
  const trimmed = token.trim();
  if (trimmed === "") return null;

  const separator = trimmed.indexOf("=");
  if (separator <= 0) return { label: trimmed };

  const value = trimmed.slice(separator + 1).trim();
  return value === ""
    ? { label: trimmed.slice(0, separator) }
    : { label: trimmed.slice(0, separator), value };
}
