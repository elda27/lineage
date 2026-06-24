import { newId } from "../shared/Id";
import { nowIso } from "../shared/Clock";

// 記録(メモ/振り返り/改善案)。UI では「ドキュメント」と意識させないが内部表現はこれ。
export type DocumentType = "memo" | "note" | "attachment";

export interface DocumentAsset {
  id: string;
  workspaceId: string;
  title: string;
  bodyText: string | null;
  blobUri: string | null;
  documentType: DocumentType;
  createdAt: string;
  updatedAt: string;
}

export function createDocumentAsset(input: {
  workspaceId: string;
  title: string;
  bodyText?: string | null;
  blobUri?: string | null;
  documentType: DocumentType;
}): DocumentAsset {
  const ts = nowIso();
  return {
    id: newId(),
    workspaceId: input.workspaceId,
    title: input.title,
    bodyText: input.bodyText ?? null,
    blobUri: input.blobUri ?? null,
    documentType: input.documentType,
    createdAt: ts,
    updatedAt: ts,
  };
}
