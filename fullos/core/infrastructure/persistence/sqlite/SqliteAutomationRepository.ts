import type {
  AutomationRule,
  AutomationRuleInput,
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

/**
 * ローカル SQLite の `automation_rules` を読み書きする。
 *
 * ここが fullos で唯一の書き込み先になる。lineage(links) に触る操作は含まない
 * （それは agentos が同一トランザクションで確定させる）。
 * SQL は D1 版と共通にできる形にしてある（実行ハンドルだけが違う）。
 */
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

  async save(workspaceId: string, input: AutomationRuleInput): Promise<AutomationRule> {
    const now = new Date().toISOString();
    const rule: AutomationRule = {
      id: input.id ?? crypto.randomUUID(),
      workspaceId,
      name: input.name,
      description: input.description,
      prompt: input.prompt,
      backend: input.backend,
      backendConfig: input.backendConfig,
      triggerKind: input.triggerKind,
      trigger: input.trigger,
      enabled: input.enabled,
      // 新規なら作成日時も now。更新では下の ON CONFLICT が既存の値を残す。
      createdAt: now,
      updatedAt: now,
    };

    await this.db.execute(
      `INSERT INTO automation_rules
           (id, workspace_id, name, description, prompt, backend_kind, backend_config,
            trigger_kind, trigger_config, enabled, created_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
       ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           prompt = excluded.prompt,
           backend_kind = excluded.backend_kind,
           backend_config = excluded.backend_config,
           trigger_kind = excluded.trigger_kind,
           trigger_config = excluded.trigger_config,
           enabled = excluded.enabled,
           updated_at = excluded.updated_at`,
      [
        rule.id,
        rule.workspaceId,
        rule.name,
        rule.description,
        rule.prompt,
        rule.backend,
        JSON.stringify(toBackendConfigJson(rule.backendConfig)),
        rule.triggerKind,
        JSON.stringify(toTriggerJson(rule.trigger)),
        rule.enabled ? 1 : 0,
        rule.createdAt,
        rule.updatedAt,
      ],
    );

    return rule;
  }

  async remove(id: string): Promise<void> {
    // 実行履歴は消さない。ルールが無くなっても「いつ何が作られたか」は残す必要がある
    // （lineage 側に結果 document への link が残っているため）。
    await this.db.execute("DELETE FROM automation_rules WHERE id = $1", [id]);
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
 * 保存する JSON は Rust 側（serde）が読む形に合わせる。
 *
 * `undefined` はキーごと消えてしまい serde が既定値を使えなくなるので、
 * 明示的に null にしてから書き出す。
 */
function toBackendConfigJson(config: BackendConfig) {
  return {
    provider: config.provider,
    model: config.model ?? null,
    effort: config.effort ?? null,
  };
}

function toTriggerJson(trigger: Trigger) {
  return {
    metas: trigger.metas.map((meta) => ({ label: meta.label, value: meta.value ?? null })),
    cron: trigger.cron ?? null,
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
