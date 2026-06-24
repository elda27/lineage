import { describe, expect, it } from "vitest";
import {
  InMemoryAssetRepository,
  InMemoryLineageRepository,
  InMemoryStore,
  InMemoryUnitOfWork,
} from "../infrastructure/persistence/memory/InMemoryRepositories";
import { Sha256Hasher } from "../infrastructure/crypto/Sha256Hasher";
import { CreateTable } from "./CreateTable";
import { AppendRow } from "./AppendRow";
import { WriteMemo } from "./WriteMemo";
import { VerifyLineage } from "./VerifyLineage";
import { ListMemos } from "./ListMemos";

const WS = "w1";

function wire() {
  const store = new InMemoryStore();
  const assets = new InMemoryAssetRepository(store);
  const lineage = new InMemoryLineageRepository(store);
  const uow = new InMemoryUnitOfWork(store);
  const hasher = new Sha256Hasher();
  return { store, assets, lineage, uow, hasher };
}

describe("WriteMemo", () => {
  it("inserts a document and appends a memo_for link in one flow", async () => {
    const { assets, lineage, uow, hasher } = wire();

    const table = await new CreateTable(assets).execute({
      workspaceId: WS,
      name: "trades",
      schema: { columns: [{ key: "pnl", label: "pnl", type: "number" }] },
    });
    const { row } = await new AppendRow(assets).execute({
      tableId: table.id,
      values: { pnl: "-8000" },
    });

    const { document, link } = await new WriteMemo(
      assets,
      lineage,
      uow,
      hasher
    ).execute({
      workspaceId: WS,
      rowId: row.id,
      title: "SOXL loss-cut",
      bodyText: "expectation-driven move, exited",
      actor: "tester",
    });

    expect(link.relationType).toBe("memo_for");
    expect(link.sourceId).toBe(row.id);
    expect(link.targetId).toBe(document.id);
    expect(link.seq).toBe(1);

    const memos = await new ListMemos(assets, lineage).execute(WS, row.id);
    expect(memos).toHaveLength(1);
    expect(memos[0].document.title).toBe("SOXL loss-cut");

    const verify = await new VerifyLineage(lineage, hasher).execute(WS);
    expect(verify.ok).toBe(true);
    expect(verify.length).toBe(1);
  });

  it("keeps the chain valid across multiple memos", async () => {
    const { assets, lineage, uow, hasher } = wire();
    const table = await new CreateTable(assets).execute({
      workspaceId: WS,
      name: "t",
      schema: { columns: [] },
    });
    const { row } = await new AppendRow(assets).execute({
      tableId: table.id,
      values: {},
    });

    const memo = new WriteMemo(assets, lineage, uow, hasher);
    await memo.execute({ workspaceId: WS, rowId: row.id, title: "a", actor: "x" });
    await memo.execute({ workspaceId: WS, rowId: row.id, title: "b", actor: "x" });
    await memo.execute({ workspaceId: WS, rowId: row.id, title: "c", actor: "x" });

    const links = await lineage.list(WS);
    expect(links.map((l) => l.seq)).toEqual([1, 2, 3]);

    const verify = await new VerifyLineage(lineage, hasher).execute(WS);
    expect(verify.ok).toBe(true);
  });
});
