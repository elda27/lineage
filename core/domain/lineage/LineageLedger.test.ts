import { describe, expect, it } from "vitest";
import { GENESIS_HASH, LineageLedger } from "./LineageLedger";
import { LineageInput, LineageRecord } from "./LineageRecord";
import { Sha256Hasher } from "../../infrastructure/crypto/Sha256Hasher";

const hasher = new Sha256Hasher();

function input(n: number): LineageInput {
  return {
    workspaceId: "w1",
    sourceKind: "row",
    sourceId: `row-${n}`,
    targetKind: "document",
    targetId: `doc-${n}`,
    relationType: "memo_for",
    actor: "tester",
    createdAt: "2026-06-24T00:00:00.000Z",
  };
}

async function buildChain(length: number): Promise<LineageRecord[]> {
  const ledger = new LineageLedger(hasher);
  const records: LineageRecord[] = [];
  let prev: LineageRecord | null = null;
  for (let i = 1; i <= length; i++) {
    prev = await ledger.appendNext(prev, input(i));
    records.push(prev);
  }
  return records;
}

describe("LineageLedger", () => {
  it("links seq and prev_hash into a chain", async () => {
    const records = await buildChain(3);
    expect(records.map((r) => r.seq)).toEqual([1, 2, 3]);
    expect(records[0].prevHash).toBe(GENESIS_HASH);
    expect(records[1].prevHash).toBe(records[0].contentHash);
    expect(records[2].prevHash).toBe(records[1].contentHash);
  });

  it("verifies an intact chain", async () => {
    const ledger = new LineageLedger(hasher);
    const result = await ledger.verify(await buildChain(5));
    expect(result.ok).toBe(true);
    expect(result.length).toBe(5);
  });

  it("detects tampering of a record's content", async () => {
    const ledger = new LineageLedger(hasher);
    const records = await buildChain(4);
    // 2件目の内容を改ざん（content_hash は元のまま）
    records[1] = { ...records[1], targetId: "doc-HACKED" };
    const result = await ledger.verify(records);
    expect(result.ok).toBe(false);
    expect(result.brokenAt).toBe(2);
  });

  it("detects a broken prev_hash link", async () => {
    const ledger = new LineageLedger(hasher);
    const records = await buildChain(3);
    records[2] = { ...records[2], prevHash: GENESIS_HASH };
    const result = await ledger.verify(records);
    expect(result.ok).toBe(false);
    expect(result.brokenAt).toBe(3);
  });

  it("detects a removed (out-of-sequence) record", async () => {
    const ledger = new LineageLedger(hasher);
    const records = await buildChain(3);
    const withGap = [records[0], records[2]]; // seq 2 を削除
    const result = await ledger.verify(withGap);
    expect(result.ok).toBe(false);
    expect(result.brokenAt).toBe(3);
  });
});
