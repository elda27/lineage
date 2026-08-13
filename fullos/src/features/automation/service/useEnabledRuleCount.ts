import { useEffect, useState } from "react";

import { appClient } from "@/shared/api/appClient";

/**
 * 有効な自動化ルールの件数。ホームのカードに出すだけなので、
 * 読めなくても 0 として黙って続ける（一覧の表示を妨げない）。
 */
export function useEnabledRuleCount() {
  const [count, setCount] = useState(0);

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.listAutomationRules())
      .then((rules) => {
        if (active) setCount(rules.filter((rule) => rule.enabled).length);
      })
      .catch((error) => console.error("自動化ルールを数えられませんでした", error));
    return () => {
      active = false;
    };
  }, []);

  return count;
}
