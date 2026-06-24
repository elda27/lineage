import { AssetRepository } from "../domain/ports/AssetRepository";
import { TableAsset } from "../domain/table/TableAsset";
import { Row } from "../domain/table/Row";
import { Cell } from "../domain/table/Cell";

export interface TableDataRow {
  row: Row;
  cells: Cell[];
}

export interface TableData {
  table: TableAsset;
  rows: TableDataRow[];
}

// グリッド表示用。テーブル定義 + 全行 + 各行のセル。
export class GetTableData {
  constructor(private readonly assets: AssetRepository) {}

  async execute(tableId: string): Promise<TableData> {
    const table = await this.assets.getTable(tableId);
    if (!table) throw new Error(`table not found: ${tableId}`);
    const rows = await this.assets.listRows(tableId);
    const data: TableDataRow[] = [];
    for (const row of rows) {
      data.push({ row, cells: await this.assets.listCells(row.id) });
    }
    return { table, rows: data };
  }
}
