import { useRef, useState } from "react";

import { findActiveTagToken, type MetaSuggestion, type TagToken } from "@core/domain/meta/MetaTag";
import { Icon } from "@/shared/ui/kit";
import { useMetaSuggestions } from "../service/useMetaSuggestions";

/**
 * 補完対象の見直しが要るキャレット移動キー。
 * 上下は候補の選択に使うので、ここには入れない（選択位置が戻ってしまう）。
 */
const CARET_KEYS = ["ArrowLeft", "ArrowRight", "Home", "End"];

/**
 * 検索バー。`#` に続けて打つとメタ情報を補完し、確定した `#タグ` はそのまま絞り込み条件になる
 * （docs/ui.md「検索画面」）。並び順と一致規則は minos の入力補完と同じ。
 */
export function SearchBox({
  value,
  onChange,
  onSubmit,
  large = false,
  className = "",
}: {
  value: string;
  onChange: (v: string) => void;
  onSubmit?: () => void;
  large?: boolean;
  className?: string;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [token, setToken] = useState<TagToken | null>(null);
  const [highlight, setHighlight] = useState(0);
  const suggestions = useMetaSuggestions(token?.query ?? null);
  const open = token !== null && suggestions.length > 0;
  // 候補は非同期に届くので、入力が進んで件数が減っても選択位置がはみ出さないようにする。
  const selected = Math.min(highlight, suggestions.length - 1);

  // カーソルが `#` トークンの中にいるかは、入力とカーソル移動のたびに見直す。
  const syncToken = (el: HTMLInputElement) => {
    setToken(findActiveTagToken(el.value.slice(0, el.selectionStart ?? el.value.length)));
    setHighlight(0);
  };

  const accept = (suggestion: MetaSuggestion) => {
    const el = input.current;
    if (!el || !token) return;
    // 置き換えるのは `#` からカーソルまで。`#タス` が `#タスク ` になる。
    const inserted = `#${suggestion.label} `;
    const caret = token.start + inserted.length;
    onChange(
      `${value.slice(0, token.start)}${inserted}${value.slice(el.selectionStart ?? value.length)}`,
    );
    setToken(null);
    // 制御コンポーネントなので、値が反映される描画を1回待ってからキャレットを置き直す。
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(caret, caret);
    });
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    // IME 変換中の Enter は変換の確定なので、補完にも検索にも使わない。
    if (e.nativeEvent.isComposing) return;
    if (open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        setHighlight(
          (h) =>
            (Math.min(h, suggestions.length - 1) +
              (e.key === "ArrowDown" ? 1 : suggestions.length - 1)) %
            suggestions.length,
        );
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        accept(suggestions[selected]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setToken(null);
        return;
      }
    }
    if (e.key === "Enter") onSubmit?.();
  };

  return (
    <div className={`relative ${large ? "max-w-[720px]" : ""} ${className}`}>
      <div
        className={`flex items-center gap-[11px] rounded-[10px] border border-[#deded8] bg-white px-3.5 shadow-[0_1px_3px_#2f302914] focus-within:border-[#9a92cc] focus-within:shadow-[0_0_0_3px_#7165ba14] ${large ? "h-[56px]" : "h-[46px]"}`}
      >
        <Icon name="search" />
        <input
          ref={input}
          className="min-w-0 flex-1 border-0 bg-transparent text-sm text-ink outline-none placeholder:text-[#a0a19b]"
          aria-label="メモを検索"
          role="combobox"
          aria-expanded={open}
          aria-controls="meta-suggestions"
          aria-autocomplete="list"
          value={value}
          onChange={(e) => {
            onChange(e.target.value);
            syncToken(e.target);
          }}
          onClick={(e) => syncToken(e.currentTarget)}
          onKeyUp={(e) => {
            if (CARET_KEYS.includes(e.key)) syncToken(e.currentTarget);
          }}
          onBlur={() => setToken(null)}
          onKeyDown={onKeyDown}
          placeholder="メモやタスクを検索…（# でメタ情報）"
        />
        <span className="rounded-[5px] border border-[#e1e1dc] bg-[#f8f8f5] px-[7px] py-0.5 text-[11px] text-[#a0a19b]">
          ⌘ K
        </span>
      </div>
      {/* onMouseDown を止めないと、クリックより先に blur が起きて候補が消える。 */}
      {open && (
        <ul
          id="meta-suggestions"
          role="listbox"
          aria-label="メタ情報の候補"
          className="absolute z-10 mt-1.5 w-full max-w-[420px] overflow-hidden rounded-[10px] border border-line bg-white py-1 shadow-[0_10px_30px_#2f302920]"
          onMouseDown={(e) => e.preventDefault()}
        >
          {suggestions.map((suggestion, index) => (
            <li
              key={suggestion.label}
              role="option"
              aria-selected={index === selected}
              className={`flex cursor-pointer items-center gap-2.5 px-3 py-2 text-[12px] ${index === selected ? "bg-[#f1f0ed]" : ""}`}
              onMouseEnter={() => setHighlight(index)}
              onClick={() => accept(suggestion)}
            >
              <Icon name="tag" size={14} />
              <b className="font-medium">#{suggestion.label}</b>
              {suggestion.shorthand && (
                <span className="font-mono text-[10px] text-[#8f918a]">{suggestion.shorthand}</span>
              )}
              <small className="ml-auto text-[10px] text-[#a0a19c]">
                {suggestion.usageCount}回{suggestion.matched === "shorthandPrefix" && " · 短縮"}
              </small>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
