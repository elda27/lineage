import { useState, type FormEvent } from "react";

import {
  PROMPT_PLACEHOLDERS,
  metaConditionText,
  validateRule,
  type AutomationRule,
  type AutomationRuleInput,
  type BackendKind,
  type MetaCondition,
  type TriggerKind,
} from "@core/domain/automation/AutomationRule";
import {
  API_KEY_PROVIDERS,
  BROWSER_PROVIDERS,
  providerLabel,
} from "@core/domain/automation/BrowserProfile";
import { Icon, primaryButton, secondaryButton, tagChip } from "@/shared/ui/kit";
import { MetaConditionInput } from "./MetaConditionInput";

const field = "flex flex-col gap-1.5";
const label = "text-[11px] font-semibold text-[#6e706a]";
const hint = "text-[10px] text-[#96978f]";
const input =
  "rounded-lg border border-[#deded8] px-3 py-2 text-[12px] outline-none focus:border-[#9a92cc]";

/** 新規作成の初期値。まずは手動実行の APIキー方式にしておく（最も試しやすい）。 */
function emptyRule(): AutomationRuleInput {
  return {
    name: "",
    description: null,
    prompt: "次の記録を3行で要約して:\n{{memo.body}}",
    backend: "api_key",
    backendConfig: { provider: "anthropic", model: null, effort: null },
    triggerKind: "manual",
    trigger: { metas: [], cron: null },
    enabled: true,
  };
}

/** 編集中のルールを `AutomationRuleInput` に整える。 */
function toInput(rule: AutomationRule): AutomationRuleInput {
  return {
    id: rule.id,
    name: rule.name,
    description: rule.description,
    prompt: rule.prompt,
    backend: rule.backend,
    backendConfig: rule.backendConfig,
    triggerKind: rule.triggerKind,
    trigger: rule.trigger,
    enabled: rule.enabled,
  };
}

/**
 * 自動化ルールの作成・編集。
 *
 * バックエンドを切り替えると提供元の選択肢も変わる（APIキー方式は HTTP API を
 * 持つ提供元、ブラウザ方式は Web UI を持つ提供元）。取り違えると実行時まで
 * 気づけないので、選択肢の側で先に絞る。
 */
