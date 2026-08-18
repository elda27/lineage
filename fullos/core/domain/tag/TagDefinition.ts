export type TagKind = "user" | "builtin" | "metadata";
export type TagDefinition = {
  id: string; workspaceId: string; kind: TagKind; displayName: string;
  shorthand: string | null; usageCount: number; lastUsedAt: string | null;
  enabled: boolean; managed: boolean; deletedAt: string | null;
  view: string | null; recipe: string | null; recipeManaged: boolean;
};

export type TagUpdate = Pick<TagDefinition, "displayName" | "shorthand" | "enabled" | "view" | "recipe" | "recipeManaged">;
