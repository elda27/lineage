import { useCallback, useMemo, useState, type Dispatch, type SetStateAction } from "react";

import { appClient } from "@/shared/api/appClient";
import type { Memo, MemoActions } from "./memoView";

/** まだ保存されていない下書きの id（app/App.tsx が採番する）。 */
const DRAFT_PREFIX = "draft-";

/**
 * 組み込みタグの操作（完了・アーカイブ・ゴミ箱）を DB へ反映する。
 *
 * 画面の見た目を先に更新してから書き込み、失敗したら元へ戻して理由を出す。
 * 書き込みは1行の状態更新で、失敗しても記録そのものは壊れない。
 */
export function useMemoActions(setMemos: Dispatch<SetStateAction<Memo[]>>) {
  const [error, setError] = useState<string | null>(null);

  const patch = useCallback(
    (id: string, change: Partial<Memo>) =>
      setMemos((memos) => memos.map((m) => (m.id === id ? { ...m, ...change } : m))),
    [setMemos],
  );

  const drop = useCallback(
    (id: string) => setMemos((memos) => memos.filter((m) => m.id !== id)),
    [setMemos],
  );

  /** 見た目を先に変え、書き込みに失敗したら戻す。 */
  const apply = useCallback(
    async (
      memo: Memo,
      optimistic: () => void,
      rollback: () => void,
      write: (client: Awaited<ReturnType<typeof appClient>>) => Promise<void>,
    ) => {
      optimistic();
      // 下書きはまだ DB に無いので、画面の上だけで完結させる。
      if (memo.id.startsWith(DRAFT_PREFIX)) return;
      try {
        setError(null);
        await write(await appClient());
      } catch (cause) {
        console.error("記録の状態を保存できませんでした", cause);
        rollback();
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [],
  );

  const actions: MemoActions = useMemo(
    () => ({
      toggleDone: (memo) =>
        void apply(
          memo,
          () => patch(memo.id, { done: !memo.done }),
          () => patch(memo.id, { done: memo.done }),
          (client) => client.setMemoDone(memo.id, !memo.done),
        ),

      setArchived: (memo, archived) =>
        void apply(
          memo,
          () => patch(memo.id, { archived }),
          () => patch(memo.id, { archived: memo.archived }),
          (client) => client.setMemoArchived(memo.id, archived),
        ),

      // ゴミ箱は一覧から消えるので、戻すときは元の位置ではなく先頭に戻す。
      // 失敗自体がまれで、順序は次回の読み込みで正しくなる。
      trash: (memo) =>
        void apply(
          memo,
          () => drop(memo.id),
          () => setMemos((memos) => [memo, ...memos]),
          (client) => client.trashMemo(memo.id),
        ),
    }),
    [apply, drop, patch, setMemos],
  );

  return { actions, error, dismissError: () => setError(null) };
}
