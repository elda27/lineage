import Database from "@tauri-apps/plugin-sql";
import schemaSql from "../../db/schema.sql?raw";

import { Sha256Hasher } from "../../core/infrastructure/crypto/Sha256Hasher";
import {
  SqliteAssetRepository,
  SqliteLineageRepository,
  SqliteUnitOfWork,
} from "../../core/infrastructure/persistence/sqlite";
import { CreateTable } from "../../core/application/CreateTable";
import { AppendRow } from "../../core/application/AppendRow";
import { EditCells } from "../../core/application/EditCells";
import { WriteMemo } from "../../core/application/WriteMemo";
import { AppendLink } from "../../core/application/AppendLink";
import { VerifyLineage } from "../../core/application/VerifyLineage";
import { GetTableData } from "../../core/application/GetTableData";
import { GetRowDetail } from "../../core/application/GetRowDetail";
import { ListMemos } from "../../core/application/ListMemos";
import { ApplicationPort } from "./ApplicationPort";

const LOCAL_ACTOR = "local";

// ローカル接続（認証なし、単一利用者）の composition root。
// in-process で application を直接呼ぶ。
export async function createLocalAppClient(
  workspaceId: string,
  workspaceName = "My Workspace"
): Promise<ApplicationPort> {
  const db = await Database.load("sqlite:lineage.db");
  await applySchema(db);
  await ensureWorkspace(db, workspaceId, workspaceName);

  const assets = new SqliteAssetRepository(db);
  const lineage = new SqliteLineageRepository(db);
  const uow = new SqliteUnitOfWork(db);
  const hasher = new Sha256Hasher();

  return {
    workspaceId,

    createTable: (args) =>
      new CreateTable(assets).execute({ workspaceId, ...args }),
    listTables: () => assets.listTables(workspaceId),
    getTableData: (tableId) => new GetTableData(assets).execute(tableId),

    appendRow: (args) => new AppendRow(assets).execute(args),
    editCells: (args) => new EditCells(assets).execute(args),
    getRowDetail: (rowId) => new GetRowDetail(assets).execute(rowId),

    writeMemo: (args) =>
      new WriteMemo(assets, lineage, uow, hasher).execute({
        workspaceId,
        actor: LOCAL_ACTOR,
        ...args,
      }),
    listMemos: (rowId) => new ListMemos(assets, lineage).execute(workspaceId, rowId),

    appendLink: (args) =>
      new AppendLink(lineage, hasher).execute({
        workspaceId,
        actor: LOCAL_ACTOR,
        ...args,
      }),
    listLinks: (filter) => lineage.list(workspaceId, filter),
    verifyLineage: () => new VerifyLineage(lineage, hasher).execute(workspaceId),
  };
}

async function applySchema(db: Database): Promise<void> {
  for (const stmt of splitStatements(schemaSql)) {
    await db.execute(stmt);
  }
}

async function ensureWorkspace(
  db: Database,
  workspaceId: string,
  name: string
): Promise<void> {
  await db.execute(
    "INSERT OR IGNORE INTO workspaces (id, name, owner_user_id, created_at) VALUES (?, ?, ?, ?)",
    [workspaceId, name, null, new Date().toISOString()]
  );
}

// schema.sql を文単位に分割（コメント行と空文を除去）。
function splitStatements(sql: string): string[] {
  return sql
    .split("\n")
    .filter((line) => !line.trim().startsWith("--"))
    .join("\n")
    .split(";")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}
