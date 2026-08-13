import { cardSurface, Icon, type IconName } from "@/shared/ui/kit";
import type { LoadState, Memo } from "../service/memoView";
import { MemoCard } from "./MemoCard";

/** 読み込み中・失敗・0件をひと通り出す一覧。 */
export function MemoList({
  memos,
  status,
  openMemo,
  toggleMemo,
  className = cardSurface,
  empty,
}: {
  memos: Memo[];
  status: LoadState;
  openMemo: (m: Memo) => void;
  toggleMemo: (id: string) => void;
  className?: string;
  empty: { icon: IconName; title: string; hint: string };
}) {
  const emptyBox = "p-[75px] text-center text-[#999a94]";
  const emptyTitle = "mt-[1em] mb-1 text-sm font-bold text-[#555]";
  const emptyHint = "my-[1em] text-[11px]";
  if (status === "loading")
    return (
      <div className={emptyBox}>
        <Icon name="clock" size={30} />
        <h3 className={emptyTitle}>記録を読み込んでいます…</h3>
      </div>
    );
  if (status === "error")
    return (
      <div className={emptyBox}>
        <Icon name="inbox" size={30} />
        <h3 className={emptyTitle}>記録を読み込めませんでした</h3>
        <p className={emptyHint}>
          minos のデータベース（%LOCALAPPDATA%\minos\lineage.db）を開けませんでした。
        </p>
      </div>
    );
  if (!memos.length)
    return (
      <div className={emptyBox}>
        <Icon name={empty.icon} size={30} />
        <h3 className={emptyTitle}>{empty.title}</h3>
        <p className={emptyHint}>{empty.hint}</p>
      </div>
    );
  return (
    <div className={className}>
      {memos.map((m) => (
        <MemoCard
          memo={m}
          key={m.id}
          onOpen={() => openMemo(m)}
          onToggle={() => toggleMemo(m.id)}
        />
      ))}
    </div>
  );
}
