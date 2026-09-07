import { useId, useRef, useState } from "react";

import { findActiveTagToken, type MetaSuggestion, type TagToken } from "@core/domain/meta/MetaTag";
import { Icon } from "@/shared/ui/kit";
import { useMetaSuggestions } from "../service/useMetaSuggestions";

type Editor = HTMLInputElement | HTMLTextAreaElement;

const CARET_KEYS = ["ArrowLeft", "ArrowRight", "Home", "End"];

/**
 * minos の入力欄と同じメタ情報補完を、fullos の検索・編集欄へ接続する。
 * 入力要素の見た目は呼び出し側が所有し、トークン認識・キー操作・候補表示はここだけで管理する。
 */
export function useMetaCompletion({
  value,
  onChange,
  onEnter,
}: {
  value: string;
  onChange: (value: string) => void;
  onEnter?: () => void;
}) {
  const ref = useRef<Editor>(null);
  const listId = useId();
  const [token, setToken] = useState<TagToken | null>(null);
  const [highlight, setHighlight] = useState(0);
  const suggestions = useMetaSuggestions(token?.query ?? null);
  const open = token !== null && suggestions.length > 0;
  const selected = Math.min(highlight, suggestions.length - 1);

  const syncToken = (editor: Editor) => {
    setToken(
      findActiveTagToken(editor.value.slice(0, editor.selectionStart ?? editor.value.length)),
    );
    setHighlight(0);
  };

  const accept = (suggestion: MetaSuggestion) => {
    const editor = ref.current;
    if (!editor || !token) return;
    const inserted = `#${suggestion.label} `;
    const caret = token.start + inserted.length;
    onChange(
      `${value.slice(0, token.start)}${inserted}${value.slice(editor.selectionStart ?? value.length)}`,
    );
    setToken(null);
    requestAnimationFrame(() => {
      editor.focus();
      editor.setSelectionRange(caret, caret);
    });
  };

  const inputProps = {
    ref: (editor: Editor | null) => {
      ref.current = editor;
    },
    role: "combobox" as const,
    "aria-expanded": open,
    "aria-controls": listId,
    "aria-autocomplete": "list" as const,
    value,
    onChange: (event: React.ChangeEvent<Editor>) => {
      onChange(event.target.value);
      syncToken(event.target);
    },
    onClick: (event: React.MouseEvent<Editor>) => syncToken(event.currentTarget),
    onKeyUp: (event: React.KeyboardEvent<Editor>) => {
      if (CARET_KEYS.includes(event.key)) syncToken(event.currentTarget);
    },
    onBlur: () => setToken(null),
    onKeyDown: (event: React.KeyboardEvent<Editor>) => {
      if (event.nativeEvent.isComposing) return;
      if (open) {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          setHighlight(
            (current) =>
              (Math.min(current, suggestions.length - 1) +
                (event.key === "ArrowDown" ? 1 : suggestions.length - 1)) %
              suggestions.length,
          );
          return;
        }
        if (event.key === "Enter" || event.key === "Tab") {
          event.preventDefault();
          accept(suggestions[selected]);
          return;
        }
        if (event.key === "Escape") {
          event.preventDefault();
          setToken(null);
          return;
        }
      }
      if (event.key === "Enter") onEnter?.();
    },
  };

  const suggestionsElement = open ? (
    <ul
      id={listId}
      role="listbox"
      aria-label="メタ情報の候補"
      className="absolute z-30 mt-1.5 w-full max-w-[420px] overflow-hidden rounded-[10px] border border-line bg-white py-1 shadow-[0_10px_30px_#2f302920]"
      onMouseDown={(event) => event.preventDefault()}
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
  ) : null;

  return { inputProps, suggestionsElement };
}
