import { LineageRepository } from "../domain/ports/LineageRepository";
import { Hasher } from "../domain/shared/Hasher";
import { nowIso } from "../domain/shared/Clock";
import { LineageLedger } from "../domain/lineage/LineageLedger";
import {
  AssetKind,
  LineageRecord,
  RelationType,
} from "../domain/lineage/LineageRecord";

export interface AppendLinkInput {
  workspaceId: string;
  sourceKind: AssetKind;
  sourceId: string;
  targetKind: AssetKind;
  targetId: string;
  relationType: RelationType;
  actor: string | null;
}

// 任意の lineage を append（append-only）。単一 INSERT なのでそれ自体が原子的。
export class AppendLink {
  private readonly ledger: LineageLedger;

  constructor(
    private readonly lineage: LineageRepository,
    hasher: Hasher
  ) {
    this.ledger = new LineageLedger(hasher);
  }

  async execute(input: AppendLinkInput): Promise<LineageRecord> {
    const prev = await this.lineage.lastLink(input.workspaceId);
    const link = await this.ledger.appendNext(prev, {
      ...input,
      createdAt: nowIso(),
    });
    await this.lineage.append(link);
    return link;
  }
}
