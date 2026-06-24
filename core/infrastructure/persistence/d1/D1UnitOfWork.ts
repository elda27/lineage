import { UnitOfWork } from "../../../domain/ports/UnitOfWork";
import { DocumentAsset } from "../../../domain/document/DocumentAsset";
import { LineageRecord } from "../../../domain/lineage/LineageRecord";
import { D1Database } from "./D1Database";
import * as q from "../sql";

export class D1UnitOfWork implements UnitOfWork {
  constructor(private readonly db: D1Database) {}

  async insertDocumentWithLink(
    document: DocumentAsset,
    link: LineageRecord
  ): Promise<void> {
    // D1 の batch は1トランザクションとして原子的に適用される。
    await this.db.batch([
      this.db.prepare(q.SQL.insertDocument).bind(...q.documentParams(document)),
      this.db.prepare(q.SQL.insertLink).bind(...q.linkParams(link)),
    ]);
  }
}
