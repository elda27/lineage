import type { TagRepository } from "../../domain/ports/TagRepository";
import type { TagUpdate } from "../../domain/tag/TagDefinition";
export class ManageTags {
  constructor(private readonly tags: TagRepository) {}
  list(workspaceId: string) { return this.tags.all(workspaceId); }
  update(id: string, value: TagUpdate) { return this.tags.update(id, value); }
  remove(id: string) { return this.tags.remove(id); }
}
