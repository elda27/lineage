/**
 * 組み込みタグの機能が付ける、記録ごとの状態。
 *
 * 実体は document_states テーブルの1行で、行が無い記録は既定の状態
 * （未完了・未アーカイブ・ゴミ箱でない）として扱う。
 *
 * メタ情報（document_meta）と分けているのは、これが利用者の打った `#タグ` ではなく
 * アプリの操作結果だからで、補完候補の学習にも検索にも混ぜない。
 */

/** 記録1件の状態。 */
export type MemoState = {
  /** 完了フラグ（`complete` 機能のチェック）。 */
  done: boolean;
  /** 完了にした日時（RFC3339）。未完了なら null。 */
  doneAt: string | null;
  /** アーカイブした日時。一覧から外れ、検索したときだけ出る。 */
  archivedAt: string | null;
  /** ゴミ箱に入れた日時。一覧にも検索結果にも出ない。 */
  deletedAt: string | null;
};

/** document_states に行が無い記録の状態。 */
export const DEFAULT_MEMO_STATE: MemoState = {
  done: false,
  doneAt: null,
  archivedAt: null,
  deletedAt: null,
};

/** どの記録の状態かを持つ形。リポジトリが返す単位。 */
export type MemoStateRecord = {
  documentId: string;
  state: MemoState;
};

export function isArchived(state: MemoState): boolean {
  return state.archivedAt !== null;
}

export function isTrashed(state: MemoState): boolean {
  return state.deletedAt !== null;
}
