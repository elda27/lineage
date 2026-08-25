import type { TagDefinition } from "../tag/TagDefinition";
export interface TagRepository {
  all(workspaceId: string): Promise<TagDefinition[]>;
  get(id: string): Promise<TagDefinition | null>;
}
