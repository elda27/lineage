import { longDate } from "@/shared/format";
import type { Page } from "@/shared/navigation";
import {
  eyebrow,
  heroPadding,
  Icon,
  quietButton,
  smallPrimaryButton,
  subheading,
} from "@/shared/ui/kit";
import type { LoadState, Memo } from "../service/memoView";
import { MemoList } from "./MemoList";
import { SearchBox } from "./SearchBox";

export function HomePage({
  memos,
  status,
  query,
  setQuery,
  setPage,
  openMemo,
  toggleMemo,
  createMemo,
  enabledRuleCount,
}: {
  memos: Memo[];
  status: LoadState;
  query: string;
  setQuery: (v: string) => void;
  setPage: (p: Page) => void;
  openMemo: (m: Memo) => void;
  toggleMemo: (id: string) => void;
  createMemo: () => void;
  enabledRuleCount: number;
}) {
  const quickCard =
    "flex cursor-pointer items-center gap-3 rounded-[10px] border border-line bg-white p-[15px] text-left hover:-translate-y-px hover:border-[#cfcec7]";
  return (
    <div className="min-h-screen">
      <header className="flex h-[66px] items-center gap-2 border-b border-line px-[38px]">
        <div className="flex-1" />
        <button
          className={quietButton}
          aria-label="テーマ設定を開く"
          onClick={() => setPage("settings")}
        >
          <Icon name="moon" size={17} />
        </button>
        <button className={smallPrimaryButton} onClick={createMemo}>
          <Icon name="plus" size={16} />
          新しいメモ
        </button>
      </header>
      <div className={`mx-auto max-w-[1100px] pt-[68px] pb-[52px] ${heroPadding}`}>
        <p className={eyebrow}>{longDate(new Date())}</p>
        <p className="mb-[34px] text-muted">思考の続きを、ここから始めましょう。</p>
        <SearchBox large value={query} onChange={setQuery} onSubmit={() => setPage("search")} />
      </div>
      <section
        className={`border-t border-line bg-white pt-[42px] pb-[70px] ${heroPadding} [&>*]:mx-auto [&>*]:max-w-[940px]`}
      >
        <div className="mb-[19px] flex items-start justify-between">
          <div>
            <h2 className={subheading}>最近の記録</h2>
            <p className="text-xs text-muted">新しく追加・更新されたメモ</p>
          </div>
          <button
            className="flex cursor-pointer items-center gap-[7px] border-0 bg-transparent text-xs text-[#686a65]"
            onClick={() => setPage("search")}
          >
            すべて表示 <Icon name="arrow" size={15} />
          </button>
        </div>
        <MemoList
          memos={memos.slice(0, 4)}
          status={status}
          openMemo={openMemo}
          toggleMemo={toggleMemo}
          empty={{
            icon: "inbox",
            title: "まだ記録がありません",
            hint: "minos を Alt + Space で呼び出して、最初のメモを残しましょう。",
          }}
        />
        <div className="mt-[15px] grid grid-cols-2 gap-[13px]">
          <button className={quickCard} onClick={() => setPage("search")}>
            <span className="grid h-9 w-9 place-items-center rounded-[9px] bg-[#eeecf7] text-[#7568b8]">
              <Icon name="inbox" />
            </span>
            <span className="flex flex-1 flex-col">
              <b className="text-[12px]">すべての記録</b>
              <small className="mt-[3px] text-[9px] text-[#969791]">
                {memos.length} 件のメモとタスク
              </small>
            </span>
            <Icon name="arrow" />
          </button>
          <button className={quickCard} onClick={() => setPage("automation")}>
            <span className="grid h-9 w-9 place-items-center rounded-[9px] bg-[#e9f2ee] text-[#5a8171]">
              <Icon name="sparkles" />
            </span>
            <span className="flex flex-1 flex-col">
              <b className="text-[12px]">自動化ルール</b>
              <small className="mt-[3px] text-[9px] text-[#969791]">
                {enabledRuleCount} 件が有効
              </small>
            </span>
            <Icon name="arrow" />
          </button>
        </div>
      </section>
    </div>
  );
}
