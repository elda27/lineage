import Database from "@tauri-apps/plugin-sql";
import { invoke } from "@tauri-apps/api/core";
import { join, localDataDir } from "@tauri-apps/api/path";

import { ListMemos, DEFAULT_MEMO_LIMIT } from "../../core/application/ListMemos";
import { SuggestMetaTags, DEFAULT_SUGGESTION_LIMIT } from "../../core/application/SuggestMetaTags";
import type {
  AutomationRule,
  AutomationRuleInput,
  AutomationRun,
} from "../../core/domain/automation/AutomationRule";
import {
  BROWSER_PROFILES_SETTING_KEY,
  parseBrowserProfileOverrides,
  resolveBrowserProfile,
  type BrowserProfile,
} from "../../core/domain/automation/BrowserProfile";
import {
  SqliteAutomationRuleRepository,
  SqliteAutomationRunRepository,
} from "../../core/infrastructure/persistence/sqlite/SqliteAutomationRepository";
import { SqliteSettingsRepository } from "../../core/infrastructure/persistence/sqlite/SqliteSettingsRepository";
import { SqliteMemoRepository } from "../../core/infrastructure/persistence/sqlite/SqliteMemoRepository";
import { SqliteMetaTagRepository } from "../../core/infrastructure/persistence/sqlite/SqliteMetaTagRepository";
import type { ApplicationPort } from "./ApplicationPort";

/** minos がローカルで使う workspace（minos/src/app.rs の DEFAULT_WORKSPACE_ID）。 */
export const DEFAULT_WORKSPACE_ID = "local";

/** minos の DB は `%LOCALAPPDATA%\minos\lineage.db`（lineage-core の sqlite.rs）。 */
const MINOS_DIRECTORY = "minos";
const DATABASE_FILE_NAME = "lineage.db";

/** 実行履歴の既定の取得件数。 */
const DEFAULT_RUN_LIMIT = 50;

/**
 * ローカル接続（認証なし・単一利用者）の composition root。
 *
 * minos と同じ SQLite ファイルを開き、application を in-process で呼ぶ。
 *
 * 書き込みは2系統に分かれる。
 *
 * - 自動化ルール … lineage を生まない設定なので、ここから plugin-sql で直接書く
 * - 自動化の実行 … 結果 document と links の追記を伴うので、Tauri コマンド越しに
 *   agentos（Rust）へ委ねる。webview からも鎖に書けると hash-chain の作り方が
 *   アプリごとに分岐しうるため（docs/concept/MINIMAL_ARCHITECTURE.md 4.）
 */
export async function createLocalAppClient(): Promise<ApplicationPort> {
  const db = await Database.load(`sqlite:${await minosDatabasePath()}`);
  const memos = new SqliteMemoRepository(db);
  const metaTags = new SqliteMetaTagRepository(db);
  const automationRules = new SqliteAutomationRuleRepository(db);
  const automationRuns = new SqliteAutomationRunRepository(db);
  const settings = new SqliteSettingsRepository(db);

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

    listAutomationRules: () => automationRules.all(DEFAULT_WORKSPACE_ID),

    saveAutomationRule: (input: AutomationRuleInput) =>
      automationRules.save(DEFAULT_WORKSPACE_ID, input),

    deleteAutomationRule: (id: string) => automationRules.remove(id),

    listAutomationRuns: (limit = DEFAULT_RUN_LIMIT) =>
      automationRuns.recent(DEFAULT_WORKSPACE_ID, limit),

    matchAutomationRules: (memoId: string) =>
      invoke<AutomationRule[]>("automation_match", { memoId }),

    runAutomation: (ruleId: string, memoId: string) =>
      runAutomation(automationRules, settings, ruleId, memoId),

    credentialStatus: async (providers: string[]) => {
      const entries = await Promise.all(
        providers.map(
          async (provider) =>
            [provider, await invoke<boolean>("credential_has", { provider })] as const,
        ),
      );
      return Object.fromEntries(entries);
    },

    setCredential: (provider: string, secret: string) =>
      invoke("credential_set", { provider, secret }),

    deleteCredential: (provider: string) => invoke("credential_delete", { provider }),

    verifyLineage: () => invoke<{ ok: boolean; detail: string }>("verify_lineage"),

    browserProfileOverrides: async () =>
      parseBrowserProfileOverrides(
        await settings.get(DEFAULT_WORKSPACE_ID, BROWSER_PROFILES_SETTING_KEY),
      ),

    scheduleStatus: () => invoke<boolean>("schedule_status"),

    registerSchedule: () => invoke("schedule_register"),

    unregisterSchedule: () => invoke("schedule_unregister"),

    saveBrowserProfileOverrides: (overrides) =>
      settings.set(
        DEFAULT_WORKSPACE_ID,
        BROWSER_PROFILES_SETTING_KEY,
        JSON.stringify(overrides),
      ),
  };
}

/**
 * バックエンドに応じて実行する。
 *
 * APIキー方式は agentos が最後まで面倒を見る。ブラウザ方式だけは WebView を持つ
 * fullos がプロンプトの送信と応答の回収を担うので、「組み立て → 実行 → 確定」の
 * 3段に分かれる。結果の保存（document と links）はどちらも agentos が行う。
 */
async function runAutomation(
  rules: SqliteAutomationRuleRepository,
  settings: SqliteSettingsRepository,
  ruleId: string,
  memoId: string,
): Promise<AutomationRun> {
  const rule = (await rules.all(DEFAULT_WORKSPACE_ID)).find((r) => r.id === ruleId);
  if (!rule) throw new Error(`自動化ルールが見つかりません: ${ruleId}`);

  if (rule.backend === "api_key") {
    return invoke<AutomationRun>("automation_run", { ruleId, memoId });
  }

  const overrides = parseBrowserProfileOverrides(
    await settings.get(DEFAULT_WORKSPACE_ID, BROWSER_PROFILES_SETTING_KEY),
  );
  const profile: BrowserProfile = resolveBrowserProfile(
    rule.backendConfig.provider,
    overrides,
  );

  const prompt = await invoke<string>("automation_render", { ruleId, memoId });
  const text = await invoke<string>("browser_agent_run", { profile, prompt });
  return invoke<AutomationRun>("automation_record", { ruleId, memoId, text });
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
