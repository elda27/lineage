import type { TagDefinition, TagUpdate } from "../tag/TagDefinition";
export interface TagRepository {
  all(workspaceId: string): Promise<TagDefinition[]>;
  get(id: string): Promise<TagDefinition | null>;
  update(id: string, value: TagUpdate): Promise<void>;
  remove(id: string): Promise<void>;
}
