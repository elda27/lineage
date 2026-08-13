/**
 * 自動化ルールを fullos 側から見たときの姿。
 *
 * 実体は `automation_rules` テーブルの1行で、Rust 側（lineage-core の
 * `domain/automation.rs`）と同じ形を JSON 越しに受け取る。
 *
 * 実行と lineage への追記はここには無い。それは agentos（Rust）1本に寄せてあり、
 * fullos は「どのルールをどの記録に当てるか」を決めるところまでを受け持つ
 * （docs/concept/MINIMAL_ARCHITECTURE.md「4. Lineage の真正性担保」）。
 *
 * ここは domain なので DB / Tauri / fetch には一切依存しない。
 */

import type { MetaAssignment } from "../memo/Memo";

/** 自動化の結果につく document_type。記録（`memo`）と混ざらないよう分けてある。 */
export const DOCUMENT_TYPE_AUTOMATION_RESULT = "automation_result";

/** どこで推論を実行するか。 */
export type BackendKind =
  /** ローカルに置いた API キーで、提供元の HTTP API を直接呼ぶ。 */
  | "api_key"
  /** ブラウザ（WebView）上の AI にプロンプトを貼り付けて、応答を画面から読み取る。 */
  | "browser";

/** 何をきっかけに実行するか。 */
export type TriggerKind =
  /** 利用者が明示的に実行したときだけ動く。 */
  | "manual"
  /** 条件に合う記録が現れたら動く。 */
  | "meta_match"
  /** cron の時刻で動く。 */
  | "schedule";

/** 実行1回の状態。 */
export type RunStatus = "running" | "succeeded" | "failed" | "refused";

/** メタ情報1件ぶんの条件。`value` が null ならラベルの一致だけを見る。 */
export type MetaCondition = {
  label: string;
  value: string | null;
};

/** バックエンドの設定。 */
export type BackendConfig = {
  /** 提供元の識別子（資格情報ストアの account 名にもなる）。例: `anthropic`。 */
  provider: string;
  /** api_key のときのモデル ID。null なら提供元ごとの既定。 */
  model: string | null;
  /** api_key のときの思考の深さ。 */
  effort: string | null;
};

/** 実行のきっかけ。 */
export type Trigger = {
  /** 対象を絞り込むメタ情報。空なら「すべての記録」。 */
  metas: MetaCondition[];
  /** trigger_kind が `schedule` のときの cron 式。 */
  cron: string | null;
};

/** 自動化ルール1件。 */
export type AutomationRule = {
  id: string;
  workspaceId: string;
  name: string;
  description: string | null;
  prompt: string;
  backend: BackendKind;
  backendConfig: BackendConfig;
  triggerKind: TriggerKind;
  trigger: Trigger;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
};

/** 新規作成・更新でユーザが決める部分。id と日時は保存側が埋める。 */
export type AutomationRuleInput = Omit<
  AutomationRule,
  "id" | "workspaceId" | "createdAt" | "updatedAt"
> & {
  /** 既存ルールの更新なら id を渡す。新規なら省略。 */
  id?: string;
};

/** 実行1回の記録。 */
export type AutomationRun = {
  id: string;
  workspaceId: string;
  ruleId: string;
  sourceDocumentId: string;
  resultDocumentId: string | null;
  status: RunStatus;
  backend: BackendKind;
  error: string | null;
  startedAt: string;
  finishedAt: string | null;
};

/** プロンプトに書ける差し込み。式や条件分岐は持たない（規則は Rust 側と同一）。 */
export const PROMPT_PLACEHOLDERS = [
  { token: "{{memo.title}}", description: "記録のタイトル（本文1行目）" },
  { token: "{{memo.body}}", description: "本文そのまま" },
  { token: "{{memo.metas}}", description: "#タスク #app=chrome.exe のような文字列" },
  { token: "{{now}}", description: "実行時刻" },
] as const;

/** バックエンドの表示名。 */
export function backendLabel(backend: BackendKind): string {
  return backend === "api_key" ? "APIキー" : "ブラウザ";
}

/** トリガの表示名。 */
export function triggerLabel(kind: TriggerKind): string {
  switch (kind) {
    case "manual":
      return "手動";
    case "meta_match":
      return "メタ情報マッチ";
    case "schedule":
      return "スケジュール";
  }
}

/** 実行状態の表示名。 */
export function statusLabel(status: RunStatus): string {
  switch (status) {
    case "running":
      return "実行中";
    case "succeeded":
      return "成功";
    case "failed":
      return "失敗";
    case "refused":
      return "拒否";
  }
}

/**
 * 一覧に出す「どういうルールか」の1行説明。
 *
 * description が書かれていればそれを使う。書かれていないルールも多いので、
 * 空のまま出さずトリガと条件から組み立てる。
 */
export function ruleSummary(rule: AutomationRule): string {
  if (rule.description?.trim()) return rule.description;

  const conditions = rule.trigger.metas.map(metaConditionText).join(" ");
  switch (rule.triggerKind) {
    case "manual":
      return conditions ? `${conditions} の記録に手動で実行` : "選んだ記録に手動で実行";
    case "meta_match":
      return conditions ? `${conditions} が付いた記録を自動で処理` : "すべての記録を自動で処理";
    case "schedule":
      return `${rule.trigger.cron ?? "(未設定)"} に${conditions ? ` ${conditions} を` : ""}実行`;
  }
}

/** `#ラベル` / `#ラベル=値` の表示。 */
export function metaConditionText(condition: MetaCondition): string {
  return condition.value ? `#${condition.label}=${condition.value}` : `#${condition.label}`;
}

/**
 * ルールが記録を対象にとるか。
 *
 * 条件は AND で、すべて満たしたときだけ一致。条件が空なら「すべての記録」。
 * 判定の本体は agentos（Rust の `matches`）にあり、これはその手元での予告表示用。
 * 実行時の可否は必ず Rust 側の判定に従う。
 */
export function matchesMemo(rule: AutomationRule, metas: MetaAssignment[]): boolean {
  if (!rule.enabled) return false;
  return rule.trigger.metas.every((condition) =>
    metas.some(
      (meta) =>
        meta.label === condition.label &&
        (condition.value === null || meta.value === condition.value),
    ),
  );
}

/** 保存前の検査。UI がそのまま出せる日本語のメッセージを返す（問題なければ null）。 */
export function validateRule(input: AutomationRuleInput): string | null {
  if (!input.name.trim()) return "名前を入力してください。";
  if (!input.prompt.trim()) return "プロンプトを入力してください。";
  if (!input.backendConfig.provider.trim()) return "提供元を選んでください。";
  if (input.triggerKind === "schedule" && !input.trigger.cron?.trim()) {
    return "スケジュール実行には cron 式が必要です。";
  }
  return null;
}
