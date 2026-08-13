import { useEffect, useState } from "react";

import type { StorageUsage } from "@core/domain/storage/StorageUsage";
import { appClient } from "@/shared/api/appClient";

/**
 * ストレージ使用量。割り当て上限を持たない接続（ローカル接続）では null のまま。
 *
 * 取得に失敗したときも null にして、メーターを黙って隠す
 * （記録一覧と違い、無くても操作の妨げにならないため）。
 */
export function useStorageUsage() {
  const [usage, setUsage] = useState<StorageUsage | null>(null);

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.storageUsage())
      .then((value) => {
        if (active) setUsage(value);
      })
      .catch((error) => console.error("ストレージ使用量を取得できませんでした", error));
    return () => {
      active = false;
    };
  }, []);

  return usage;
}
