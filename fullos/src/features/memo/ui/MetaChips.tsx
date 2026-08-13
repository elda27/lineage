import { metaText, type MetaAssignment } from "@core/domain/memo/Memo";
import { tagChip } from "@/shared/ui/kit";

/** メタ情報のバッジ列。`#ラベル` / `#ラベル=値` の形で出す。 */
export function MetaChips({ metas }: { metas: MetaAssignment[] }) {
  return (
    <>
      {metas.map((meta) => {
        const text = metaText(meta);
        return (
          <span className={tagChip} key={text}>
            #{text}
          </span>
        );
      })}
    </>
  );
}
