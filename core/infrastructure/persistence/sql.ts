// SQLite / D1 共通の SQL 文字列・パラメータ整形・行マッパー。
// 実行ハンドル(plugin-sql Database / D1Database)だけが各実装で異なる。
// ここを1本に保つことで「永続化1スキーマ」を SQL レベルでも担保する。

import { TableAsset } from "../../domain/table/TableAsset";
import { Row } from "../../domain/table/Row";
import { Cell } from "../../domain/table/Cell";
import { DocumentAsset, DocumentType } from "../../domain/document/DocumentAsset";
import { LineageRecord, AssetKind, RelationType } from "../../domain/lineage/LineageRecord";

// ---- INSERT/SELECT SQL（? プレースホルダは SQLite/D1 共通）----

export const SQL = {
  insertTable:
    "INSERT INTO table_assets (id, workspace_id, name, schema_json, created_at) VALUES (?, ?, ?, ?, ?)",
  getTable: "SELECT * FROM table_assets WHERE id = ?",
  listTables:
    "SELECT * FROM table_assets WHERE workspace_id = ? ORDER BY created_at ASC",

  insertRow:
    "INSERT INTO rows (id, table_id, row_index, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
  getRow: "SELECT * FROM rows WHERE id = ?",
  listRows: "SELECT * FROM rows WHERE table_id = ? ORDER BY row_index ASC",
  maxRowIndex:
    "SELECT COALESCE(MAX(row_index), -1) AS max_index FROM rows WHERE table_id = ?",

  upsertCell:
    "INSERT INTO cells (id, row_id, column_key, raw_value, computed_value, formula, value_type, updated_at) " +
    "VALUES (?, ?, ?, ?, ?, ?, ?, ?) " +
    "ON CONFLICT(row_id, column_key) DO UPDATE SET " +
    "raw_value=excluded.raw_value, computed_value=excluded.computed_value, " +
    "formula=excluded.formula, value_type=excluded.value_type, updated_at=excluded.updated_at",
  listCells: "SELECT * FROM cells WHERE row_id = ? ORDER BY column_key ASC",

  insertDocument:
    "INSERT INTO documents (id, workspace_id, title, body_text, blob_uri, document_type, created_at, updated_at) " +
    "VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
  getDocument: "SELECT * FROM documents WHERE id = ?",

  insertLink:
    "INSERT INTO links (id, workspace_id, seq, source_kind, source_id, target_kind, target_id, relation_type, actor, created_at, content_hash, prev_hash) " +
    "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  lastLink:
    "SELECT * FROM links WHERE workspace_id = ? ORDER BY seq DESC LIMIT 1",
  listLinks:
    "SELECT * FROM links WHERE workspace_id = ? ORDER BY seq ASC",
} as const;

// ---- パラメータ整形（INSERT の bind 配列）----

export function tableParams(t: TableAsset): unknown[] {
  return [t.id, t.workspaceId, t.name, JSON.stringify(t.schema), t.createdAt];
}

export function rowParams(r: Row): unknown[] {
  return [r.id, r.tableId, r.rowIndex, r.createdAt, r.updatedAt];
}

export function cellParams(c: Cell): unknown[] {
  return [
    c.id,
    c.rowId,
    c.columnKey,
    c.rawValue,
    c.computedValue,
    c.formula,
    c.valueType,
    c.updatedAt,
  ];
}

export function documentParams(d: DocumentAsset): unknown[] {
  return [
    d.id,
    d.workspaceId,
    d.title,
    d.bodyText,
    d.blobUri,
    d.documentType,
    d.createdAt,
    d.updatedAt,
  ];
}

export function linkParams(l: LineageRecord): unknown[] {
  return [
    l.id,
    l.workspaceId,
    l.seq,
    l.sourceKind,
    l.sourceId,
    l.targetKind,
    l.targetId,
    l.relationType,
    l.actor,
    l.createdAt,
    l.contentHash,
    l.prevHash,
  ];
}

// ---- 行マッパー（DB 行 → ドメイン）----

type DbRow = Record<string, unknown>;

export function mapTable(r: DbRow): TableAsset {
  return {
    id: String(r.id),
    workspaceId: String(r.workspace_id),
    name: String(r.name),
    schema: JSON.parse(String(r.schema_json)),
    createdAt: String(r.created_at),
  };
}

export function mapRow(r: DbRow): Row {
  return {
    id: String(r.id),
    tableId: String(r.table_id),
    rowIndex: Number(r.row_index),
    createdAt: String(r.created_at),
    updatedAt: String(r.updated_at),
  };
}

export function mapCell(r: DbRow): Cell {
  return {
    id: String(r.id),
    rowId: String(r.row_id),
    columnKey: String(r.column_key),
    rawValue: nullableStr(r.raw_value),
    computedValue: nullableStr(r.computed_value),
    formula: nullableStr(r.formula),
    valueType: nullableStr(r.value_type),
    updatedAt: String(r.updated_at),
  };
}

export function mapDocument(r: DbRow): DocumentAsset {
  return {
    id: String(r.id),
    workspaceId: String(r.workspace_id),
    title: String(r.title),
    bodyText: nullableStr(r.body_text),
    blobUri: nullableStr(r.blob_uri),
    documentType: String(r.document_type) as DocumentType,
    createdAt: String(r.created_at),
    updatedAt: String(r.updated_at),
  };
}

export function mapLink(r: DbRow): LineageRecord {
  return {
    id: String(r.id),
    workspaceId: String(r.workspace_id),
    seq: Number(r.seq),
    sourceKind: String(r.source_kind) as AssetKind,
    sourceId: String(r.source_id),
    targetKind: String(r.target_kind) as AssetKind,
    targetId: String(r.target_id),
    relationType: String(r.relation_type) as RelationType,
    actor: nullableStr(r.actor),
    createdAt: String(r.created_at),
    contentHash: String(r.content_hash),
    prevHash: String(r.prev_hash),
  };
}

function nullableStr(v: unknown): string | null {
  return v === null || v === undefined ? null : String(v);
}
