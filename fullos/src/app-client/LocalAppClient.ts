import Database from "@tauri-apps/plugin-sql";
import { join, localDataDir } from "@tauri-apps/api/path";

import { ListMemos, DEFAULT_MEMO_LIMIT } from "../../core/application/ListMemos";
import { SuggestMetaTags, DEFAULT_SUGGESTION_LIMIT } from "../../core/application/SuggestMetaTags";
import { SqliteMemoRepository } from "../../core/infrastructure/persistence/sqlite/SqliteMemoRepository";
import { SqliteMetaTagRepository } from "../../core/infrastructure/persistence/sqlite/SqliteMetaTagRepository";
import type { ApplicationPort } from "./ApplicationPort";

/** minos がローカルで使う workspace（minos/src/app.rs の DEFAULT_WORKSPACE_ID）。 */
export const DEFAULT_WORKSPACE_ID = "local";

/** minos の DB は `%LOCALAPPDATA%\minos\lineage.db`（minos/src/infrastructure/sqlite.rs）。 */
const MINOS_DIRECTORY = "minos";
const DATABASE_FILE_NAME = "lineage.db";

/**
 * ローカル接続（認証なし・単一利用者）の composition root。
 *
 * minos と同じ SQLite ファイルを開き、application を in-process で呼ぶ。
 * fullos は表示側なので書き込みはしない。
 */
export async function createLocalAppClient(): Promise<ApplicationPort> {
  const db = await Database.load(`sqlite:${await minosDatabasePath()}`);
  const memos = new SqliteMemoRepository(db);
  const metaTags = new SqliteMetaTagRepository(db);

  return {
    listMemos: (limit = DEFAULT_MEMO_LIMIT) =>
      new ListMemos(memos).execute(DEFAULT_WORKSPACE_ID, limit),

    suggestMetaTags: (query, limit = DEFAULT_SUGGESTION_LIMIT) =>
      new SuggestMetaTags(metaTags).execute(DEFAULT_WORKSPACE_ID, query, limit),

    // ローカル接続の保存先は利用者のディスクそのもので、割り当て上限が存在しない。
    // 使用量メーターはクォータのあるクラウド接続だけの関心事なので null を返す（＝非表示）。
    storageUsage: async () => null,

    // ローカル接続は認証なしの単一利用者で、名乗るアカウントが存在しない。
    // アカウント欄は認証のあるクラウド接続だけの関心事なので null を返す（＝非表示）。
    currentAccount: async () => null,
  };
}

/**
 * minos の DB の絶対パス。
 *
 * plugin-sql は接続文字列のパスをアプリのデータディレクトリへ join するが、
 * 絶対パスを渡せばそちらが優先されるので、minos の DB を直接開ける。
 */
async function minosDatabasePath(): Promise<string> {
  return join(await localDataDir(), MINOS_DIRECTORY, DATABASE_FILE_NAME);
}
