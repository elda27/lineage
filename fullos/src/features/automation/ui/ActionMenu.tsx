import { useEffect, useRef, useState } from "react";

import {
  backendLabel,
  statusLabel,
  type AutomationRule,
  type AutomationRun,
} from "../../core/domain/automation/AutomationRule";
import { appClient } from "../app-client/appClient";
import { Icon, tagChip } from "../ui";

type Phase =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "candidates"; rules: AutomationRule[] }
  | { kind: "running"; rule: AutomationRule }
  | { kind: "done"; run: AutomationRun }
  | { kind: "error"; message: string };

/**
 * 記録の隣に出す「Action」。
 *
 * 押すとその記録に当てられるルールを候補として出し、選ぶと実行する。
 * 候補の判定は実行エンジン（agentos）に問い合わせる。画面側でも同じ判定はできるが、
 * 二重に実装すると条件の解釈がずれたときに「出るのに実行できない」候補ができる。
 */
export function ActionMenu({
  memoId,
  onFinished,
}: {
  memoId: string;
  /** 実行が終わったら呼ばれる。呼び出し側が結果を読み直すために使う。 */
  onFinished?: (run: AutomationRun) => void;
}) {
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const container = useRef<HTMLDivElement>(null);

  // 実行中は開いたままにする。数分かかることがあり、外側を触っただけで
  // 消えると「動いているのか分からない」状態になる。
  const dismissable = phase.kind !== "running";

  useEffect(() => {
    if (phase.kind === "idle" || !dismissable) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!container.current?.contains(event.target as Node)) {
        setPhase({ kind: "idle" });
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [phase.kind, dismissable]);

  const open = async () => {
    if (phase.kind !== "idle") {
      setPhase({ kind: "idle" });
      return;
    }
    setPhase({ kind: "loading" });
    try {
      const client = await appClient();
      setPhase({ kind: "candidates", rules: await client.matchAutomationRules(memoId) });
    } catch (error) {
      setPhase({ kind: "error", message: `候補を取得できませんでした: ${error}` });
    }
  };

  const run = async (rule: AutomationRule) => {
    setPhase({ kind: "running", rule });
    try {
      const client = await appClient();
      const result = await client.runAutomation(rule.id, memoId);
      setPhase({ kind: "done", run: result });
      onFinished?.(result);
    } catch (error) {
      setPhase({ kind: "error", message: `${error}` });
    }
  };

  return (
    <div className="relative" ref={container}>
      <button
        className="flex cursor-pointer items-center gap-1 rounded-[7px] border border-[#deded8] bg-white px-2.5 py-1.5 text-[10px] font-semibold text-[#6e706a] hover:border-[#cfcec7]"
        aria-label="この記録に自動化を実行"
        aria-expanded={phase.kind !== "idle"}
        onClick={(event) => {
          // 記録の詳細を開く親のクリックと二重で反応させない。
          event.stopPropagation();
          void open();
        }}
      >
        <Icon name="sparkles" size={13} />
        Action
      </button>

      {phase.kind !== "idle" && (
        <div
          className="absolute right-0 z-20 mt-1.5 w-[280px] overflow-hidden rounded-[10px] border border-line bg-white py-1 shadow-[0_10px_30px_#2f302920]"
          onClick={(event) => event.stopPropagation()}
        >
          <Body phase={phase} onRun={run} onClose={() => setPhase({ kind: "idle" })} />
        </div>
      )}
    </div>
  );
}

function Body({
  phase,
  onRun,
  onClose,
}: {
  phase: Phase;
  onRun: (rule: AutomationRule) => void;
  onClose: () => void;
}) {
  const message = "px-3 py-2.5 text-[11px] text-[#81837c]";

  switch (phase.kind) {
    case "loading":
      return <p className={message}>候補を探しています…</p>;

    case "candidates":
      if (phase.rules.length === 0) {
        return (
          <p className={message}>
            この記録に当てられるルールがありません。自動化画面でルールを作るか、条件を見直してください。
          </p>
        );
      }
      return (
        <>
          {phase.rules.map((rule) => (
            <button
              key={rule.id}
              className="flex w-full cursor-pointer items-center gap-2.5 border-0 bg-transparent px-3 py-2 text-left text-[12px] hover:bg-[#f1f0ed]"
              onClick={() => onRun(rule)}
            >
              <Icon name="command" size={14} />
              <span className="min-w-0 flex-1 truncate font-medium">{rule.name}</span>
              <span className={tagChip}>{backendLabel(rule.backend)}</span>
            </button>
          ))}
        </>
      );

    case "running":
      return (
        <p className={message}>
          「{phase.rule.name}」を実行しています…
          {phase.rule.backend === "browser" && (
            <>
              <br />
              ブラウザのウィンドウが開きます。初回はログインしてください。
            </>
          )}
        </p>
      );

    case "done":
      return (
        <div className={message}>
          <b className="text-ink">{statusLabel(phase.run.status)}</b>
          {phase.run.error && <p className="mt-1 text-[#a35]">{phase.run.error}</p>}
          {phase.run.status === "succeeded" && (
            <p className="mt-1">結果を新しい記録として保存しました。</p>
          )}
          <button
            className="mt-2 cursor-pointer border-0 bg-transparent p-0 text-[10px] text-[#7063b6]"
            onClick={onClose}
          >
            閉じる
          </button>
        </div>
      );

    case "error":
      return (
        <div className={message}>
          <p className="text-[#a35]">{phase.message}</p>
          <button
            className="mt-2 cursor-pointer border-0 bg-transparent p-0 text-[10px] text-[#7063b6]"
            onClick={onClose}
          >
            閉じる
          </button>
        </div>
      );

    case "idle":
      return null;
  }
}
