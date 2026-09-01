import { useState, type FormEvent } from "react";

import { bodyPreview, metaText, type MetaAssignment } from "@core/domain/memo/Memo";
import { ActionMenu } from "@/features/automation/ui/ActionMenu";
import { absoluteDateTime } from "@/shared/format";
import { Icon, primaryButton, quietButton, secondaryButton } from "@/components/base";
import { can, type Memo, type MemoActions } from "../service/memoView";
import { MetaChips } from "./MetaChips";
import { useMetaCompletion } from "./MetaCompletion";
import { MemoImages } from "./MemoImages";

export function MemoDetail({
  memo,
  close,
  update,
  actions,
}: {
  memo: Memo;
  close: () => void;
  update: (m: Memo) => void;
  actions: MemoActions;
}) {
  const [editing, setEditing] = useState(false),
    [title, setTitle] = useState(memo.title),
    [body, setBody] = useState(memo.body),
    [metas, setMetas] = useState(memo.metas);
  const bodyCompletion = useMetaCompletion({ value: body, onChange: setBody });
  const save = (e: FormEvent) => {
    e.preventDefault();
    update({ ...memo, title, body, metas, preview: bodyPreview(body) });
    setEditing(false);
  };
  const cancelEditing = () => {
    setTitle(memo.title);
    setBody(memo.body);
    setMetas(memo.metas);
    setEditing(false);
  };
  const startEditing = () => {
    setTitle(memo.title);
    setBody(memo.body);
    setMetas(memo.metas);
    setEditing(true);
  };
  const removeMeta = (removed: MetaAssignment) =>
    setMetas((current) => current.filter((meta) => metaText(meta) !== metaText(removed)));
  return (
    <div
      className="fixed inset-0 z-20 flex animate-[fade_0.18s] justify-end bg-[#27272245]"
      onMouseDown={(e) => e.target === e.currentTarget && close()}
    >
      <aside className="h-full w-full overflow-y-auto animate-[slide_0.25s_ease] bg-white px-5 py-[25px] shadow-[-10px_0_40px_#0002] sm:w-[min(540px,80vw)] sm:px-[38px] lg:w-[min(540px,50vw)]">
        <header className="mb-[55px] flex items-center justify-between">
          <button
            className="cursor-pointer border-0 bg-transparent text-xs text-[#777972]"
            onClick={close}
          >
            ← 戻る
          </button>
          <div className="flex">
            {/* 組み込みタグで有効になった操作（docs/ui.md「組み込みタグ」）。 */}
            {can(memo, "complete") && (
              <button
                className={`${quietButton} ${memo.done ? "text-[#578170]" : ""}`}
                aria-label={memo.done ? "未完了に戻す" : "完了にする"}
                aria-pressed={memo.done}
                onClick={() => actions.toggleDone(memo)}
              >
                <Icon name="check" />
              </button>
            )}
            {can(memo, "archive") && (
              <button
                className={`${quietButton} ${memo.archived ? "text-[#578170]" : ""}`}
                aria-label={memo.archived ? "アーカイブから戻す" : "アーカイブする"}
                aria-pressed={memo.archived}
                onClick={() => actions.setArchived(memo, !memo.archived)}
              >
                <Icon name="archive" />
              </button>
            )}
            <button className={quietButton} aria-label="編集" onClick={startEditing}>
              <Icon name="edit" />
            </button>
            <button
              className={`${quietButton} text-[#b05d5d]`}
              aria-label="ゴミ箱へ入れる"
              onClick={() => {
                actions.trash(memo);
                close();
              }}
            >
              <Icon name="trash" />
            </button>
          </div>
        </header>
        <div className="mb-[18px] grid h-[42px] w-[42px] place-items-center rounded-[10px] bg-[#eeecf7] text-[#7063b6]">
          <Icon name={memo.type === "task" ? "check" : "file"} />
        </div>
        {editing ? (
          <form onSubmit={save} className="flex flex-col gap-3.5">
            <input
              className="rounded-lg border border-[#deded8] p-3 font-serif text-[23px] outline-none"
              autoFocus
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
            <div className="relative">
              <textarea
                {...bodyCompletion.inputProps}
                aria-label="本文（# でメタ情報を補完）"
                className="min-h-[150px] w-full resize-y whitespace-pre-wrap rounded-lg border border-[#deded8] p-3 text-xs leading-[1.8] outline-none"
                placeholder="# でメタ情報を補完"
              />
              {bodyCompletion.suggestionsElement}
            </div>
            <div className="rounded-lg border border-[#deded8] p-3">
              <div className="mb-2 text-[11px] text-[#969791]">タグ</div>
              {metas.length > 0 ? (
                <div className="flex flex-wrap gap-1.5">
                  <MetaChips metas={metas} onRemove={removeMeta} />
                </div>
              ) : (
                <p className="text-xs text-[#969791]">タグはありません</p>
              )}
            </div>
            <div className="flex justify-end gap-2">
              <button type="button" className={secondaryButton} onClick={cancelEditing}>
                キャンセル
              </button>
              <button className={primaryButton}>保存する</button>
            </div>
          </form>
        ) : (
          <>
            <h1 className="mb-[21px] font-serif text-[29px] font-normal leading-[1.45]">
              {memo.title}
            </h1>
            <p className="whitespace-pre-wrap border-b border-line pb-7 text-[13px] leading-[2] text-[#686a64]">
              {memo.body}
            </p>
            <MemoImages images={memo.images} />
          </>
        )}
        <div className="mt-[25px] [&>div]:mb-[19px] [&>div]:grid [&>div]:grid-cols-[100px_1fr] [&>div]:items-start [&>div]:text-[11px] [&>div>span:first-child]:text-[#969791]">
          <div>
            <span>作成日時</span>
            <b className="font-medium">{absoluteDateTime(memo.createdAt)}</b>
          </div>
          <div>
            <span>種類</span>
            <b className="font-medium">{memo.type === "task" ? "タスク" : "メモ"}</b>
          </div>
          {memo.capabilities.length > 0 && (
            <div>
              <span>状態</span>
              <b className="font-medium">{stateLabel(memo)}</b>
            </div>
          )}
          <div>
            <span>メタ情報</span>
            <div>
              <MetaChips metas={memo.metas} />
            </div>
          </div>
          {!memo.id.startsWith("draft-") && (
            <div>
              <span>自動化</span>
              <div className="flex">
                <ActionMenu memoId={memo.id} />
              </div>
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}

/**
 * 組み込みタグの状態の表示。
 *
 * 完了したタスクは fullos を閉じるときにアーカイブされるので、その予告も兼ねる。
 */
function stateLabel(memo: Memo): string {
  if (memo.archived) return "アーカイブ済み";
  if (memo.done) return "完了（閉じるときにアーカイブされます）";
  return "未処理";
}
