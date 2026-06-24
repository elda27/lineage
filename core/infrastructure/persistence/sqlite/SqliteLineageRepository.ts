import {
  LineageRepository,
  LineageListFilter,
} from "../../../domain/ports/LineageRepository";
import { LineageRecord } from "../../../domain/lineage/LineageRecord";
import { SqlDatabase } from "./SqlDatabase";
import * as q from "../sql";

export class SqliteLineageRepository implements LineageRepository {
  constructor(private readonly db: SqlDatabase) {}

  async lastLink(workspaceId: string): Promise<LineageRecord | null> {
    const rows = await this.db.select<Record<string, unknown>[]>(
      q.SQL.lastLink,
      [workspaceId]
    );
    return rows[0] ? q.mapLink(rows[0]) : null;
  }

  async append(link: LineageRecord): Promise<void> {
    await this.db.execute(q.SQL.insertLink, q.linkParams(link));
  }

  async list(
    workspaceId: string,
    filter?: LineageListFilter
  ): Promise<LineageRecord[]> {
    const rows = await this.db.select<Record<string, unknown>[]>(
      q.SQL.listLinks,
      [workspaceId]
    );
    let records = rows.map(q.mapLink);
    if (filter?.sourceKind) {
      records = records.filter((r) => r.sourceKind === filter.sourceKind);
    }
    if (filter?.sourceId) {
      records = records.filter((r) => r.sourceId === filter.sourceId);
    }
    return records;
  }
}
