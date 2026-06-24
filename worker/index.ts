import { Hono } from "hono";
import { cors } from "hono/cors";
import { createRemoteJWKSet, jwtVerify } from "jose";

import {
  D1AssetRepository,
  D1LineageRepository,
  D1UnitOfWork,
} from "../core/infrastructure/persistence/d1";
import { Sha256Hasher } from "../core/infrastructure/crypto/Sha256Hasher";
import { CreateTable } from "../core/application/CreateTable";
import { AppendRow } from "../core/application/AppendRow";
import { EditCells } from "../core/application/EditCells";
import { WriteMemo } from "../core/application/WriteMemo";
import { AppendLink } from "../core/application/AppendLink";
import { VerifyLineage } from "../core/application/VerifyLineage";
import { GetTableData } from "../core/application/GetTableData";
import { GetRowDetail } from "../core/application/GetRowDetail";
import { ListMemos } from "../core/application/ListMemos";
import {
  AssetKind,
  RelationType,
} from "../core/domain/lineage/LineageRecord";

type Env = {
  DB: D1Database;
  // 未設定なら JWT 検証をスキップ（スタブ配線）。設定済みなら jose で検証する。
  SUPABASE_JWKS_URL?: string;
  SUPABASE_JWT_ISSUER?: string;
};

type Vars = { actor: string };

const app = new Hono<{ Bindings: Env; Variables: Vars }>();

app.use("*", cors());

// 認証ミドルウェア（スタブ配線）:
//  - SUPABASE_JWKS_URL / SUPABASE_JWT_ISSUER が未設定なら無効化し actor="anonymous"。
//  - 設定済みなら Bearer JWT を jose で検証し actor=sub。
app.use("/api/*", async (c, next) => {
  const jwksUrl = c.env.SUPABASE_JWKS_URL;
  const issuer = c.env.SUPABASE_JWT_ISSUER;

  if (!jwksUrl || !issuer) {
    c.set("actor", "anonymous");
    return next();
  }

  const auth = c.req.header("Authorization");
  if (!auth?.startsWith("Bearer ")) {
    return c.json({ error: "Unauthorized" }, 401);
  }
  try {
    const jwks = createRemoteJWKSet(new URL(jwksUrl));
    const { payload } = await jwtVerify(auth.slice(7), jwks, { issuer });
    c.set("actor", String(payload.sub));
  } catch {
    return c.json({ error: "Unauthorized" }, 401);
  }
  return next();
});

function deps(env: Env) {
  const assets = new D1AssetRepository(env.DB);
  const lineage = new D1LineageRepository(env.DB);
  const uow = new D1UnitOfWork(env.DB);
  const hasher = new Sha256Hasher();
  return { assets, lineage, uow, hasher };
}

app.post("/api/tables", async (c) => {
  const body = await c.req.json();
  const { assets } = deps(c.env);
  const table = await new CreateTable(assets).execute({
    workspaceId: body.workspaceId,
    name: body.name,
    schema: body.schema,
  });
  return c.json(table);
});

app.get("/api/tables", async (c) => {
  const workspaceId = c.req.query("workspaceId") ?? "";
  const { assets } = deps(c.env);
  return c.json(await assets.listTables(workspaceId));
});

app.get("/api/tables/:tableId", async (c) => {
  const { assets } = deps(c.env);
  return c.json(await new GetTableData(assets).execute(c.req.param("tableId")));
});

app.post("/api/tables/:tableId/rows", async (c) => {
  const body = await c.req.json();
  const { assets } = deps(c.env);
  const result = await new AppendRow(assets).execute({
    tableId: c.req.param("tableId"),
    values: body.values ?? {},
  });
  return c.json(result);
});

app.patch("/api/rows/:rowId/cells", async (c) => {
  const body = await c.req.json();
  const { assets } = deps(c.env);
  const cells = await new EditCells(assets).execute({
    rowId: c.req.param("rowId"),
    values: body.values ?? {},
  });
  return c.json(cells);
});

app.get("/api/rows/:rowId", async (c) => {
  const { assets } = deps(c.env);
  return c.json(await new GetRowDetail(assets).execute(c.req.param("rowId")));
});

app.post("/api/rows/:rowId/memos", async (c) => {
  const body = await c.req.json();
  const { assets, lineage, uow, hasher } = deps(c.env);
  const result = await new WriteMemo(assets, lineage, uow, hasher).execute({
    workspaceId: body.workspaceId,
    rowId: c.req.param("rowId"),
    title: body.title ?? "memo",
    bodyText: body.bodyText ?? null,
    actor: c.get("actor"),
  });
  return c.json(result);
});

app.get("/api/rows/:rowId/memos", async (c) => {
  const workspaceId = c.req.query("workspaceId") ?? "";
  const { assets, lineage } = deps(c.env);
  return c.json(
    await new ListMemos(assets, lineage).execute(workspaceId, c.req.param("rowId"))
  );
});

app.post("/api/links", async (c) => {
  const body = await c.req.json();
  const { lineage, hasher } = deps(c.env);
  const link = await new AppendLink(lineage, hasher).execute({
    workspaceId: body.workspaceId,
    sourceKind: body.sourceKind as AssetKind,
    sourceId: body.sourceId,
    targetKind: body.targetKind as AssetKind,
    targetId: body.targetId,
    relationType: body.relationType as RelationType,
    actor: c.get("actor"),
  });
  return c.json(link);
});

app.get("/api/links", async (c) => {
  const workspaceId = c.req.query("workspaceId") ?? "";
  const { lineage } = deps(c.env);
  const records = await lineage.list(workspaceId, {
    sourceKind: c.req.query("sourceKind") as AssetKind | undefined,
    sourceId: c.req.query("sourceId") || undefined,
  });
  return c.json(records);
});

app.get("/api/workspaces/:id/lineage/verify", async (c) => {
  const { lineage, hasher } = deps(c.env);
  return c.json(await new VerifyLineage(lineage, hasher).execute(c.req.param("id")));
});

export default app;
