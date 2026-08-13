import { useUpdater } from "./useUpdater";

const banner =
  "fixed inset-x-0 top-0 z-10 flex flex-wrap items-center justify-center gap-3 border-b px-4 py-2.5 text-[0.9em]";
const infoBanner = `${banner} border-b-[#b7d9f5] bg-[#e8f4ff] dark:border-b-[#2f4759] dark:bg-[#1d2a38] dark:text-[#f6f6f6]`;
const errorBanner = `${banner} border-b-[#f5b7b7] bg-[#ffeaea] dark:border-b-[#5e2f2f] dark:bg-[#3a2020] dark:text-[#f6f6f6]`;
const action =
  "cursor-pointer rounded-md border border-transparent bg-[#646cff] px-3.5 py-1 text-[0.95em] text-white hover:bg-[#535bf2]";
const ghostAction =
  "cursor-pointer rounded-md border border-[#b7d9f5] bg-transparent px-3.5 py-1 text-[0.95em] text-[#0f0f0f] hover:bg-[#d8ebfb] dark:border-[#2f4759] dark:text-[#f6f6f6] dark:hover:bg-[#26384a]";
const notes = "basis-full m-0 text-center whitespace-pre-wrap text-[#3a3a3a] dark:text-[#cfd8e0]";

/**
 * 更新がある時だけ出る通知バー。
 * 起動時の自動チェックで何も無ければ何も描画しない。
 */
export function UpdateBanner() {
  const { status, checkForUpdate, installUpdate, dismiss } = useUpdater();

  switch (status.kind) {
    case "idle":
    case "checking":
      return null;

    case "up-to-date":
      return (
        <div className={infoBanner}>
          <span>最新版を使用しています。</span>
          <button type="button" className={action} onClick={dismiss}>
            閉じる
          </button>
        </div>
      );

    case "available":
      return (
        <div className={infoBanner}>
          <span>
            新しいバージョン <strong>{status.version}</strong> が利用できます。
          </span>
          {status.notes && <p className={notes}>{status.notes}</p>}
          <button type="button" className={action} onClick={() => void installUpdate()}>
            更新する
          </button>
          <button type="button" className={ghostAction} onClick={dismiss}>
            あとで
          </button>
        </div>
      );

    case "downloading":
      return (
        <div className={infoBanner}>
          <span>
            {status.version} をダウンロード中
            {status.percent !== null ? ` (${status.percent}%)` : "…"}
          </span>
        </div>
      );

    case "ready":
      return (
        <div className={infoBanner}>
          <span>{status.version} を適用しました。再起動しています…</span>
        </div>
      );

    case "error":
      return (
        <div className={errorBanner}>
          <span>更新に失敗しました: {status.message}</span>
          <button type="button" className={action} onClick={() => void checkForUpdate()}>
            再試行
          </button>
          <button type="button" className={ghostAction} onClick={dismiss}>
            閉じる
          </button>
        </div>
      );
  }
}
