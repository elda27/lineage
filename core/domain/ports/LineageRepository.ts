import { AssetKind, LineageRecord } from "../lineage/LineageRecord";

export interface LineageListFilter {
  sourceKind?: AssetKind;
  sourceId?: string;
}

export interface LineageRepository {
  lastLink(workspaceId: string): Promise<LineageRecord | null>; // 鎖の末尾（seq 最大）
  append(link: LineageRecord): Promise<void>; // append-only
  list(
    workspaceId: string,
    filter?: LineageListFilter
  ): Promise<LineageRecord[]>; // seq 昇順
}
