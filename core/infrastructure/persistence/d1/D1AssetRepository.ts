import { AssetRepository } from "../../../domain/ports/AssetRepository";
import { TableAsset } from "../../../domain/table/TableAsset";
import { Row } from "../../../domain/table/Row";
import { Cell } from "../../../domain/table/Cell";
import { DocumentAsset } from "../../../domain/document/DocumentAsset";
import { D1Database } from "./D1Database";
import * as q from "../sql";

type DbRow = Record<string, unknown>;

export class D1AssetRepository implements AssetRepository {
  constructor(private readonly db: D1Database) {}

  async insertTable(t: TableAsset): Promise<void> {
    await this.db.prepare(q.SQL.insertTable).bind(...q.tableParams(t)).run();
  }

  async getTable(tableId: string): Promise<TableAsset | null> {
    const r = await this.db.prepare(q.SQL.getTable).bind(tableId).first<DbRow>();
    return r ? q.mapTable(r) : null;
  }

  async listTables(workspaceId: string): Promise<TableAsset[]> {
    const res = await this.db
      .prepare(q.SQL.listTables)
      .bind(workspaceId)
      .all<DbRow>();
    return res.results.map(q.mapTable);
  }

  async insertRow(r: Row, cells: Cell[]): Promise<void> {
    const stmts = [
      this.db.prepare(q.SQL.insertRow).bind(...q.rowParams(r)),
      ...cells.map((c) =>
        this.db.prepare(q.SQL.upsertCell).bind(...q.cellParams(c))
      ),
    ];
    await this.db.batch(stmts);
  }

  async getRow(rowId: string): Promise<Row | null> {
    const r = await this.db.prepare(q.SQL.getRow).bind(rowId).first<DbRow>();
    return r ? q.mapRow(r) : null;
  }

  async listRows(tableId: string): Promise<Row[]> {
    const res = await this.db
      .prepare(q.SQL.listRows)
      .bind(tableId)
      .all<DbRow>();
    return res.results.map(q.mapRow);
  }

  async nextRowIndex(tableId: string): Promise<number> {
    const r = await this.db
      .prepare(q.SQL.maxRowIndex)
      .bind(tableId)
      .first<{ max_index: number }>();
    return (r?.max_index ?? -1) + 1;
  }

  async upsertCells(cells: Cell[]): Promise<void> {
    const stmts = cells.map((c) =>
      this.db.prepare(q.SQL.upsertCell).bind(...q.cellParams(c))
    );
    await this.db.batch(stmts);
  }

  async listCells(rowId: string): Promise<Cell[]> {
    const res = await this.db
      .prepare(q.SQL.listCells)
      .bind(rowId)
      .all<DbRow>();
    return res.results.map(q.mapCell);
  }

  async insertDocument(d: DocumentAsset): Promise<void> {
    await this.db
      .prepare(q.SQL.insertDocument)
      .bind(...q.documentParams(d))
      .run();
  }

  async getDocument(documentId: string): Promise<DocumentAsset | null> {
    const r = await this.db
      .prepare(q.SQL.getDocument)
      .bind(documentId)
      .first<DbRow>();
    return r ? q.mapDocument(r) : null;
  }
}
