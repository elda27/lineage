import Database from "@tauri-apps/plugin-sql";
import { invoke } from "@tauri-apps/api/core";
import { join, localDataDir } from "@tauri-apps/api/path";

import { ListMemos, DEFAULT_MEMO_LIMIT } from "@core/features/memo/ListMemos";
import { SuggestMetaTags, DEFAULT_SUGGESTION_LIMIT } from "@core/features/meta/SuggestMetaTags";
import type {
  AutomationRule,
  AutomationRulePatch,
  AutomationRuleInput,
  AutomationRun,
} from "@core/domain/automation/AutomationRule";
import { builtinTagLabels } from "@core/domain/memo/BuiltinTag";
import {
  BROWSER_PROFILES_SETTING_KEY,
  parseBrowserProfileOverrides,
  resolveBrowserProfile,
  type BrowserProfile,
} from "@core/domain/automation/BrowserProfile";
import {
  AGENT_SKILL_PREFERENCE_KEY,
  parseAgentSkillPreference,
  type AgentSkillPreference,
} from "@core/domain/skill/AgentSkill";
import { InstallAgentSkills, SyncAgentSkills } from "@core/features/skill/SyncAgentSkills";
import { TauriAgentSkillStore } from "@core/infra/agent/TauriAgentSkillStore";
import {
  SqliteAutomationRuleRepository,
  SqliteAutomationRunRepository,
} from "@core/infra/persistence/sqlite/SqliteAutomationRepository";
import { SqliteSettingsRepository } from "@core/infra/persistence/sqlite/SqliteSettingsRepository";
import { SqliteMemoRepository } from "@core/infra/persistence/sqlite/SqliteMemoRepository";
import { SqliteMemoStateRepository } from "@core/infra/persistence/sqlite/SqliteMemoStateRepository";
import { SqliteMetaTagRepository } from "@core/infra/persistence/sqlite/SqliteMetaTagRepository";
import type { ApplicationPort } from "./ApplicationPort";
import { SqliteTagRepository } from "@core/infra/persistence/sqlite/SqliteTagRepository";
import { applyLocalMutationOrThrow } from "./localMutation";

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
 * DB の書き込みはすべて Rust の差分 mutation API に委ねる。
 * WebView は plugin-sql の読み出し権限だけを持ち、SQL の execute は呼ばない。
 */
export async function createLocalAppClient(): Promise<ApplicationPort> {
  const db = await Database.load(`sqlite:${await minosDatabasePath()}`);
  const memos = new SqliteMemoRepository(db);
  const memoStates = new SqliteMemoStateRepository(db);
  const metaTags = new SqliteMetaTagRepository(db);
  const automationRules = new SqliteAutomationRuleRepository(db);
  const automationRuns = new SqliteAutomationRunRepository(db);
  const settings = new SqliteSettingsRepository(db);
  const tags = new SqliteTagRepository(db);
  // skill の配置はホーム以下のファイル操作なので、DB ではなく Rust 側に委ねる。
  const agentSkills = new TauriAgentSkillStore(invoke);

  return {
    listTags: () => tags.all(DEFAULT_WORKSPACE_ID),
    updateTag: (id, patch) => applyLocalMutationOrThrow({ type: "tag_patch", tagId: id, patch }),
    deleteTag: (id) => applyLocalMutationOrThrow({ type: "tag_delete", tagId: id }),
    listMemos: (limit = DEFAULT_MEMO_LIMIT) =>
      new ListMemos(memos, memoStates).execute(DEFAULT_WORKSPACE_ID, limit),

    setMemoDone: (memoId, done) =>
      applyLocalMutationOrThrow({
        type: "memo_state_patch",
        memoId,
        patch: { done },
      }),

    setMemoArchived: (memoId, archived) =>
      applyLocalMutationOrThrow({
        type: "memo_state_patch",
        memoId,
        patch: { archived },
      }),

    trashMemo: (memoId) =>
      applyLocalMutationOrThrow({
        type: "memo_state_patch",
        memoId,
        patch: { trashed: true },
      }),

    archiveCompletedTasks: () =>
      applyLocalMutationOrThrow({
        type: "archive_completed_tasks",
        labels: builtinTagLabels("task"),
      }),

    suggestMetaTags: (query, limit = DEFAULT_SUGGESTION_LIMIT) =>
      new SuggestMetaTags(metaTags).execute(DEFAULT_WORKSPACE_ID, query, limit),

    // ローカル接続の保存先は利用者のディスクそのもので、割り当て上限が存在しない。
    // 使用量メーターはクォータのあるクラウド接続だけの関心事なので null を返す（＝非表示）。
    storageUsage: async () => null,

    // ローカル接続は認証なしの単一利用者で、名乗るアカウントが存在しない。
    // アカウント欄は認証のあるクラウド接続だけの関心事なので null を返す（＝非表示）。
    currentAccount: async () => null,

    listAutomationRules: () => automationRules.all(DEFAULT_WORKSPACE_ID),

    createAutomationRule: (input: Omit<AutomationRuleInput, "id">) =>
      applyLocalMutationOrThrow({ type: "automation_rule_create", input }),

    updateAutomationRule: (id: string, patch: AutomationRulePatch) =>
      applyLocalMutationOrThrow({ type: "automation_rule_patch", ruleId: id, patch }),

    deleteAutomationRule: (id: string) =>
      applyLocalMutationOrThrow({ type: "automation_rule_delete", ruleId: id }),

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

    syncAgentSkills: () => new SyncAgentSkills(agentSkills).execute(),

    installAgentSkills: (targetIds: string[]) =>
      new InstallAgentSkills(agentSkills).execute(targetIds),

    agentSkillPreference: async () =>
      parseAgentSkillPreference(
        await settings.get(DEFAULT_WORKSPACE_ID, AGENT_SKILL_PREFERENCE_KEY),
      ),

    saveAgentSkillPreference: (preference: AgentSkillPreference) =>
      applyLocalMutationOrThrow({
        type: "setting_set",
        key: AGENT_SKILL_PREFERENCE_KEY,
        value: JSON.stringify(preference),
      }),

    saveBrowserProfileOverrides: (overrides) =>
      applyLocalMutationOrThrow({
        type: "setting_set",
        key: BROWSER_PROFILES_SETTING_KEY,
        value: JSON.stringify(overrides),
      }),
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
  const profile: BrowserProfile = resolveBrowserProfile(rule.backendConfig.provider, overrides);

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
