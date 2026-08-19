import { metaText, type MetaAssignment } from "@core/domain/memo/Memo";
import { tagChip } from "@/shared/ui/kit";

/** メタ情報のバッジ列。`#ラベル` / `#ラベル=値` の形で出す。 */
export function MetaChips({
  metas,
  onRemove,
}: {
  metas: MetaAssignment[];
  onRemove?: (meta: MetaAssignment) => void;
}) {
  return (
    <>
      {metas.map((meta) => {
        const text = metaText(meta);
        return (
          <span className={`${tagChip} gap-1`} key={text}>
            #{text}
            {onRemove && (
              <button
                type="button"
                className="grid h-4 w-4 cursor-pointer place-items-center rounded-full border-0 bg-transparent p-0 text-[13px] leading-none text-[#777972] hover:bg-[#d8d7d2] hover:text-[#b05d5d]"
                aria-label={`タグ「${text}」を削除`}
                title="タグを削除"
                onClick={() => onRemove(meta)}
              >
                ×
              </button>
            )}
          </span>
        );
      })}
    </>
  );
}
