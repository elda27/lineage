import { useUpdater } from "./useUpdater";
import "./UpdateBanner.css";

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
        <div className="update-banner">
          <span>最新版を使用しています。</span>
          <button type="button" onClick={dismiss}>
            閉じる
          </button>
        </div>
      );

    case "available":
      return (
        <div className="update-banner">
          <span>
            新しいバージョン <strong>{status.version}</strong> が利用できます。
          </span>
          {status.notes && <p className="update-banner__notes">{status.notes}</p>}
          <button type="button" onClick={() => void installUpdate()}>
            更新する
          </button>
          <button type="button" className="update-banner__ghost" onClick={dismiss}>
            あとで
          </button>
        </div>
      );

    case "downloading":
      return (
        <div className="update-banner">
          <span>
            {status.version} をダウンロード中
            {status.percent !== null ? ` (${status.percent}%)` : "…"}
          </span>
        </div>
      );

    case "ready":
      return (
        <div className="update-banner">
          <span>{status.version} を適用しました。再起動しています…</span>
        </div>
      );

    case "error":
      return (
        <div className="update-banner update-banner--error">
          <span>更新に失敗しました: {status.message}</span>
          <button type="button" onClick={() => void checkForUpdate()}>
            再試行
          </button>
          <button type="button" className="update-banner__ghost" onClick={dismiss}>
            閉じる
          </button>
        </div>
      );
  }
}
