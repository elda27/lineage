import { UnitOfWork } from "../../../domain/ports/UnitOfWork";
import { DocumentAsset } from "../../../domain/document/DocumentAsset";
import { LineageRecord } from "../../../domain/lineage/LineageRecord";
import { SqlDatabase, runInTransaction } from "./SqlDatabase";
import * as q from "../sql";

export class SqliteUnitOfWork implements UnitOfWork {
  constructor(private readonly db: SqlDatabase) {}

  async insertDocumentWithLink(
    document: DocumentAsset,
    link: LineageRecord
  ): Promise<void> {
    await runInTransaction(this.db, async () => {
      await this.db.execute(q.SQL.insertDocument, q.documentParams(document));
      await this.db.execute(q.SQL.insertLink, q.linkParams(link));
    });
  }
}
