import { useEffect, useState } from "react";

import type { MetaCondition } from "../../core/domain/automation/AutomationRule";
import type { MetaSuggestion } from "../../core/domain/meta/MetaTag";
import { appClient } from "../app-client/appClient";
import { Icon, tagChip } from "../ui";

/** 一度に出す候補の上限（minos の MAX_SUGGESTIONS と揃える）。 */
const MAX_SUGGESTIONS = 8;

/**
 * 対象を絞り込むメタ情報の入力。
 *
 * 候補は minos が学習した `meta_tags` から引く。自由入力にすると、実在しない
 * ラベルを条件にしてしまい「一致する記録がいつまでも現れない」ルールができる。
 */
export function MetaConditionInput({
  conditions,
  onChange,
}: {
  conditions: MetaCondition[];
  onChange: (conditions: MetaCondition[]) => void;
}) {
  const [query, setQuery] = useState("");
  const [value, setValue] = useState("");
  const [suggestions, setSuggestions] = useState<MetaSuggestion[]>([]);

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.suggestMetaTags(query, MAX_SUGGESTIONS))
      .then((found) => {
        if (active) setSuggestions(found);
      })
      .catch((error) => {
        console.error("メタ情報の候補を取得できませんでした", error);
        if (active) setSuggestions([]);
      });
    return () => {
      active = false;
    };
  }, [query]);

  const add = (label: string) => {
    // 同じラベルを二重に足しても絞り込みは変わらないので、既にあれば何もしない。
    if (conditions.some((condition) => condition.label === label)) return;
    onChange([...conditions, { label, value: value.trim() || null }]);
    setQuery("");
    setValue("");
  };

  const remove = (label: string) =>
    onChange(conditions.filter((condition) => condition.label !== label));

  return (
    <div className="flex flex-col gap-2">
      {conditions.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {conditions.map((condition) => (
            <span
              key={condition.label}
              className={`${tagChip} items-center gap-1 py-1 text-[10px]`}
            >
              #{condition.label}
              {condition.value && `=${condition.value}`}
              <button
                type="button"
                aria-label={`${condition.label} を条件から外す`}
                className="cursor-pointer border-0 bg-transparent p-0 text-[#a0a19c] hover:text-ink"
                onClick={() => remove(condition.label)}
              >
                <Icon name="trash" size={11} />
              </button>
            </span>
          ))}
        </div>
      )}

      <div className="flex gap-2">
        <input
          className="min-w-0 flex-1 rounded-lg border border-[#deded8] px-3 py-2 text-[12px] outline-none focus:border-[#9a92cc]"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="メタ情報を検索（例: タスク）"
          aria-label="条件に足すメタ情報"
        />
        <input
          className="w-[140px] rounded-lg border border-[#deded8] px-3 py-2 text-[12px] outline-none focus:border-[#9a92cc]"
          value={value}
          onChange={(event) => setValue(event.target.value)}
          placeholder="値（省略可）"
          aria-label="メタ情報の値"
        />
      </div>

      {suggestions.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {suggestions.map((suggestion) => (
            <button
              type="button"
              key={suggestion.label}
              className={`${tagChip} cursor-pointer hover:bg-[#e5e4e0]`}
              onClick={() => add(suggestion.label)}
            >
              #{suggestion.label}
              <small className="ml-1 text-[#a0a19c]">{suggestion.usageCount}</small>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
