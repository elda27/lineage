import {
  builtinCapabilities,
  primaryBuiltinTag,
  type BuiltinTagCapability,
  type BuiltinTagId,
} from "@core/domain/memo/BuiltinTag";
import {
  bodyPreview,
  type MetaAssignment,
  type Memo as RecordedMemo,
  type MemoImage,
} from "@core/domain/memo/Memo";
import { isArchived } from "@core/domain/memo/MemoState";

/** 画面が扱う記録。minos が保存した Memo を表示用に整えたもの。 */
export type Memo = {
  id: string;
  title: string;
  body: string;
  preview: string;
  metas: MetaAssignment[];
  images: MemoImage[];
  createdAt: string;
  /** 見た目（アイコンと「種類」の表示）を決める組み込みタグ。無ければ "memo" 扱い。 */
  type: BuiltinTagId;
  /**
   * 組み込みタグによって使えるようになった機能。
   *
   * `type` の既定値と違い、これは実際に付いているタグからしか生えない
   * （`#メモ` の付いていない記録にゴミ箱ボタンは出ない）。
   */
  capabilities: BuiltinTagCapability[];
  done: boolean;
  archived: boolean;
};

export type LoadState = "loading" | "ready" | "error";

/** 一覧のボタンから呼ばれる操作。組み込みタグで有効になったものだけを画面に出す。 */
export type MemoActions = {
  toggleDone: (memo: Memo) => void;
  setArchived: (memo: Memo, archived: boolean) => void;
  trash: (memo: Memo) => void;
};

export function toView(memo: RecordedMemo): Memo {
  return {
    id: memo.id,
    title: memo.title,
    body: memo.bodyText,
    // タイトルは本文1行目なので、一覧では続きだけを見せる。
    preview: bodyPreview(memo.bodyText),
    metas: memo.metas,
    images: memo.images,
    createdAt: memo.createdAt,
    type: primaryBuiltinTag(memo.metas)?.id ?? "memo",
    capabilities: builtinCapabilities(memo.metas),
    done: memo.state.done,
    archived: isArchived(memo.state),
  };
}

/** その機能が使えるか（＝対応する組み込みタグが付いているか）。 */
export function can(memo: Memo, capability: BuiltinTagCapability): boolean {
  return memo.capabilities.includes(capability);
}

/** 一覧に出す記録。アーカイブ済みは検索したときだけ出るので、ここでは外す。 */
export function activeMemos(memos: Memo[]): Memo[] {
  return memos.filter((memo) => !memo.archived);
}
