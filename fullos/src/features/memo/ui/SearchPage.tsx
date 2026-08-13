import { useMemo, useState } from "react";

import { matchesMemoQuery, parseMemoQuery } from "@core/domain/memo/MemoQuery";
import { eyebrow, serifTitle, standardPage } from "@/shared/ui/kit";
import type { LoadState, Memo } from "../service/memoView";
import { MemoList } from "./MemoList";
import { MetaChips } from "./MetaChips";
import { SearchBox } from "./SearchBox";

export function SearchPage({
  memos,
  status,
  query,
  setQuery,
  openMemo,
  toggleMemo,
}: {
  memos: Memo[];
  status: LoadState;
  query: string;
  setQuery: (v: string) => void;
  openMemo: (m: Memo) => void;
  toggleMemo: (id: string) => void;
}) {
  const [filter, setFilter] = useState("すべて");
  // 入力は「キーワード」と「#メタ情報」に分けて解釈する（規則は core/domain/memo/MemoQuery.ts）。
  const parsed = useMemo(() => parseMemoQuery(query), [query]);
  const results = useMemo(
    () =>
      memos.filter(
        (m) =>
          (filter === "すべて" || (filter === "タスク" ? m.type === "task" : m.type === "memo")) &&
          matchesMemoQuery({ title: m.title, bodyText: m.body, metas: m.metas }, parsed),
      ),
    [memos, parsed, filter],
  );
  return (
    <div className={standardPage}>
      <div className="mb-8">
        <p className={eyebrow}>LIBRARY</p>
        <h1 className={`${serifTitle} mb-[9px] text-[34px]`}>検索</h1>
        <p className="text-[13px] text-muted">
          これまでに残したすべての記録を探せます。
          <code className="ml-1 rounded bg-[#f1f0ed] px-1 font-mono text-[11px]">#</code>{" "}
          でメタ情報を補完して絞り込めます。
        </p>
      </div>
      <SearchBox large className="max-w-none" value={query} onChange={setQuery} />
      <div className="mt-[22px] mb-[13px] flex items-center gap-[7px]">
        {["すべて", "メモ", "タスク"].map((f) => (
          <button
            className={`cursor-pointer rounded-[7px] border border-transparent px-[13px] py-1.5 text-[11px] ${filter === f ? "bg-[#ecebe7] font-semibold" : "bg-transparent"}`}
            onClick={() => setFilter(f)}
            key={f}
          >
            {f}
          </button>
        ))}
        {parsed.metas.length > 0 && (
          <span className="flex items-center gap-1.5 border-l border-line pl-[9px]">
            <MetaChips metas={parsed.metas} />
          </span>
        )}
        <span className="ml-auto text-[10px] text-[#999a94]">{results.length} 件</span>
      </div>
      <MemoList
        memos={results}
        status={status}
        openMemo={openMemo}
        toggleMemo={toggleMemo}
        empty={{
          icon: "search",
          title: "一致する記録がありません",
          hint: "キーワードや絞り込みを変えてみてください。",
        }}
      />
    </div>
  );
}
