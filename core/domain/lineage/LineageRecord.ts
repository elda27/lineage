// Lineage の1レコード（= 1 link）。append-only。
// hash-chain: content_hash = sha256(canonicalize(seq, source, target, relation_type, actor, created_at, prev_hash))

export type AssetKind = "row" | "cell" | "document" | "table" | "attachment";

export type RelationType =
  | "memo_for"
  | "attachment_for"
  | "references"
  | "derived_from"
  | "evidence_for";

// hash 計算の対象になる本体（content_hash 自身は含まない）。
export interface LineageInput {
  workspaceId: string;
  sourceKind: AssetKind;
  sourceId: string;
  targetKind: AssetKind;
  targetId: string;
  relationType: RelationType;
  actor: string | null;
  createdAt: string;
}

export interface LineageRecord extends LineageInput {
  id: string;
  seq: number;
  prevHash: string;
  contentHash: string;
}

export interface VerifyResult {
  ok: boolean;
  brokenAt?: number; // 整合性が壊れた seq
  length: number; // 検証したレコード数
}
