import { AssetRepository } from "../domain/ports/AssetRepository";
import { TableAsset } from "../domain/table/TableAsset";
import { Row } from "../domain/table/Row";
import { Cell } from "../domain/table/Cell";

export interface RowDetail {
  row: Row;
  table: TableAsset | null;
  cells: Cell[];
}

// 行詳細画面用。行 + 所属テーブル + セル。
export class GetRowDetail {
  constructor(private readonly assets: AssetRepository) {}

  async execute(rowId: string): Promise<RowDetail> {
    const row = await this.assets.getRow(rowId);
    if (!row) throw new Error(`row not found: ${rowId}`);
    const table = await this.assets.getTable(row.tableId);
    const cells = await this.assets.listCells(rowId);
    return { row, table, cells };
  }
}
