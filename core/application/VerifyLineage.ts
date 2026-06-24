import { LineageRepository } from "../domain/ports/LineageRepository";
import { Hasher } from "../domain/shared/Hasher";
import { LineageLedger } from "../domain/lineage/LineageLedger";
import { VerifyResult } from "../domain/lineage/LineageRecord";

// hash-chain の真正性検証。ローカル/クラウド同一ロジック。
export class VerifyLineage {
  private readonly ledger: LineageLedger;

  constructor(
    private readonly lineage: LineageRepository,
    hasher: Hasher
  ) {
    this.ledger = new LineageLedger(hasher);
  }

  async execute(workspaceId: string): Promise<VerifyResult> {
    const records = await this.lineage.list(workspaceId);
    return this.ledger.verify(records);
  }
}
