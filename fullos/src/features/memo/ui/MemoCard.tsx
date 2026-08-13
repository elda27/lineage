import { ActionMenu } from "@/features/automation/ui/ActionMenu";
import { relativeTime } from "@/shared/format";
import { Icon } from "@/shared/ui/kit";
import type { Memo } from "../service/memoView";
import { MetaChips } from "./MetaChips";

export function MemoCard({
  memo,
  onOpen,
  onToggle,
}: {
  memo: Memo;
  onOpen: () => void;
  onToggle: () => void;
}) {
  return (
    <article
      className="group relative flex cursor-pointer gap-3.5 border-b border-[#ecece7] px-[18px] py-[17px] outline-none transition duration-150 last:border-b-0 hover:bg-[#fafaf7] focus:bg-[#fafaf7]"
      onClick={onOpen}
      tabIndex={0}
      onKeyDown={(e) => e.key === "Enter" && onOpen()}
    >
      <div
        className={`grid h-8 w-8 place-items-center rounded-lg ${memo.type === "task" ? "bg-[#ebf3ef] text-[#578170]" : "bg-[#eeecf7] text-[#7467bb]"}`}
      >
        <Icon name={memo.type === "task" ? "check" : "file"} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-[9px]">
          <h3
            className={`text-[13px] font-semibold ${memo.done ? "text-[#a0a19c] line-through" : ""}`}
          >
            {memo.title}
          </h3>
          {memo.type === "task" && (
            <button
              className={`grid h-[17px] w-[17px] cursor-pointer place-items-center rounded-[5px] border p-0 text-white ${memo.done ? "border-[#6c967f] bg-[#6c967f]" : "border-[#cfd2cb] bg-white"}`}
              aria-label="完了状態を切り替え"
              onClick={(e) => {
                e.stopPropagation();
                onToggle();
              }}
            >
              {memo.done && <Icon name="check" size={13} />}
            </button>
          )}
        </div>
        {memo.preview && (
          <p className="mt-[5px] mb-2.5 truncate text-[11px] text-muted">{memo.preview}</p>
        )}
        <div className="flex items-center gap-1.5">
          <MetaChips metas={memo.metas} />
          <span className="ml-1 flex items-center gap-1 text-[9px] text-[#a0a19c]">
            <Icon name="clock" size={13} />
            {relativeTime(memo.createdAt)}
          </span>
        </div>
      </div>
      {/* 下書き（まだ保存していない記録）には自動化を当てられない。 */}
      {!memo.id.startsWith("draft-") && (
        <div className="self-center opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100">
          <ActionMenu memoId={memo.id} />
        </div>
      )}
      <button
        className="self-center border-0 bg-transparent text-[#b0b1ab] opacity-0 group-hover:opacity-100"
        aria-label="詳細を開く"
      >
        <Icon name="chevron" />
      </button>
    </article>
  );
}
