import { invoke } from "@tauri-apps/api/core";

import type { MemoStateRecord } from "../../../domain/memo/MemoState";
import type { MemoStateRepository } from "../../../domain/ports/MemoStateRepository";
import { selectOrEmpty, type SqlHandle } from "./SqlHandle";

type StateRow = {
  document_id: string;
  done: number;
  done_at: string | null;
  archived_at: string | null;
  deleted_at: string | null;
};

/**
 * 組み込みタグの状態（document_states）の読み書き。
 *
 * 読み取りは WebView の plugin-sql、mutation は Rust command を使う。
 * WebView に `sql:allow-execute` を与えないことで、状態変更の入口を application
 * boundary に固定する（ADR-0004）。
 */
export class SqliteMemoStateRepository implements MemoStateRepository {
  constructor(private readonly db: SqlHandle) {}

  async all(workspaceId: string): Promise<MemoStateRecord[]> {
    const rows = await selectOrEmpty<StateRow>(
      this.db,
      `SELECT document_id, done, done_at, archived_at, deleted_at
       FROM document_states
       WHERE workspace_id = $1`,
      [workspaceId],
    );

    return rows.map((row) => ({
      documentId: row.document_id,
      state: {
        done: row.done !== 0,
        doneAt: row.done_at,
        archivedAt: row.archived_at,
        deletedAt: row.deleted_at,
      },
    }));
  }

  async setDone(workspaceId: string, documentId: string, done: boolean, at: string): Promise<void> {
    await invoke<void>("memo_set_done", {
      workspaceId,
      memoId: documentId,
      done,
      at,
    });
  }

  async setArchived(workspaceId: string, documentId: string, at: string | null): Promise<void> {
    await invoke<void>("memo_set_archived", {
      workspaceId,
      memoId: documentId,
      archivedAt: at,
      updatedAt: at ?? new Date().toISOString(),
    });
  }

  async trash(workspaceId: string, documentId: string, at: string): Promise<void> {
    await invoke<void>("memo_trash", {
      workspaceId,
      memoId: documentId,
      at,
    });
  }

  async archiveDone(workspaceId: string, labels: string[], at: string): Promise<void> {
    await invoke<void>("memo_archive_done", {
      workspaceId,
      labels,
      at,
    });
  }
}
