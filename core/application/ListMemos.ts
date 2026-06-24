import { AssetRepository } from "../domain/ports/AssetRepository";
import { LineageRepository } from "../domain/ports/LineageRepository";
import { DocumentAsset } from "../domain/document/DocumentAsset";
import { LineageRecord } from "../domain/lineage/LineageRecord";

export interface MemoEntry {
  document: DocumentAsset;
  link: LineageRecord;
}

// ある行に紐づくメモ一覧。lineage(memo_for, source=row) から document を辿る。
export class ListMemos {
  constructor(
    private readonly assets: AssetRepository,
    private readonly lineage: LineageRepository
  ) {}

  async execute(workspaceId: string, rowId: string): Promise<MemoEntry[]> {
    const links = await this.lineage.list(workspaceId, {
      sourceKind: "row",
      sourceId: rowId,
    });
    const entries: MemoEntry[] = [];
    for (const link of links) {
      if (link.relationType !== "memo_for") continue;
      const document = await this.assets.getDocument(link.targetId);
      if (document) entries.push({ document, link });
    }
    return entries;
  }
}
