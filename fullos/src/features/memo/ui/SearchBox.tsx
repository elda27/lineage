import { Icon } from "@/components/base";
import { useMetaCompletion } from "./MetaCompletion";

/** 検索と記録編集で共用する、minos と同じメタ情報補完付き検索バー。 */
export function SearchBox({
  value,
  onChange,
  onSubmit,
  large = false,
  className = "",
}: {
  value: string;
  onChange: (value: string) => void;
  onSubmit?: () => void;
  large?: boolean;
  className?: string;
}) {
  const completion = useMetaCompletion({ value, onChange, onEnter: onSubmit });

  return (
    <div className={`relative ${large ? "max-w-[720px]" : ""} ${className}`}>
      <div
        className={`flex items-center gap-[11px] rounded-[10px] border border-[#deded8] bg-white px-3.5 shadow-[0_1px_3px_#2f302914] focus-within:border-[#9a92cc] focus-within:shadow-[0_0_0_3px_#7165ba14] ${large ? "h-[56px]" : "h-[46px]"}`}
      >
        <Icon name="search" />
        <input
          {...completion.inputProps}
          className="min-w-0 flex-1 border-0 bg-transparent text-sm text-ink outline-none placeholder:text-[#a0a19b]"
          aria-label="メモを検索"
          placeholder="メモやタスクを検索…（# でメタ情報）"
        />
        <span className="rounded-[5px] border border-[#e1e1dc] bg-[#f8f8f5] px-[7px] py-0.5 text-[11px] text-[#a0a19b]">
          ⌘ K
        </span>
      </div>
      {completion.suggestionsElement}
    </div>
  );
}
