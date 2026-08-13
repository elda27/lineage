/**
 * minos が記録した「入力1件」を fullos 側から見たときの姿。
 *
 * 実体は documents テーブルの document_type = 'memo' の行と、
 * それに紐づく document_meta（メタ情報）である。
 * ここは domain なので DB / Tauri / fetch には一切依存しない。
 */

/** document_type の値。minos の `DOCUMENT_TYPE_MEMO` と対応する。 */
export const DOCUMENT_TYPE_MEMO = "memo";

/**
 * 記録に付与されたメタ情報1件（`#タスク` や `#app=chrome.exe`）。
 *
 * 補完候補の母集合になる「学習済みタグ」（core/domain/meta/MetaTag.ts）とは別物で、
 * minos の `MetaAssignment` / `MetaTag` の呼び分けに合わせている。
 */
export type MetaAssignment = {
  label: string;
  /** `#label=value` の value。値なしのタグでは undefined。 */
  value?: string;
};

/** 記録本体。 */
export type Memo = {
  id: string;
  workspaceId: string;
  /** minos が本文1行目から導出したタイトル。 */
  title: string;
  bodyText: string;
  metas: MetaAssignment[];
  /** RFC3339（UTC）。 */
  createdAt: string;
  updatedAt: string;
};

/**
 * タスクとして扱うメタ情報のラベル。
 *
 * minos 側にタスク種別は無く、利用者が付けた `#タスク` が唯一の手がかりになる
 * （docs/ui.md「自動化画面」でもメタ情報でタスクを束ねる想定）。
 */
const TASK_LABELS = ["タスク", "task"];

export function isTask(memo: Memo): boolean {
  return memo.metas.some((meta) => TASK_LABELS.includes(meta.label));
}

/** タイトルは本文1行目なので、一覧では2行目以降だけを本文プレビューにする。 */
export function bodyPreview(bodyText: string): string {
  const lines = bodyText.split("\n");
  const titleLine = lines.findIndex((line) => line.trim() !== "");
  if (titleLine < 0) return "";
  return lines
    .slice(titleLine + 1)
    .join("\n")
    .trim();
}

/** 検索やタグ表示に使う文字列表現（`label` または `label=value`）。 */
export function metaText(meta: MetaAssignment): string {
  return meta.value ? `${meta.label}=${meta.value}` : meta.label;
}
