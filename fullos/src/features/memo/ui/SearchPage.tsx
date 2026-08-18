import { useMemo, useState } from "react";

import { matchesMemoQuery, parseMemoQuery } from "@core/domain/memo/MemoQuery";
import { eyebrow, serifTitle, standardPage } from "@/shared/ui/kit";
import type { LoadState, Memo, MemoActions } from "../service/memoView";
import { MemoList } from "./MemoList";
import { MetaChips } from "./MetaChips";
import { SearchBox } from "./SearchBox";

const FILTERS = ["すべて", "メモ", "タスク", "アーカイブ"] as const;

export function SearchPage({
  memos,
  status,
  query,
  setQuery,
  openMemo,
  actions,
}: {
  memos: Memo[];
  status: LoadState;
  query: string;
  setQuery: (v: string) => void;
  openMemo: (m: Memo) => void;
  actions: MemoActions;
}) {
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>("すべて");
  // 入力は「キーワード」と「#メタ情報」に分けて解釈する（規則は core/domain/memo/MemoQuery.ts）。
  const parsed = useMemo(() => parseMemoQuery(query), [query]);
  // アーカイブ済みは「検索したとき」だけ出す（docs/ui.md「組み込みタグ」）。
  // 条件を何も入れていない状態は一覧と同じなので、アーカイブは伏せたままにする。
  const searching = parsed.text !== "" || parsed.metas.length > 0;
  const results = useMemo(
    () =>
      memos.filter(
        (m) =>
          matchesFilter(m, filter) &&
          (filter === "アーカイブ" || searching || !m.archived) &&
          matchesMemoQuery({ title: m.title, bodyText: m.body, metas: m.metas }, parsed),
      ),
    [memos, parsed, filter, searching],
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
          <br />
          アーカイブした記録は一覧には出ませんが、ここで検索すれば見つかります。
        </p>
      </div>
      <SearchBox large className="max-w-none" value={query} onChange={setQuery} />
      <div className="mt-[22px] mb-[13px] flex flex-wrap items-center gap-[7px]">
        {FILTERS.map((f) => (
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
        <span className="ml-auto whitespace-nowrap text-[10px] text-[#999a94]">
          {results.length} 件
        </span>
      </div>
      <MemoList
        memos={results}
        status={status}
        openMemo={openMemo}
        actions={actions}
        empty={{
          icon: "search",
          title: "一致する記録がありません",
          hint: "キーワードや絞り込みを変えてみてください。",
        }}
      />
    </div>
  );
}

function matchesFilter(memo: Memo, filter: (typeof FILTERS)[number]): boolean {
  switch (filter) {
    case "すべて":
      return true;
    case "タスク":
      return memo.type === "task";
    case "メモ":
      return memo.type === "memo";
    case "アーカイブ":
      return memo.archived;
  }
}
