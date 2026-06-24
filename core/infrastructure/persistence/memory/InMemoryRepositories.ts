import { AssetRepository } from "../../../domain/ports/AssetRepository";
import {
  LineageRepository,
  LineageListFilter,
} from "../../../domain/ports/LineageRepository";
import { UnitOfWork } from "../../../domain/ports/UnitOfWork";
import { TableAsset } from "../../../domain/table/TableAsset";
import { Row } from "../../../domain/table/Row";
import { Cell } from "../../../domain/table/Cell";
import { DocumentAsset } from "../../../domain/document/DocumentAsset";
import { LineageRecord } from "../../../domain/lineage/LineageRecord";

// テスト用のインメモリ実装。SQL を介さず application / hash-chain の振る舞いを検証する。
export class InMemoryStore {
  tables = new Map<string, TableAsset>();
  rows = new Map<string, Row>();
  cells: Cell[] = [];
  documents = new Map<string, DocumentAsset>();
  links: LineageRecord[] = [];
}

export class InMemoryAssetRepository implements AssetRepository {
  constructor(private readonly s: InMemoryStore) {}

  async insertTable(t: TableAsset): Promise<void> {
    this.s.tables.set(t.id, t);
  }
  async getTable(tableId: string): Promise<TableAsset | null> {
    return this.s.tables.get(tableId) ?? null;
  }
  async listTables(workspaceId: string): Promise<TableAsset[]> {
    return [...this.s.tables.values()].filter(
      (t) => t.workspaceId === workspaceId
    );
  }

  async insertRow(r: Row, cells: Cell[]): Promise<void> {
    this.s.rows.set(r.id, r);
    for (const c of cells) this.upsert(c);
  }
  async getRow(rowId: string): Promise<Row | null> {
    return this.s.rows.get(rowId) ?? null;
  }
  async listRows(tableId: string): Promise<Row[]> {
    return [...this.s.rows.values()]
      .filter((r) => r.tableId === tableId)
      .sort((a, b) => a.rowIndex - b.rowIndex);
  }
  async nextRowIndex(tableId: string): Promise<number> {
    const rows = await this.listRows(tableId);
    return rows.reduce((m, r) => Math.max(m, r.rowIndex), -1) + 1;
  }

  async upsertCells(cells: Cell[]): Promise<void> {
    for (const c of cells) this.upsert(c);
  }
  async listCells(rowId: string): Promise<Cell[]> {
    return this.s.cells
      .filter((c) => c.rowId === rowId)
      .sort((a, b) => a.columnKey.localeCompare(b.columnKey));
  }

  async insertDocument(d: DocumentAsset): Promise<void> {
    this.s.documents.set(d.id, d);
  }
  async getDocument(documentId: string): Promise<DocumentAsset | null> {
    return this.s.documents.get(documentId) ?? null;
  }

  private upsert(c: Cell): void {
    const i = this.s.cells.findIndex(
      (x) => x.rowId === c.rowId && x.columnKey === c.columnKey
    );
    if (i >= 0) this.s.cells[i] = c;
    else this.s.cells.push(c);
  }
}

export class InMemoryLineageRepository implements LineageRepository {
  constructor(private readonly s: InMemoryStore) {}

  async lastLink(workspaceId: string): Promise<LineageRecord | null> {
    const links = this.s.links
      .filter((l) => l.workspaceId === workspaceId)
      .sort((a, b) => a.seq - b.seq);
    return links[links.length - 1] ?? null;
  }
  async append(link: LineageRecord): Promise<void> {
    this.s.links.push(link);
  }
  async list(
    workspaceId: string,
    filter?: LineageListFilter
  ): Promise<LineageRecord[]> {
    let records = this.s.links
      .filter((l) => l.workspaceId === workspaceId)
      .sort((a, b) => a.seq - b.seq);
    if (filter?.sourceKind)
      records = records.filter((r) => r.sourceKind === filter.sourceKind);
    if (filter?.sourceId)
      records = records.filter((r) => r.sourceId === filter.sourceId);
    return records;
  }
}

export class InMemoryUnitOfWork implements UnitOfWork {
  constructor(private readonly s: InMemoryStore) {}

  async insertDocumentWithLink(
    document: DocumentAsset,
    link: LineageRecord
  ): Promise<void> {
    this.s.documents.set(document.id, document);
    this.s.links.push(link);
  }
}
