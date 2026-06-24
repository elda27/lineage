import {
  LineageRepository,
  LineageListFilter,
} from "../../../domain/ports/LineageRepository";
import { LineageRecord } from "../../../domain/lineage/LineageRecord";
import { D1Database } from "./D1Database";
import * as q from "../sql";

type DbRow = Record<string, unknown>;

export class D1LineageRepository implements LineageRepository {
  constructor(private readonly db: D1Database) {}

  async lastLink(workspaceId: string): Promise<LineageRecord | null> {
    const r = await this.db
      .prepare(q.SQL.lastLink)
      .bind(workspaceId)
      .first<DbRow>();
    return r ? q.mapLink(r) : null;
  }

  async append(link: LineageRecord): Promise<void> {
    await this.db.prepare(q.SQL.insertLink).bind(...q.linkParams(link)).run();
  }

  async list(
    workspaceId: string,
    filter?: LineageListFilter
  ): Promise<LineageRecord[]> {
    const res = await this.db
      .prepare(q.SQL.listLinks)
      .bind(workspaceId)
      .all<DbRow>();
    let records = res.results.map(q.mapLink);
    if (filter?.sourceKind) {
      records = records.filter((r) => r.sourceKind === filter.sourceKind);
    }
    if (filter?.sourceId) {
      records = records.filter((r) => r.sourceId === filter.sourceId);
    }
    return records;
  }
}
