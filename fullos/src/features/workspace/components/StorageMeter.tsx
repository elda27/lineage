import { usageRatio } from "@core/domain/storage/StorageUsage";
import { formatBytes } from "@/shared/format";
import { useStorageUsage } from "../service/useStorageUsage";

/** クラウド接続のときだけ出る使用量メーター。ローカル接続では何も描画しない。 */
export function StorageMeter() {
  const usage = useStorageUsage();
  if (!usage) return null;
  return (
    <div className="px-[9px] pb-5">
      <div className="mb-2 flex justify-between text-[10px] text-[#92938e]">
        <span>ストレージ</span>
        <span>
          {formatBytes(usage.usedBytes)} / {formatBytes(usage.quotaBytes)}
        </span>
      </div>
      <div className="h-[3px] rounded bg-[#deded8]">
        <i
          className="block h-full rounded bg-[#7165ba]"
          style={{ width: `${usageRatio(usage) * 100}%` }}
        />
      </div>
    </div>
  );
}
