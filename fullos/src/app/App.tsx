/**
 * 画面のシェル（composition root）。
 *
 * 画面そのものは features/<機能>/ui が持つ。ここがするのは
 * 「どの画面を出すか」と「画面をまたいで持つ状態（記録の一覧と選択中の記録）」の管理だけ。
 */

import { useState } from "react";

import { AutomationPage } from "@/features/automation/ui/AutomationPage";
import { useEnabledRuleCount } from "@/features/automation/service/useEnabledRuleCount";
import { HomePage } from "@/features/memo/ui/HomePage";
import { MemoDetail } from "@/features/memo/ui/MemoDetail";
import { SearchPage } from "@/features/memo/ui/SearchPage";
import type { Memo } from "@/features/memo/service/memoView";
import { useRecordedMemos } from "@/features/memo/service/useRecordedMemos";
import { SettingsPage } from "@/features/settings/ui/SettingsPage";
import { UpdateBanner } from "@/features/updater/ui/UpdateBanner";
import { useAccount } from "@/features/workspace/service/useAccount";
import type { Page } from "@/shared/navigation";
import { Sidebar } from "./Sidebar";

export default function App() {
  const [page, setPage] = useState<Page>("home"),
    [selected, setSelected] = useState<Memo | null>(null);
  const [query, setQuery] = useState("");
  const { memos, setMemos, status } = useRecordedMemos();
  const enabledRuleCount = useEnabledRuleCount();
  // アカウントはサイドバーとホームの両方が使うのでここで1回だけ読む。
  const account = useAccount();
  const toggle = (id: string) =>
    setMemos((m) => m.map((v) => (v.id === id ? { ...v, done: !v.done } : v)));
  const create = () =>
    setSelected({
      id: `draft-${Date.now()}`,
      title: "無題のメモ",
      body: "ここに内容を入力してください。",
      preview: "",
      metas: [],
      createdAt: new Date().toISOString(),
      type: "memo",
    });
  return (
    <div className="flex min-h-screen">
      <UpdateBanner />
      <Sidebar page={page} setPage={setPage} account={account} />
      <main className="ml-[226px] min-h-screen w-[calc(100%-226px)]">
        {page === "home" && (
          <HomePage
            memos={memos}
            status={status}
            query={query}
            setQuery={setQuery}
            setPage={setPage}
            openMemo={setSelected}
            toggleMemo={toggle}
            createMemo={create}
            enabledRuleCount={enabledRuleCount}
          />
        )}{" "}
        {page === "search" && (
          <SearchPage
            memos={memos}
            status={status}
            query={query}
            setQuery={setQuery}
            openMemo={setSelected}
            toggleMemo={toggle}
          />
        )}{" "}
        {page === "automation" && <AutomationPage />} {page === "settings" && <SettingsPage />}
      </main>
      {selected && (
        <MemoDetail
          memo={selected}
          close={() => setSelected(null)}
          update={(m) => {
            setMemos((v) =>
              v.some((x) => x.id === m.id) ? v.map((x) => (x.id === m.id ? m : x)) : [m, ...v],
            );
            setSelected(m);
          }}
          remove={() => {
            setMemos((v) => v.filter((x) => x.id !== selected.id));
            setSelected(null);
          }}
        />
      )}
    </div>
  );
}
