import { AssetRepository } from "../domain/ports/AssetRepository";
import {
  TableAsset,
  TableSchema,
  createTableAsset,
} from "../domain/table/TableAsset";

export interface CreateTableInput {
  workspaceId: string;
  name: string;
  schema: TableSchema;
}

export class CreateTable {
  constructor(private readonly assets: AssetRepository) {}

  async execute(input: CreateTableInput): Promise<TableAsset> {
    const table = createTableAsset(input);
    await this.assets.insertTable(table);
    return table;
  }
}
