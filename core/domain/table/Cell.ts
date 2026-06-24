import { newId } from "../shared/Id";
import { nowIso } from "../shared/Clock";

export interface Cell {
  id: string;
  rowId: string;
  columnKey: string;
  rawValue: string | null;
  computedValue: string | null;
  formula: string | null;
  valueType: string | null;
  updatedAt: string;
}

export function createCell(input: {
  rowId: string;
  columnKey: string;
  rawValue?: string | null;
  computedValue?: string | null;
  formula?: string | null;
  valueType?: string | null;
}): Cell {
  return {
    id: newId(),
    rowId: input.rowId,
    columnKey: input.columnKey,
    rawValue: input.rawValue ?? null,
    computedValue: input.computedValue ?? null,
    formula: input.formula ?? null,
    valueType: input.valueType ?? null,
    updatedAt: nowIso(),
  };
}
