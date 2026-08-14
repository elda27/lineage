import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { appClient } from "@/shared/api/appClient";

/**
 * 完了したタスクを、fullos を閉じるときにアーカイブする
 * （組み込みタグ `#タスク` の `complete` 機能。docs/ui.md「組み込みタグ」）。
 *
 * チェックした瞬間に消さないのは、開いている間は押し間違いを取り消せるようにするため。
 * 閉じる要求は一度止めて、書き込みを終えてからウィンドウを破棄する。止めずに走らせると
 * プロセスの終了が書き込みに間に合わない。
 */
export function useArchiveOnClose() {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    let closing = false;

    const register = async () => {
      const appWindow = getCurrentWindow();
      const stop = await appWindow.onCloseRequested(async (event) => {
        // destroy() は close-requested を投げ直さないが、閉じる操作が重なっても
        // アーカイブを二度走らせない。
        if (closing) return;
        closing = true;
        event.preventDefault();
        try {
          await (await appClient()).archiveCompletedTasks();
        } catch (error) {
          // 閉じる操作は止めない。アーカイブは次に閉じるときにやり直せる。
          console.error("完了したタスクをアーカイブできませんでした", error);
        }
        await appWindow.destroy();
      });
      if (disposed) stop();
      else unlisten = stop;
    };

    // Tauri の外（ブラウザでの vite dev）では窓を取れない。その場合は何もしない。
    register().catch((error) => console.error("閉じるときの処理を登録できませんでした", error));

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
