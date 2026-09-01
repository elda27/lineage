import type { Account } from "@core/domain/account/Account";
import { AccountButton } from "@/features/workspace/ui/AccountButton";
import { StorageMeter } from "@/features/workspace/ui/StorageMeter";
import type { Page } from "@/shared/navigation";
import { Icon, type IconName } from "@/components/base";

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
    { page: "tags", label: "タグ", icon: "tag" },
    { page: "automation", label: "自動化", icon: "sparkles" },
    { page: "settings", label: "設定", icon: "settings" },
  ];
  const navItem =
    "flex min-w-0 flex-1 flex-col items-center justify-center gap-1 rounded-lg border-0 px-1 py-2 text-center text-[10px] cursor-pointer lg:h-[42px] lg:flex-none lg:flex-row lg:justify-start lg:gap-3 lg:px-[11px] lg:py-0 lg:text-left lg:text-[13px]";
  return (
    <aside className="fixed inset-x-0 bottom-0 z-[15] flex h-[68px] border-t border-[#e3e3dc] bg-[#f1f1ed] px-2 pb-[max(4px,env(safe-area-inset-bottom))] lg:inset-y-0 lg:right-auto lg:h-auto lg:w-[226px] lg:flex-col lg:border-t-0 lg:border-r lg:px-[15px] lg:pt-6 lg:pb-[18px]">
      <button
        className="hidden cursor-pointer items-center gap-2.5 border-0 bg-transparent px-2 pt-[3px] pb-[30px] text-[18px] font-semibold lg:flex"
        onClick={() => setPage("home")}
      >
        <span className="grid h-[29px] w-[29px] place-items-center rounded-[9px] bg-[#302e35] font-serif text-[17px] text-white">
          L
        </span>
        <span>lineage</span>
      </button>
      <nav className="flex min-w-0 flex-1 gap-1 lg:flex-col">
        {items.map((item) => (
          <button
            key={item.page}
            className={`${navItem} ${page === item.page ? "bg-white font-semibold text-ink shadow-[0_1px_2px_#00000008]" : "bg-transparent text-[#6f716b] hover:bg-[#e9e9e4] hover:text-ink"}`}
            onClick={() => setPage(item.page)}
          >
            <Icon name={item.icon} />
            <span>{item.label}</span>
            {item.page === "search" && (
              <kbd className="ml-auto hidden font-sans text-[11px] text-[#a0a19c] lg:block">
                ⌘ K
              </kbd>
            )}
          </button>
        ))}
      </nav>
      <div className="mt-auto hidden lg:block">
        <StorageMeter />
        <AccountButton account={account} />
      </div>
    </aside>
  );
}
