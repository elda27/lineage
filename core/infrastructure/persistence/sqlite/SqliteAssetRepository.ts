import { AssetRepository } from "../../../domain/ports/AssetRepository";
import { TableAsset } from "../../../domain/table/TableAsset";
import { Row } from "../../../domain/table/Row";
import { Cell } from "../../../domain/table/Cell";
import { DocumentAsset } from "../../../domain/document/DocumentAsset";
import { SqlDatabase, runInTransaction } from "./SqlDatabase";
import * as q from "../sql";

export class SqliteAssetRepository implements AssetRepository {
  constructor(private readonly db: SqlDatabase) {}

  async insertTable(t: TableAsset): Promise<void> {
    await this.db.execute(q.SQL.insertTable, q.tableParams(t));
  }

  async getTable(tableId: string): Promise<TableAsset | null> {
    const rows = await this.db.select<Record<string, unknown>[]>(
      q.SQL.getTable,
      [tableId]
    );
    return rows[0] ? q.mapTable(rows[0]) : null;
  }

  async listTables(workspaceId: string): Promise<TableAsset[]> {
    const rows = await this.db.select<Record<string, unknown>[]>(
      q.SQL.listTables,
      [workspaceId]
    );
    return rows.map(q.mapTable);
  }

  async insertRow(r: Row, cells: Cell[]): Promise<void> {
    await runInTransaction(this.db, async () => {
      await this.db.execute(q.SQL.insertRow, q.rowParams(r));
      for (const c of cells) {
        await this.db.execute(q.SQL.upsertCell, q.cellParams(c));
      }
    });
  }

  async getRow(rowId: string): Promise<Row | null> {
    const rows = await this.db.select<Record<string, unknown>[]>(q.SQL.getRow, [
      rowId,
    ]);
    return rows[0] ? q.mapRow(rows[0]) : null;
  }

  async listRows(tableId: string): Promise<Row[]> {
    const rows = await this.db.select<Record<string, unknown>[]>(
      q.SQL.listRows,
      [tableId]
    );
    return rows.map(q.mapRow);
  }

  async nextRowIndex(tableId: string): Promise<number> {
    const rows = await this.db.select<{ max_index: number }[]>(
      q.SQL.maxRowIndex,
      [tableId]
    );
    return (rows[0]?.max_index ?? -1) + 1;
  }

  async upsertCells(cells: Cell[]): Promise<void> {
    await runInTransaction(this.db, async () => {
      for (const c of cells) {
        await this.db.execute(q.SQL.upsertCell, q.cellParams(c));
      }
    });
  }

  async listCells(rowId: string): Promise<Cell[]> {
    const rows = await this.db.select<Record<string, unknown>[]>(
      q.SQL.listCells,
      [rowId]
    );
    return rows.map(q.mapCell);
  }

  async insertDocument(d: DocumentAsset): Promise<void> {
    await this.db.execute(q.SQL.insertDocument, q.documentParams(d));
  }

  async getDocument(documentId: string): Promise<DocumentAsset | null> {
    const rows = await this.db.select<Record<string, unknown>[]>(
      q.SQL.getDocument,
      [documentId]
    );
    return rows[0] ? q.mapDocument(rows[0]) : null;
  }
}
