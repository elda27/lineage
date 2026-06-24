// UI が依存する唯一のインターフェース。実装は LocalAppClient / HttpAppClient の2つ。
// UI は接続モード（ローカル/クラウド）を意識しない。
import { TableAsset, TableSchema } from "../../core/domain/table/TableAsset";
import { Cell } from "../../core/domain/table/Cell";
import {
  AssetKind,
  LineageRecord,
  RelationType,
  VerifyResult,
} from "../../core/domain/lineage/LineageRecord";
import { LineageListFilter } from "../../core/domain/ports/LineageRepository";
import { AppendRowResult } from "../../core/application/AppendRow";
import { TableData } from "../../core/application/GetTableData";
import { RowDetail } from "../../core/application/GetRowDetail";
import { MemoEntry } from "../../core/application/ListMemos";
import { WriteMemoResult } from "../../core/application/WriteMemo";

export interface CreateTableArgs {
  name: string;
  schema: TableSchema;
}

export interface AppendRowArgs {
  tableId: string;
  values: Record<string, string>;
}

export interface EditCellsArgs {
  rowId: string;
  values: Record<string, string>;
}

export interface WriteMemoArgs {
  rowId: string;
  title: string;
  bodyText?: string | null;
}

export interface AppendLinkArgs {
  sourceKind: AssetKind;
  sourceId: string;
  targetKind: AssetKind;
  targetId: string;
  relationType: RelationType;
}

// workspaceId / actor は各クライアントが内部で付与する（UI は渡さない）。
export interface ApplicationPort {
  readonly workspaceId: string;

  createTable(args: CreateTableArgs): Promise<TableAsset>;
  listTables(): Promise<TableAsset[]>;
  getTableData(tableId: string): Promise<TableData>;

  appendRow(args: AppendRowArgs): Promise<AppendRowResult>;
  editCells(args: EditCellsArgs): Promise<Cell[]>;
  getRowDetail(rowId: string): Promise<RowDetail>;

  writeMemo(args: WriteMemoArgs): Promise<WriteMemoResult>;
  listMemos(rowId: string): Promise<MemoEntry[]>;

  appendLink(args: AppendLinkArgs): Promise<LineageRecord>;
  listLinks(filter?: LineageListFilter): Promise<LineageRecord[]>;
  verifyLineage(): Promise<VerifyResult>;
}
