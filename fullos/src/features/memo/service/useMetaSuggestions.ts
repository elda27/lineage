import { useEffect, useState } from "react";

import type { MetaSuggestion } from "@core/domain/meta/MetaTag";
import { appClient } from "@/shared/api/appClient";

/** 一度に出す補完候補の上限（minos の MAX_SUGGESTIONS と揃える）。 */
const MAX_SUGGESTIONS = 12;

/**
 * `#` トークンを打っている間だけ、学習済みメタ情報の候補を読む。
 *
 * `query` が null なら補完は閉じている。候補が取れなくても検索自体は続けられるので、
 * 失敗しても黙って候補なしにする。
 */
export function useMetaSuggestions(query: string | null) {
  const [suggestions, setSuggestions] = useState<MetaSuggestion[]>([]);

  useEffect(() => {
    if (query === null) {
      setSuggestions([]);
      return;
    }
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

  return suggestions;
}
