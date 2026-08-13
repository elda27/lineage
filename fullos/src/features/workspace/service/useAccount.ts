import { useEffect, useState } from "react";

import type { Account } from "@core/domain/account/Account";
import { appClient } from "@/shared/api/appClient";

/**
 * ログイン中のアカウント。認証を持たない接続（ローカル接続）では null のまま。
 *
 * 取得に失敗したときも null にして、アカウント欄を黙って隠す
 * （記録一覧と違い、無くても操作の妨げにならないため）。
 */
export function useAccount() {
  const [account, setAccount] = useState<Account | null>(null);

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.currentAccount())
      .then((value) => {
        if (active) setAccount(value);
      })
      .catch((error) => console.error("アカウントを取得できませんでした", error));
    return () => {
      active = false;
    };
  }, []);

  return account;
}