export function RuleEditor({
  rule,
  onSave,
  onClose,
}: {
  /** 既存ルールの編集なら渡す。未指定なら新規作成。 */
  rule?: AutomationRule;
  onSave: (input: AutomationRuleInput) => Promise<void>;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<AutomationRuleInput>(() =>
    rule ? toInput(rule) : emptyRule(),
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const patch = (changes: Partial<AutomationRuleInput>) =>
    setDraft((current) => ({ ...current, ...changes }));

  const providers = draft.backend === "api_key" ? API_KEY_PROVIDERS : BROWSER_PROVIDERS;

  const changeBackend = (backend: BackendKind) => {
    // 提供元が新しいバックエンドで使えないなら、その先頭に付け替える。
    const available = backend === "api_key" ? API_KEY_PROVIDERS : BROWSER_PROVIDERS;
    const provider = available.includes(draft.backendConfig.provider)
      ? draft.backendConfig.provider
      : available[0];
    patch({ backend, backendConfig: { ...draft.backendConfig, provider } });
  };

  const changeTrigger = (triggerKind: TriggerKind) => {
    patch({
      triggerKind,
      trigger: {
        ...draft.trigger,
        // スケジュールに切り替えたら cron の入力欄を出す。既定は毎朝9時。
        cron:
          triggerKind === "schedule" ? (draft.trigger.cron ?? "0 0 9 * * *") : draft.trigger.cron,
      },
    });
  };

  const setConditions = (metas: MetaCondition[]) => patch({ trigger: { ...draft.trigger, metas } });

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const problem = validateRule(draft);
    if (problem) {
      setError(problem);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await onSave(draft);
      onClose();
    } catch (failure) {
      setError(`保存できませんでした: ${failure}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-30 flex animate-[fade_0.18s] items-center justify-center bg-[#27272245] p-6"
      onMouseDown={(event) => event.target === event.currentTarget && onClose()}
    >
      <form
        onSubmit={submit}
        className="flex max-h-full w-[min(680px,100%)] flex-col overflow-auto rounded-[14px] bg-white p-7 shadow-[0_20px_60px_#0003]"
      >
        <h2 className="mb-1 font-serif text-[22px] font-normal">
          {rule ? "ルールを編集" : "新しいルール"}
        </h2>
        <p className="mb-6 text-[11px] text-muted">
          記録とプロンプトを生成AIに渡し、結果を新しい記録として残します。
        </p>

        <div className="flex flex-col gap-[18px]">
          <div className={field}>
            <label className={label} htmlFor="rule-name">
              名前
            </label>
            <input
              id="rule-name"
              className={input}
              autoFocus
              value={draft.name}
              onChange={(event) => patch({ name: event.target.value })}
              placeholder="タスクを3行で要約"
            />
          </div>

          <div className={field}>
            <label className={label} htmlFor="rule-prompt">
              プロンプト
            </label>
            <textarea
              id="rule-prompt"
              className={`${input} min-h-[130px] resize-y font-mono leading-[1.7]`}
              value={draft.prompt}
              onChange={(event) => patch({ prompt: event.target.value })}
            />
            <div className="flex flex-wrap gap-1.5">
              {PROMPT_PLACEHOLDERS.map((placeholder) => (
                <button
                  type="button"
                  key={placeholder.token}
                  className={`${tagChip} cursor-pointer font-mono hover:bg-[#e5e4e0]`}
                  title={placeholder.description}
                  onClick={() => patch({ prompt: `${draft.prompt}${placeholder.token}` })}
                >
                  {placeholder.token}
                </button>
              ))}
            </div>
            <p className={hint}>
              クリックで差し込めます。差し込みが無いプロンプトは、そのまま定型の指示になります。
            </p>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className={field}>
              <label className={label} htmlFor="rule-backend">
                実行方法
              </label>
              <select
                id="rule-backend"
                className={input}
                value={draft.backend}
                onChange={(event) => changeBackend(event.target.value as BackendKind)}
              >
                <option value="api_key">APIキー（設定した鍵で直接呼ぶ）</option>
                <option value="browser">ブラウザ（Web版にログインして使う）</option>
              </select>
            </div>

            <div className={field}>
              <label className={label} htmlFor="rule-provider">
                提供元
              </label>
              <select
                id="rule-provider"
                className={input}
                value={draft.backendConfig.provider}
                onChange={(event) =>
                  patch({
                    backendConfig: { ...draft.backendConfig, provider: event.target.value },
                  })
                }
              >
                {providers.map((provider) => (
                  <option value={provider} key={provider}>
                    {providerLabel(provider)}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {draft.backend === "browser" && (
            <p className="rounded-lg bg-[#fdf6e8] px-3 py-2.5 text-[10px] leading-[1.7] text-[#8a6d3b]">
              ブラウザ方式は、提供元の Web
              画面をアプリ内で開いて自動操作します。多くのサービスでは自動操作が
              利用規約で禁じられており、アカウント停止の恐れがあります。初回はウィンドウが開いたら
              手動でログインしてください。
            </p>
          )}

          {draft.backend === "api_key" && (
            <div className="grid grid-cols-2 gap-4">
              <div className={field}>
                <label className={label} htmlFor="rule-model">
                  モデル（省略可）
                </label>
                <input
                  id="rule-model"
                  className={input}
                  value={draft.backendConfig.model ?? ""}
                  placeholder="claude-opus-5"
                  onChange={(event) =>
                    patch({
                      backendConfig: {
                        ...draft.backendConfig,
                        model: event.target.value || null,
                      },
                    })
                  }
                />
              </div>
              <div className={field}>
                <label className={label} htmlFor="rule-effort">
                  思考の深さ（省略可）
                </label>
                <select
                  id="rule-effort"
                  className={input}
                  value={draft.backendConfig.effort ?? ""}
                  onChange={(event) =>
                    patch({
                      backendConfig: {
                        ...draft.backendConfig,
                        effort: event.target.value || null,
                      },
                    })
                  }
                >
                  <option value="">既定</option>
                  {["low", "medium", "high", "xhigh", "max"].map((effort) => (
                    <option value={effort} key={effort}>
                      {effort}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          )}

          <div className={field}>
            <label className={label} htmlFor="rule-trigger">
              きっかけ
            </label>
            <select
              id="rule-trigger"
              className={input}
              value={draft.triggerKind}
              onChange={(event) => changeTrigger(event.target.value as TriggerKind)}
            >
              <option value="manual">手動（記録のActionボタンから実行）</option>
              <option value="meta_match">メタ情報マッチ（条件に合う記録を自動で処理）</option>
              <option value="schedule">スケジュール（時刻で実行）</option>
            </select>
          </div>

          {draft.triggerKind === "schedule" && (
            <div className={field}>
              <label className={label} htmlFor="rule-cron">
                cron 式
              </label>
              <input
                id="rule-cron"
                className={`${input} font-mono`}
                value={draft.trigger.cron ?? ""}
                onChange={(event) =>
                  patch({ trigger: { ...draft.trigger, cron: event.target.value } })
                }
                placeholder="0 0 9 * * *"
              />
              <p className={hint}>
                先頭が秒です（`0 0 9 * * *` で毎日9時）。実行には agentos の定期起動が必要です。
              </p>
            </div>
          )}

          <div className={field}>
            <span className={label}>対象の絞り込み</span>
            <MetaConditionInput conditions={draft.trigger.metas} onChange={setConditions} />
            <p className={hint}>
              {draft.trigger.metas.length === 0
                ? "条件なし。すべての記録が対象になります。"
                : `${draft.trigger.metas.map(metaConditionText).join(" と ")} の両方が付いた記録だけが対象です。`}
            </p>
          </div>
        </div>

        {error && (
          <p className="mt-5 rounded-lg bg-[#fdeaea] px-3 py-2.5 text-[11px] text-[#a33]">
            {error}
          </p>
        )}

        <div className="mt-7 flex justify-end gap-2">
          <button type="button" className={secondaryButton} onClick={onClose}>
            キャンセル
          </button>
          <button className={primaryButton} disabled={saving}>
            {saving ? (
              "保存中…"
            ) : (
              <>
                <Icon name="check" size={15} />
                保存する
              </>
            )}
          </button>
        </div>
      </form>
    </div>
  );
}
