export type TagKind = "user" | "builtin" | "metadata";
export type TagDefinition = {
  id: string;
  workspaceId: string;
  kind: TagKind;
  displayName: string;
  shorthand: string | null;
  usageCount: number;
  lastUsedAt: string | null;
  enabled: boolean;
  managed: boolean;
  deletedAt: string | null;
  view: string | null;
  recipe: string | null;
  recipeManaged: boolean;
};

/** 既存タグの変更された項目だけを送る差分。 */
export type TagRecipePatch = {
  name: string;
  managed: boolean;
};

export type TagPatch = {
  displayName?: string;
  /** `undefined` は変更なし、`null` は値を消す。 */
  shorthand?: string | null;
  enabled?: boolean;
  /** `undefined` は変更なし、`null` は binding を消す。 */
  view?: string | null;
  /** `undefined` は変更なし、`null` は binding を消す。 */
  recipe?: TagRecipePatch | null;
};
