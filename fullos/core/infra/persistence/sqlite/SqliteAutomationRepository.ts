import type {
  AutomationRule,
  AutomationRun,
  BackendConfig,
  BackendKind,
  RunStatus,
  Trigger,
  TriggerKind,
} from "../../../domain/automation/AutomationRule";
import type {
  AutomationRuleRepository,
  AutomationRunRepository,
} from "../../../domain/ports/AutomationRuleRepository";
import { selectOrEmpty, type SqlHandle } from "./SqlHandle";

type RuleRow = {
  id: string;
  workspace_id: string;
  name: string;
  description: string | null;
  prompt: string;
  backend_kind: string;
  backend_config: string;
  trigger_kind: string;
  trigger_config: string;
  enabled: number;
  created_at: string;
  updated_at: string;
};

type RunRow = {
  id: string;
  workspace_id: string;
  rule_id: string;
  source_document_id: string;
  result_document_id: string | null;
  status: string;
  backend_kind: string;
  error: string | null;
  started_at: string;
  finished_at: string | null;
};

/** ローカル SQLite の `automation_rules` を読み出す。 */
export class SqliteAutomationRuleRepository implements AutomationRuleRepository {
  constructor(private readonly db: SqlHandle) {}

  async all(workspaceId: string): Promise<AutomationRule[]> {
    const rows = await selectOrEmpty<RuleRow>(
      this.db,
      `SELECT id, workspace_id, name, description, prompt, backend_kind, backend_config,
              trigger_kind, trigger_config, enabled, created_at, updated_at
       FROM automation_rules WHERE workspace_id = $1 ORDER BY created_at ASC`,
      [workspaceId],
    );
    return rows.map(toRule);
  }
}

/** 実行履歴の読み出し。 */
export class SqliteAutomationRunRepository implements AutomationRunRepository {
  constructor(private readonly db: SqlHandle) {}

  async recent(workspaceId: string, limit: number): Promise<AutomationRun[]> {
    const rows = await selectOrEmpty<RunRow>(
      this.db,
      `SELECT id, workspace_id, rule_id, source_document_id, result_document_id,
              status, backend_kind, error, started_at, finished_at
       FROM automation_runs WHERE workspace_id = $1
       ORDER BY started_at DESC LIMIT $2`,
      [workspaceId, limit],
    );
    return rows.map(toRun);
  }
}

function toRule(row: RuleRow): AutomationRule {
  return {
    id: row.id,
    workspaceId: row.workspace_id,
    name: row.name,
    description: row.description,
    prompt: row.prompt,
    backend: row.backend_kind as BackendKind,
    backendConfig: parseJson<BackendConfig>(row.backend_config, {
      provider: "anthropic",
      model: null,
      effort: null,
    }),
    triggerKind: row.trigger_kind as TriggerKind,
    trigger: parseJson<Trigger>(row.trigger_config, { metas: [], cron: null }),
    enabled: row.enabled !== 0,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

function toRun(row: RunRow): AutomationRun {
  return {
    id: row.id,
    workspaceId: row.workspace_id,
    ruleId: row.rule_id,
    sourceDocumentId: row.source_document_id,
    resultDocumentId: row.result_document_id,
    status: row.status as RunStatus,
    backend: row.backend_kind as BackendKind,
    error: row.error,
    startedAt: row.started_at,
    finishedAt: row.finished_at,
  };
}

/**
 * 壊れた JSON でも一覧全体を落とさない。
 *
 * 1件のルールの設定が読めないだけで自動化画面が真っ白になるより、
 * 既定値で表示して利用者が直せるほうがよい。
 */
function parseJson<T>(text: string, fallback: T): T {
  try {
    return { ...fallback, ...(JSON.parse(text) as T) };
  } catch {
    return fallback;
  }
}
