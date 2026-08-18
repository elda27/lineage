import { useCallback, useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/**
 * 更新の状態機械。
 *
 * 配布物は GitHub Release に置かれ、tauri.conf.json の
 * plugins.updater.endpoints が指す latest.json から取得する。
 */
export type UpdateStatus =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up-to-date" }
  | { kind: "available"; version: string; notes?: string }
  | { kind: "downloading"; version: string; percent: number | null }
  | { kind: "ready"; version: string }
  | { kind: "error"; message: string };

const messageOf = (error: unknown) => (error instanceof Error ? error.message : String(error));

// アプリを起動し続ける利用者にも、再起動を待たず GitHub Release を届ける。
const UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>({ kind: "idle" });
  // downloadAndInstall はチェック時に得た Update をそのまま使う必要がある。
  const pending = useRef<Update | null>(null);
  const checking = useRef(false);
  const installing = useRef(false);

  /**
   * 更新の有無を問い合わせる。
   *
   * `silent` は起動直後の自動チェック用。ネットワーク断や Tauri 外
   * (ブラウザでの vite dev) での失敗をユーザーに見せない。
   */
  const checkForUpdate = useCallback(async (silent = false) => {
    if (checking.current || installing.current) return;
    checking.current = true;
    if (!silent) setStatus({ kind: "checking" });
    try {
      const update = await check();
      pending.current = update;
      if (!update) {
        setStatus(silent ? { kind: "idle" } : { kind: "up-to-date" });
        return;
      }
      setStatus({
        kind: "available",
        version: update.version,
        notes: update.body,
      });
    } catch (error) {
      console.error("update check failed", error);
      if (!silent) setStatus({ kind: "error", message: messageOf(error) });
    } finally {
      checking.current = false;
    }
  }, []);

  /** ダウンロードしてインストールし、完了後にアプリを再起動する。 */
  const installUpdate = useCallback(async () => {
    const update = pending.current;
    if (!update || installing.current) return;
    installing.current = true;

    const version = update.version;
    setStatus({ kind: "downloading", version, percent: null });
    try {
      let contentLength = 0;
      let downloaded = 0;
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            contentLength = event.data.contentLength ?? 0;
            setStatus({ kind: "downloading", version, percent: contentLength ? 0 : null });
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            setStatus({
              kind: "downloading",
              version,
              // Content-Length が無い配信もあるので、その場合は不定表示にする。
              percent: contentLength
                ? Math.min(100, Math.round((downloaded / contentLength) * 100))
                : null,
            });
            break;
          case "Finished":
            setStatus({ kind: "ready", version });
            break;
        }
      });
      setStatus({ kind: "ready", version });
      await relaunch();
    } catch (error) {
      console.error("update install failed", error);
      setStatus({ kind: "error", message: messageOf(error) });
    } finally {
      installing.current = false;
    }
  }, []);

  const dismiss = useCallback(() => setStatus({ kind: "idle" }), []);

  // 起動時に一度だけ黙って確認する。StrictMode の二重実行は ref で防ぐ。
  // その後も定期的に確認し、オフラインから戻った時にはすぐ再確認する。
  const checkedOnMount = useRef(false);
  useEffect(() => {
    if (checkedOnMount.current) return;
    checkedOnMount.current = true;
    void checkForUpdate(true);

    const interval = window.setInterval(() => {
      void checkForUpdate(true);
    }, UPDATE_CHECK_INTERVAL_MS);
    const checkWhenOnline = () => void checkForUpdate(true);
    window.addEventListener("online", checkWhenOnline);

    return () => {
      window.clearInterval(interval);
      window.removeEventListener("online", checkWhenOnline);
    };
  }, [checkForUpdate]);

  return { status, checkForUpdate, installUpdate, dismiss };
}
