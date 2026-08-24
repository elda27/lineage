/**
 * minos が記録した「入力1件」を fullos 側から見たときの姿。
 *
 * 実体は documents テーブルの document_type = 'memo' の行と、
 * それに紐づく document_meta（メタ情報）である。
 * ここは domain なので DB / Tauri / fetch には一切依存しない。
 */

import { builtinPriority, NO_BUILTIN_PRIORITY } from "./BuiltinTag";
import type { MemoState } from "./MemoState";

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

/** minos がメモへ添付したローカル画像。 */
export type MemoImage = {
  id: string;
  name: string;
  /** minos の attachments ディレクトリにある画像の絶対パス。 */
  path: string;
};

/** 記録本体。 */
export type Memo = {
  id: string;
  workspaceId: string;
  /** minos が本文1行目から導出したタイトル。 */
  title: string;
  bodyText: string;
  metas: MetaAssignment[];
  images: MemoImage[];
  /**
   * 組み込みタグの機能が付けた状態（完了・アーカイブ・ゴミ箱）。
   *
   * documents ではなく document_states に持つので、記録そのものとは別の
   * リポジトリから来る。状態を一度も変えていない記録は DEFAULT_MEMO_STATE。
   */
  state: MemoState;
  /** RFC3339（UTC）。 */
  createdAt: string;
  updatedAt: string;
};

/**
 * 一覧の並び順。
 *
 * 組み込みタグの付いた記録を先に出す（docs/ui.md「組み込みタグ」）。
 * 完了したタスクは優先度を失い、同じ優先度どうしでは新しい順になる。
 * 完了しても即座に消えないのは、閉じるまで取り消せるようにしておくため。
 */
export function compareMemosForList(a: Memo, b: Memo): number {
  return listPriority(b) - listPriority(a) || b.createdAt.localeCompare(a.createdAt);
}

function listPriority(memo: Memo): number {
  return memo.state.done ? NO_BUILTIN_PRIORITY : builtinPriority(memo.metas);
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
