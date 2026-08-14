/**
 * 組み込みタグ（docs/ui.md「組み込みタグ」）。
 *
 * 付けるとアプリ側の機能が有効になることが約束されたメタ情報で、利用者が自由に付ける
 * ふつうのメタ情報（core/domain/meta/MetaTag.ts の学習済みタグ）とは扱いが違う。
 *
 * minos 側に記録の種別は無く、記録に付いた `#タスク` / `#メモ` だけが手がかりになる。
 * どのラベルがどの機能を有効にするかの定義は、画面ごとに散らばると食い違うので
 * ここ1か所に集める。ここは domain なので DB / Tauri / fetch には依存しない。
 */

import type { MetaAssignment } from "./Memo";

/** 組み込みタグの識別子。 */
export type BuiltinTagId = "task" | "memo";

/**
 * 組み込みタグが有効にする機能。
 *
 * - `complete` … 完了フラグ（チェックボタン）。完了のまま fullos を閉じるとアーカイブされる
 * - `archive`  … アーカイブボタン。一覧から外し、検索したときだけ出るようにする
 * - `trash`    … ゴミ箱ボタン。一覧からも検索結果からも外す（論理削除）
 */
export type BuiltinTagCapability = "complete" | "archive" | "trash";

export type BuiltinTag = {
  id: BuiltinTagId;
  /** 画面に出す名前。 */
  displayName: string;
  /**
   * このタグとみなすラベル。
   *
   * minos の入力は日本語でも英語でも通るので、表記ゆれをここで吸収する。
   * 比較は大文字小文字を区別しない。
   */
  labels: string[];
  capabilities: BuiltinTagCapability[];
  /** 一覧で上へ出す強さ。大きいほど上。組み込みタグの無い記録は 0。 */
  priority: number;
};

/** 一覧の優先度が高い順。`primaryBuiltinTag` はこの並びの先頭を返す。 */
export const BUILTIN_TAGS: readonly BuiltinTag[] = [
  {
    id: "task",
    displayName: "タスク",
    labels: ["タスク", "task", "todo"],
    capabilities: ["complete"],
    priority: 20,
  },
  {
    id: "memo",
    displayName: "メモ",
    labels: ["メモ", "memo"],
    capabilities: ["archive", "trash"],
    priority: 10,
  },
];

/** 組み込みタグを持たない記録の優先度。 */
export const NO_BUILTIN_PRIORITY = 0;

/** ラベル1件が組み込みタグかどうか。値（`#ラベル=値`）は見ない。 */
export function builtinTagOfLabel(label: string): BuiltinTag | null {
  const needle = label.trim().toLowerCase();
  return BUILTIN_TAGS.find((tag) => tag.labels.some((l) => l.toLowerCase() === needle)) ?? null;
}

/** 記録に付いている組み込みタグ（優先度の高い順・重複なし）。 */
export function builtinTagsOf(metas: readonly MetaAssignment[]): BuiltinTag[] {
  return BUILTIN_TAGS.filter((tag) =>
    metas.some((meta) => builtinTagOfLabel(meta.label)?.id === tag.id),
  );
}

/** 見た目（アイコンや種類の表示）を決める組み込みタグ。無ければ null。 */
export function primaryBuiltinTag(metas: readonly MetaAssignment[]): BuiltinTag | null {
  return builtinTagsOf(metas)[0] ?? null;
}

/** 記録で使える機能。複数の組み込みタグが付いていれば合算する。 */
export function builtinCapabilities(metas: readonly MetaAssignment[]): BuiltinTagCapability[] {
  const capabilities = builtinTagsOf(metas).flatMap((tag) => tag.capabilities);
  return [...new Set(capabilities)];
}

export function hasBuiltinCapability(
  metas: readonly MetaAssignment[],
  capability: BuiltinTagCapability,
): boolean {
  return builtinCapabilities(metas).includes(capability);
}

/** 一覧での優先度。組み込みタグが複数あれば高いほうを採る。 */
export function builtinPriority(metas: readonly MetaAssignment[]): number {
  return builtinTagsOf(metas)[0]?.priority ?? NO_BUILTIN_PRIORITY;
}

/**
 * その組み込みタグとみなすラベルの一覧。
 *
 * 「完了したタスクをまとめてアーカイブする」のように、SQL 側でラベルを絞り込む
 * ユースケースへ渡す。ラベルの定義を infra へ漏らさないための入口。
 */
export function builtinTagLabels(id: BuiltinTagId): string[] {
  return BUILTIN_TAGS.find((tag) => tag.id === id)?.labels ?? [];
}
