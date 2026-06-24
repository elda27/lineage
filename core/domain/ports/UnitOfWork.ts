import { DocumentAsset } from "../document/DocumentAsset";
import { LineageRecord } from "../lineage/LineageRecord";

// Lineage を生む操作で「asset の永続化」と「link の append」を同一トランザクションで確定するための抽象。
// 実装は infra 側で SQLite なら BEGIN/COMMIT、D1 なら DB.batch([...]) を使う。
// link は append-only。ここで更新・削除はしない。
export interface UnitOfWork {
  // document を1件 insert し、同じトランザクションで lineage link を append する（WriteMemo 用）。
  insertDocumentWithLink(
    document: DocumentAsset,
    link: LineageRecord
  ): Promise<void>;
}
