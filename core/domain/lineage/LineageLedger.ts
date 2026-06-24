import { Hasher } from "../shared/Hasher";
import { canonicalize } from "../shared/canonicalize";
import { newId } from "../shared/Id";
import {
  LineageInput,
  LineageRecord,
  VerifyResult,
} from "./LineageRecord";

// 鎖の先頭が指す genesis 定数。
export const GENESIS_HASH =
  "0000000000000000000000000000000000000000000000000000000000000000";

// content_hash の対象ペイロード。id / contentHash は含めない。
// appendNext と verify で必ずこの1関数を共有し、計算ロジックを分岐させない。
function hashPayload(
  fields: LineageInput,
  seq: number,
  prevHash: string
): string {
  return canonicalize({
    workspaceId: fields.workspaceId,
    sourceKind: fields.sourceKind,
    sourceId: fields.sourceId,
    targetKind: fields.targetKind,
    targetId: fields.targetId,
    relationType: fields.relationType,
    actor: fields.actor,
    createdAt: fields.createdAt,
    seq,
    prevHash,
  });
}

export class LineageLedger {
  constructor(private readonly hasher: Hasher) {}

  // 直前レコードを受け取り、鎖を1つ伸ばす。
  async appendNext(
    prev: LineageRecord | null,
    input: LineageInput
  ): Promise<LineageRecord> {
    const seq = (prev?.seq ?? 0) + 1;
    const prevHash = prev?.contentHash ?? GENESIS_HASH;
    const contentHash = await this.hasher.sha256Hex(
      hashPayload(input, seq, prevHash)
    );
    return { ...input, id: newId(), seq, prevHash, contentHash };
  }

  // 台帳全体を再計算して鎖の整合性を検証する（records は seq 昇順）。
  async verify(records: LineageRecord[]): Promise<VerifyResult> {
    let prevHash = GENESIS_HASH;
    let expectedSeq = 1;
    for (const r of records) {
      if (r.seq !== expectedSeq) {
        return { ok: false, brokenAt: r.seq, length: records.length };
      }
      if (r.prevHash !== prevHash) {
        return { ok: false, brokenAt: r.seq, length: records.length };
      }
      const expected = await this.hasher.sha256Hex(
        hashPayload(r, r.seq, r.prevHash)
      );
      if (r.contentHash !== expected) {
        return { ok: false, brokenAt: r.seq, length: records.length };
      }
      prevHash = r.contentHash;
      expectedSeq += 1;
    }
    return { ok: true, length: records.length };
  }
}
