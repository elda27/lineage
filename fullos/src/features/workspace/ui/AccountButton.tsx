import { accountInitial, type Account } from "@core/domain/account/Account";
import { Icon } from "@/shared/ui/kit";

/** 認証のある接続のときだけ出るアカウント欄。ローカル接続では何も描画しない。 */
export function AccountButton({ account }: { account: Account | null }) {
  if (!account) return null;
  return (
    <button className="flex w-full items-center gap-[9px] border-0 border-t border-t-[#deded8] bg-transparent px-1.5 pt-[17px] text-left">
      <span className="grid h-8 w-8 place-items-center rounded-full bg-[#d9d4be] font-semibold text-[#5d5948]">
        {accountInitial(account)}
      </span>
      <span className="flex min-w-0 flex-1 flex-col">
        <b className="truncate text-[12px]">{account.displayName}</b>
        <small className="truncate text-[9px] text-[#969791]">{account.workspaceName}</small>
      </span>
      <Icon name="more" />
    </button>
  );
}
