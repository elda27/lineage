import { useState } from "react";

import {
  backendLabel,
  ruleSummary,
  statusLabel,
  triggerLabel,
  type AutomationRule,
  type AutomationRun,
  type RunStatus,
} from "@core/domain/automation/AutomationRule";
import { relativeTime } from "@/shared/format";
import {
  cardSurface,
  eyebrow,
  Icon,
  primaryButton,
  quietButton,
  serifTitle,
  standardPage,
  tagChip,
  toggleKnob,
  toggleTrack,
} from "@/shared/ui/kit";
import { RuleEditor } from "./RuleEditor";
import { useAutomationRules, useAutomationRuns } from "@/features/automation/service/useAutomation";

type Tab = "rules" | "runs";

/** 実行状態ごとの色。成否がひと目で分かるようにする。 */
const STATUS_LOOK: Record<RunStatus, string> = {
  running: "bg-[#eef1f7] text-[#5a6a8a]",
  succeeded: "bg-[#ebf3ef] text-[#578170]",
  failed: "bg-[#fdeaea] text-[#a35]",
  refused: "bg-[#fdf6e8] text-[#8a6d3b]",
};

/**
 * 自動化画面（docs/ui.md「自動化画面」）。
 *
 * ルールの管理と実行履歴の2枚。実行そのものは記録側の Action ボタンから始まるので、
 * ここには「何を自動化するか」の設定だけを置く。
 */
