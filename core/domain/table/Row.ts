import { newId } from "../shared/Id";
import { nowIso } from "../shared/Clock";

export interface Row {
  id: string;
  tableId: string;
  rowIndex: number;
  createdAt: string;
  updatedAt: string;
}

export function createRow(input: { tableId: string; rowIndex: number }): Row {
  const ts = nowIso();
  return {
    id: newId(),
    tableId: input.tableId,
    rowIndex: input.rowIndex,
    createdAt: ts,
    updatedAt: ts,
  };
}
