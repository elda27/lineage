import { useEffect, useState } from "react";

import { appClient } from "@/shared/api/appClient";
import { toView, type LoadState, type Memo } from "./memoView";

/** minos が書いたローカル DB から記録を読み込む。 */
export function useRecordedMemos() {
  const [memos, setMemos] = useState<Memo[]>([]);
  const [status, setStatus] = useState<LoadState>("loading");

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.listMemos())
      .then((records) => {
        if (!active) return;
        setMemos(records.map(toView));
        setStatus("ready");
      })
      .catch((error) => {
        if (!active) return;
        console.error("記録を読み込めませんでした", error);
        setStatus("error");
      });
    return () => {
      active = false;
    };
  }, []);

  return { memos, setMemos, status };
}
