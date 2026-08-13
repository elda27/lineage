import type { Account } from "@core/domain/account/Account";
import { AccountButton } from "@/features/workspace/ui/AccountButton";
import { StorageMeter } from "@/features/workspace/ui/StorageMeter";
import type { Page } from "@/shared/navigation";
import { Icon, type IconName } from "@/shared/ui/kit";

export function Sidebar({
  page,
  setPage,
  account,
}: {
  page: Page;
  setPage: (p: Page) => void;
  account: Account | null;
}) {
  const items: { page: Page; label: string; icon: IconName }[] = [
    { page: "home", label: "ホーム", icon: "home" },
    { page: "search", label: "検索", icon: "search" },
    { page: "automation", label: "自動化", icon: "sparkles" },
    { page: "settings", label: "設定", icon: "settings" },
  ];
  const navItem =
    "flex h-[42px] items-center gap-3 rounded-lg border-0 px-[11px] text-left cursor-pointer";
  return (
    <aside className="fixed inset-y-0 left-0 z-[5] flex w-[226px] flex-col border-r border-[#e3e3dc] bg-[#f1f1ed] px-[15px] pt-6 pb-[18px]">
      <button
        className="flex cursor-pointer items-center gap-2.5 border-0 bg-transparent px-2 pt-[3px] pb-[30px] text-[18px] font-semibold"
        onClick={() => setPage("home")}
      >
        <span className="grid h-[29px] w-[29px] place-items-center rounded-[9px] bg-[#302e35] font-serif text-[17px] text-white">
          L
        </span>
        <span>lineage</span>
      </button>
      <nav className="flex flex-col gap-1">
        {items.map((item) => (
          <button
            key={item.page}
            className={`${navItem} ${page === item.page ? "bg-white font-semibold text-ink shadow-[0_1px_2px_#00000008]" : "bg-transparent text-[#6f716b] hover:bg-[#e9e9e4] hover:text-ink"}`}
            onClick={() => setPage(item.page)}
          >
            <Icon name={item.icon} />
            <span>{item.label}</span>
            {item.page === "search" && (
              <kbd className="ml-auto font-sans text-[11px] text-[#a0a19c]">⌘ K</kbd>
            )}
          </button>
        ))}
      </nav>
      <div className="mt-auto">
        <StorageMeter />
        <AccountButton account={account} />
      </div>
    </aside>
  );
}
