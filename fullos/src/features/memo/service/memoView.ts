import {
  bodyPreview,
  isTask,
  type MetaAssignment,
  type Memo as RecordedMemo,
} from "@core/domain/memo/Memo";

/** 画面が扱う記録。minos が保存した Memo を表示用に整えたもの。 */
export type Memo = {
  id: string;
  title: string;
  body: string;
  preview: string;
  metas: MetaAssignment[];
  createdAt: string;
  type: "task" | "memo";
  done?: boolean;
};

export type LoadState = "loading" | "ready" | "error";

export function toView(memo: RecordedMemo): Memo {
  return {
    id: memo.id,
    title: memo.title,
    body: memo.bodyText,
    // タイトルは本文1行目なので、一覧では続きだけを見せる。
    preview: bodyPreview(memo.bodyText),
    metas: memo.metas,
    createdAt: memo.createdAt,
    type: isTask(memo) ? "task" : "memo",
  };
}
