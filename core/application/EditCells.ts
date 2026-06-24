import { AssetRepository } from "../domain/ports/AssetRepository";
import { Cell, createCell } from "../domain/table/Cell";

export interface EditCellsInput {
  rowId: string;
  // columnKey -> raw value
  values: Record<string, string>;
}

// セル編集（PATCH 相当）。Lineage は生まない。既存セルは columnKey 一致で上書き。
export class EditCells {
  constructor(private readonly assets: AssetRepository) {}

  async execute(input: EditCellsInput): Promise<Cell[]> {
    const existing = await this.assets.listCells(input.rowId);
    const byKey = new Map(existing.map((c) => [c.columnKey, c]));

    const updated: Cell[] = [];
    for (const [columnKey, rawValue] of Object.entries(input.values)) {
      const prev = byKey.get(columnKey);
      updated.push(
        createCell({
          rowId: input.rowId,
          columnKey,
          rawValue,
          valueType: prev?.valueType ?? null,
        })
      );
    }

    await this.assets.upsertCells(updated);
    return updated;
  }
}