export function AutomationPage() {
  const { rules, status, save, remove, toggle } = useAutomationRules();
  const { runs, reload: reloadRuns } = useAutomationRuns();
  const [tab, setTab] = useState<Tab>("rules");
  const [editing, setEditing] = useState<AutomationRule | "new" | null>(null);

  const enabledCount = rules.filter((rule) => rule.enabled).length;

  return (
    <div className={standardPage}>
      <div className="mb-8 flex items-end justify-between">
        <div>
          <p className={eyebrow}>WORKFLOWS</p>
          <h1 className={`${serifTitle} mb-[9px] text-[34px]`}>自動化</h1>
          <p className="text-[13px] text-muted">記録に基づくルールで、いつもの作業を軽くします。</p>
        </div>
        <button className={primaryButton} onClick={() => setEditing("new")}>
          <Icon name="plus" />
          新しいルール
        </button>
      </div>

      <div className="mb-[35px] flex items-center gap-5 rounded-[14px] bg-[#37353c] px-7 py-[27px] text-white shadow-[0_8px_30px_#34303916]">
        <span className="grid h-12 w-12 place-items-center rounded-xl bg-[#ffffff12] text-[#c8c0f1]">
          <Icon name="sparkles" size={26} />
        </span>
        <div className="flex-1">
          <small className="text-[9px] tracking-[0.15em] text-[#a9a4bd]">LINEAGE AGENT</small>
          <h2 className="my-1 font-serif text-[21px] font-normal">記録から、次のアクションへ。</h2>
          <p className="text-[11px] text-[#bbb9c0]">
            メモに含まれる文脈やメタ情報を読み取り、あなたに代わって整理・実行します。
            結果は新しい記録として残り、元の記録から辿れます。
          </p>
        </div>
      </div>

      <div className="mb-[13px] flex items-center gap-[7px]">
        {(
          [
            ["rules", `ルール（${enabledCount}/${rules.length} 有効）`],
            ["runs", "実行履歴"],
          ] as const
        ).map(([value, text]) => (
          <button
            key={value}
            className={`cursor-pointer rounded-[7px] border border-transparent px-[13px] py-1.5 text-[11px] ${
              tab === value ? "bg-[#ecebe7] font-semibold" : "bg-transparent"
            }`}
            onClick={() => {
              setTab(value);
              if (value === "runs") void reloadRuns();
            }}
          >
            {text}
          </button>
        ))}
      </div>

      {tab === "rules" ? (
        <RuleList
          rules={rules}
          status={status}
          onEdit={setEditing}
          onToggle={toggle}
          onRemove={remove}
        />
      ) : (
        <RunList runs={runs} rules={rules} />
      )}

      {editing && (
        <RuleEditor
          rule={editing === "new" ? undefined : editing}
          onSave={save}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  );
}

function RuleList({
  rules,
  status,
  onEdit,
  onToggle,
  onRemove,
}: {
  rules: AutomationRule[];
  status: "loading" | "ready" | "error";
  onEdit: (rule: AutomationRule) => void;
  onToggle: (rule: AutomationRule) => Promise<void>;
  onRemove: (id: string) => Promise<void>;
}) {
  if (status === "loading") {
    return <Empty icon="clock" title="ルールを読み込んでいます…" />;
  }
  if (status === "error") {
    return (
      <Empty
        icon="inbox"
        title="ルールを読み込めませんでした"
        hint="minos のデータベース（%LOCALAPPDATA%\minos\lineage.db）を開けませんでした。"
      />
    );
  }
  if (rules.length === 0) {
    return (
      <Empty
        icon="command"
        title="まだルールがありません"
        hint="「新しいルール」から、記録に対して何をしてほしいかを書いてみましょう。"
      />
    );
  }

  return (
    <div className={cardSurface}>
      {rules.map((rule) => (
        <article
          className="flex items-center gap-[15px] border-b border-line p-[18px] last:border-b-0"
          key={rule.id}
        >
          <span className="grid h-[37px] w-[37px] place-items-center rounded-[9px] bg-[#f0eef8] text-[#7164b2]">
            <Icon name="command" />
          </span>
          <div className="min-w-0 flex-1">
            <h3 className="text-[12px] font-bold">{rule.name}</h3>
            <p className="my-1 truncate text-[10px] text-[#81837c]">{ruleSummary(rule)}</p>
            <div className="flex flex-wrap items-center gap-1.5">
              <span className={tagChip}>{triggerLabel(rule.triggerKind)}</span>
              <span className={tagChip}>{backendLabel(rule.backend)}</span>
              <span className={tagChip}>{rule.backendConfig.provider}</span>
            </div>
          </div>
          <button
            aria-label={`${rule.name}を切り替え`}
            className={`${toggleTrack} ${rule.enabled ? "bg-[#7063b6]" : "bg-[#d8d8d2]"}`}
            onClick={() => void onToggle(rule)}
          >
            <i className={`${toggleKnob} ${rule.enabled ? "translate-x-4" : ""}`} />
          </button>
          <button className={quietButton} aria-label="編集" onClick={() => onEdit(rule)}>
            <Icon name="edit" />
          </button>
          <button
            className={`${quietButton} text-[#b05d5d]`}
            aria-label="削除"
            onClick={() => {
              // 実行履歴は残す（結果の記録が lineage に載っているため）。
              if (confirm(`ルール「${rule.name}」を削除しますか？実行履歴は残ります。`)) {
                void onRemove(rule.id);
              }
            }}
          >
            <Icon name="trash" />
          </button>
        </article>
      ))}
    </div>
  );
}

function RunList({ runs, rules }: { runs: AutomationRun[]; rules: AutomationRule[] }) {
  if (runs.length === 0) {
    return (
      <Empty
        icon="clock"
        title="まだ実行されていません"
        hint="記録の「Action」ボタンから実行すると、ここに履歴が残ります。"
      />
    );
  }

  const nameOf = (ruleId: string) =>
    rules.find((rule) => rule.id === ruleId)?.name ?? "(削除されたルール)";

  return (
    <div className={cardSurface}>
      {runs.map((run) => (
        <article
          className="flex items-start gap-[15px] border-b border-line p-[18px] last:border-b-0"
          key={run.id}
        >
          <span
            className={`rounded-[5px] px-2 py-1 text-[10px] font-semibold ${STATUS_LOOK[run.status]}`}
          >
            {statusLabel(run.status)}
          </span>
          <div className="min-w-0 flex-1">
            <h3 className="text-[12px] font-bold">{nameOf(run.ruleId)}</h3>
            {run.error && <p className="my-1 text-[10px] leading-[1.6] text-[#a35]">{run.error}</p>}
            <small className="text-[9px] text-[#a3a49e]">
              {relativeTime(run.startedAt)} · {backendLabel(run.backend)}
            </small>
          </div>
        </article>
      ))}
    </div>
  );
}

function Empty({
  icon,
  title,
  hint,
}: {
  icon: "clock" | "inbox" | "command";
  title: string;
  hint?: string;
}) {
  return (
    <div className="p-[75px] text-center text-[#999a94]">
      <Icon name={icon} size={30} />
      <h3 className="mt-[1em] mb-1 text-sm font-bold text-[#555]">{title}</h3>
      {hint && <p className="my-[1em] text-[11px]">{hint}</p>}
    </div>
  );
}
