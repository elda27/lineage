import { TableAsset } from "../table/TableAsset";
import { Row } from "../table/Row";
import { Cell } from "../table/Cell";
import { DocumentAsset } from "../document/DocumentAsset";

// 永続化の抽象。SQL は infrastructure に閉じる。application はこの interface だけに依存する。
export interface AssetRepository {
  insertTable(t: TableAsset): Promise<void>;
  getTable(tableId: string): Promise<TableAsset | null>;
  listTables(workspaceId: string): Promise<TableAsset[]>;

  insertRow(r: Row, cells: Cell[]): Promise<void>;
  getRow(rowId: string): Promise<Row | null>;
  listRows(tableId: string): Promise<Row[]>;
  nextRowIndex(tableId: string): Promise<number>;

  upsertCells(cells: Cell[]): Promise<void>;
  listCells(rowId: string): Promise<Cell[]>;

  insertDocument(d: DocumentAsset): Promise<void>;
  getDocument(documentId: string): Promise<DocumentAsset | null>;
}
