import { AssetRepository } from "../domain/ports/AssetRepository";
import { LineageRepository } from "../domain/ports/LineageRepository";
import { UnitOfWork } from "../domain/ports/UnitOfWork";
import { Hasher } from "../domain/shared/Hasher";
import { nowIso } from "../domain/shared/Clock";
import { LineageLedger } from "../domain/lineage/LineageLedger";
import { LineageRecord } from "../domain/lineage/LineageRecord";
import {
  DocumentAsset,
  createDocumentAsset,
} from "../domain/document/DocumentAsset";

export interface WriteMemoInput {
  workspaceId: string;
  rowId: string;
  title: string;
  bodyText?: string | null;
  actor: string | null;
}

export interface WriteMemoResult {
  document: DocumentAsset;
  link: LineageRecord;
}

// Lineage 生成の本丸:
//  1) document を1件作る
//  2) 鎖の末尾を取得
//  3) row -> document の link を appendNext で生成（content_hash 付与）
//  4) document insert と link append を同一トランザクションで確定
export class WriteMemo {
  private readonly ledger: LineageLedger;

  constructor(
    private readonly assets: AssetRepository,
    private readonly lineage: LineageRepository,
    private readonly uow: UnitOfWork,
    hasher: Hasher
  ) {
    this.ledger = new LineageLedger(hasher);
  }

  async execute(input: WriteMemoInput): Promise<WriteMemoResult> {
    const row = await this.assets.getRow(input.rowId);
    if (!row) throw new Error(`row not found: ${input.rowId}`);

    const document = createDocumentAsset({
      workspaceId: input.workspaceId,
      title: input.title,
      bodyText: input.bodyText ?? null,
      documentType: "memo",
    });

    const prev = await this.lineage.lastLink(input.workspaceId);
    const link = await this.ledger.appendNext(prev, {
      workspaceId: input.workspaceId,
      sourceKind: "row",
      sourceId: input.rowId,
      targetKind: "document",
      targetId: document.id,
      relationType: "memo_for",
      actor: input.actor,
      createdAt: nowIso(),
    });

    await this.uow.insertDocumentWithLink(document, link);
    return { document, link };
  }
}
