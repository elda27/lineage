import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

import { useUpdater } from "@/features/updater/service/useUpdater";
import { SettingRow } from "@/components/base";

const action =
  "shrink-0 cursor-pointer rounded-[7px] border border-line bg-white px-3 py-2 text-[10px] font-medium text-ink shadow-[0_1px_2px_#0000000a] hover:bg-[#f7f7f3] disabled:cursor-wait disabled:opacity-60";
const primaryAction = `${action} border-accent bg-accent text-white hover:bg-[#5b7044]`;

/** アプリケーションバージョンと手動更新を設定項目として表示する。 */
export function LineageUpdateSettings() {
  const [version, setVersion] = useState<string | null>(null);
  const { status, checkForUpdate, installUpdate, dismiss } = useUpdater();

  useEffect(() => {
    void getVersion()
      .then(setVersion)
      .catch((error) => {
        console.error("failed to read app version", error);
        setVersion("不明");
      });
  }, []);

  const checking = status.kind === "checking";
  const message = (() => {
    switch (status.kind) {
      case "idle":
        return null;
      case "checking":
        return "新しいバージョンがないか確認しています…";
      case "up-to-date":
        return "最新版を使用しています。";
      case "available":
        return `新しいバージョン ${status.version} が利用できます。`;
      case "downloading":
        return `${status.version} をダウンロード中${status.percent !== null ? ` (${status.percent}%)` : "…"}`;
      case "ready":
        return `${status.version} を適用しました。再起動しています…`;
      case "error":
        return `更新に失敗しました: ${status.message}`;
    }
  })();

  return (
    <SettingRow
      title="アプリケーションバージョン"
      desc={version === null ? "読み込み中…" : `バージョン ${version}`}
    >
      <div className="ml-4 flex max-w-[60%] flex-col items-end gap-2">
        <div className="flex flex-wrap justify-end gap-2">
          <button
            type="button"
            className={action}
            disabled={checking || status.kind === "downloading" || status.kind === "ready"}
            onClick={() => void checkForUpdate()}
          >
            {checking ? "確認中…" : status.kind === "error" ? "再試行" : "更新を確認"}
          </button>
          {status.kind === "available" && (
            <button type="button" className={primaryAction} onClick={() => void installUpdate()}>
              更新する
            </button>
          )}
        </div>
        {message && (
          <div
            className={`flex items-start gap-2 text-right text-[9px] ${status.kind === "error" ? "text-[#8a4a4a]" : "text-muted"}`}
          >
            <p className="min-w-0 flex-1 whitespace-pre-wrap">{message}</p>
            {(status.kind === "up-to-date" ||
              status.kind === "available" ||
              status.kind === "error") && (
              <button
                type="button"
                className="cursor-pointer border-0 bg-transparent text-[9px] text-muted"
                onClick={dismiss}
              >
                閉じる
              </button>
            )}
          </div>
        )}
        {status.kind === "available" && status.notes && (
          <p className="whitespace-pre-wrap text-right text-[9px] text-muted">{status.notes}</p>
        )}
      </div>
    </SettingRow>
  );
}
