import { AssetRepository } from "../domain/ports/AssetRepository";
import { Row, createRow } from "../domain/table/Row";
import { Cell, createCell } from "../domain/table/Cell";

export interface AppendRowInput {
  tableId: string;
  // columnKey -> raw value
  values: Record<string, string>;
}

export interface AppendRowResult {
  row: Row;
  cells: Cell[];
}

export class AppendRow {
  constructor(private readonly assets: AssetRepository) {}

  async execute(input: AppendRowInput): Promise<AppendRowResult> {
    const table = await this.assets.getTable(input.tableId);
    if (!table) throw new Error(`table not found: ${input.tableId}`);

    const rowIndex = await this.assets.nextRowIndex(input.tableId);
    const row = createRow({ tableId: input.tableId, rowIndex });

    const cells = table.schema.columns.map((col) =>
      createCell({
        rowId: row.id,
        columnKey: col.key,
        rawValue: input.values[col.key] ?? null,
        valueType: col.type,
      })
    );

    await this.assets.insertRow(row, cells);
    return { row, cells };
  }
}
