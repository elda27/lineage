import { newId } from "../shared/Id";
import { nowIso } from "../shared/Clock";

// テーブルのカラム定義。MVP では key/label/type のみ。
export interface ColumnDef {
  key: string;
  label: string;
  type: "text" | "number" | "date";
}

export interface TableSchema {
  columns: ColumnDef[];
}

export interface TableAsset {
  id: string;
  workspaceId: string;
  name: string;
  schema: TableSchema;
  createdAt: string;
}

export function createTableAsset(input: {
  workspaceId: string;
  name: string;
  schema: TableSchema;
}): TableAsset {
  return {
    id: newId(),
    workspaceId: input.workspaceId,
    name: input.name,
    schema: input.schema,
    createdAt: nowIso(),
  };
}
