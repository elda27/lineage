/**
 * 画面のシェル（composition root）。
 *
 * route-level screen は pages/、feature-local UI は features/<機能>/ui が持つ。ここがするのは
 * 「どの画面を出すか」と「画面をまたいで持つ状態（記録の一覧と選択中の記録）」の管理だけ。
 */

import { useState } from "react";

import { AutomationPage } from "@/pages/AutomationPage";
import { useEnabledRuleCount } from "@/features/automation/service/useEnabledRuleCount";
import { HomePage } from "@/pages/HomePage";
import { MemoDetail } from "@/features/memo/ui/MemoDetail";
import { SearchPage } from "@/pages/SearchPage";
import type { Memo } from "@/features/memo/service/memoView";
import { useArchiveOnClose } from "@/features/memo/service/useArchiveOnClose";
import { useMemoActions } from "@/features/memo/service/useMemoActions";
import { useRecordedMemos } from "@/features/memo/service/useRecordedMemos";
import { SettingsPage } from "@/pages/SettingsPage";
import { AgentSkillDialog } from "@/features/skill/ui/AgentSkillDialog";
import { TagExplorerPage } from "@/pages/TagExplorerPage";
import { useAccount } from "@/features/workspace/service/useAccount";
import type { Page } from "@/shared/navigation";
import { Sidebar } from "./Sidebar";

export default function App() {
  const [page, setPage] = useState<Page>("home"),
    [selected, setSelected] = useState<Memo | null>(null);
  const [query, setQuery] = useState("");
  const { memos, setMemos, status } = useRecordedMemos();
  const enabledRuleCount = useEnabledRuleCount();
  // 組み込みタグの操作（完了・アーカイブ・ゴミ箱）。一覧と詳細の両方から呼ぶ。
  const { actions, error, dismissError } = useMemoActions(setMemos);
  // 完了したタスクは、閉じるときにまとめてアーカイブする。
  useArchiveOnClose();
  // アカウントはサイドバーとホームの両方が使うのでここで1回だけ読む。
  const account = useAccount();
  // 詳細を開いたまま一覧側の状態が変わることがあるので、常に一覧の最新を見せる。
  const detail = selected && (memos.find((m) => m.id === selected.id) ?? selected);
  const create = () =>
    setSelected({
      id: `draft-${Date.now()}`,
      title: "無題のメモ",
      body: "ここに内容を入力してください。",
      preview: "",
      metas: [],
      images: [],
      createdAt: new Date().toISOString(),
      type: "memo",
      capabilities: [],
      done: false,
      archived: false,
    });
  return (
    <div className="min-h-screen">
      <AgentSkillDialog />
      <Sidebar page={page} setPage={setPage} account={account} />
      <main className="min-h-screen min-w-0 pb-[76px] lg:ml-[226px] lg:w-[calc(100%-226px)] lg:pb-0">
        {page === "home" && (
          <HomePage
            memos={memos}
            status={status}
            query={query}
            setQuery={setQuery}
            setPage={setPage}
            openMemo={setSelected}
            actions={actions}
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
            actions={actions}
          />
        )}{" "}
        {page === "automation" && <AutomationPage />} {page === "settings" && <SettingsPage />}
        {page === "tags" && <TagExplorerPage />}
      </main>
      {detail && (
        <MemoDetail
          memo={detail}
          close={() => setSelected(null)}
          update={(m) => {
            setMemos((v) =>
              v.some((x) => x.id === m.id) ? v.map((x) => (x.id === m.id ? m : x)) : [m, ...v],
            );
            setSelected(m);
          }}
          actions={actions}
        />
      )}
      {/* 状態の保存に失敗したときだけ出る。押し間違いではなく DB 側の事情なので、
          何をすれば直るかが分かる文言をそのまま見せる。 */}
      {error && (
        <div className="fixed right-4 bottom-[84px] left-4 z-30 flex items-center gap-3 rounded-[10px] border border-[#e0c9c9] bg-white px-4 py-3 text-[12px] text-[#8a4a4a] shadow-[0_8px_24px_#2f302920] sm:right-auto sm:left-1/2 sm:w-[min(560px,calc(100vw-32px))] sm:-translate-x-1/2 lg:bottom-5">
          <span className="flex-1">{error}</span>
          <button
            className="cursor-pointer border-0 bg-transparent text-[11px] text-[#777972]"
            onClick={dismissError}
          >
            閉じる
          </button>
        </div>
      )}
    </div>
  );
}
