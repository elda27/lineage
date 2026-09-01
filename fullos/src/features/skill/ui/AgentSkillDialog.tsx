import { useState } from "react";

import type { AgentSkillStatus } from "@core/domain/skill/AgentSkill";
import { useAgentSkills } from "@/features/skill/service/useAgentSkills";
import { Icon, primaryButton, secondaryButton } from "@/components/base";

/**
 * 起動時に出る「エージェントへ skill を追加しますか」の確認。
 *
 * 出るのは、エージェント CLI が入っていて、まだ skill を持っていないときだけ。
 * 更新は確認なしで済ませているので、ここに古い skill は出てこない。
 */
export function AgentSkillDialog() {
  const { prompt, suppress, setSuppress, install, dismiss } = useAgentSkills();

  if (prompt.kind === "idle") return null;

  return (
    <div className="fixed inset-0 z-30 grid animate-[fade_0.18s] place-items-center bg-[#27272245]">
      <div className="w-[min(520px,90vw)] rounded-[14px] bg-white p-[26px] shadow-[0_18px_50px_#0003]">
        {prompt.kind === "asking" && (
          <AskBody
            candidates={prompt.candidates}
            suppress={suppress}
            setSuppress={setSuppress}
            install={install}
            dismiss={dismiss}
          />
        )}
        {prompt.kind === "installing" && (
          <p className="text-[13px] text-muted">skill を配置しています…</p>
        )}
        {prompt.kind === "done" && <DoneBody installed={prompt.installed} close={dismiss} />}
        {prompt.kind === "error" && (
          <>
            <h2 className="mb-2 text-[17px] font-bold">skill を配置できませんでした</h2>
            <p className="mb-5 whitespace-pre-wrap text-[12px] text-[#a35]">{prompt.message}</p>
            <div className="flex justify-end">
              <button className={secondaryButton} onClick={() => void dismiss()}>
                閉じる
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function AskBody({
  candidates,
  suppress,
  setSuppress,
  install,
  dismiss,
}: {
  candidates: AgentSkillStatus[];
  suppress: boolean;
  setSuppress: (v: boolean) => void;
  install: (targetIds: string[], candidates: AgentSkillStatus[]) => Promise<void>;
  dismiss: () => Promise<void>;
}) {
  // 既定は全選択。ここに並ぶのは「入っているのに skill が無い」エージェントだけなので、
  // 利用者が個別に外す場面のほうが少ない。
  const [chosen, setChosen] = useState<string[]>(candidates.map((c) => c.target.id));
  const toggle = (id: string) =>
    setChosen((ids) => (ids.includes(id) ? ids.filter((v) => v !== id) : [...ids, id]));

  return (
    <>
      <div className="mb-[14px] flex items-center gap-3">
        <div className="grid h-[38px] w-[38px] place-items-center rounded-[10px] bg-[#eeecf7] text-[#7063b6]">
          <Icon name="sparkles" />
        </div>
        <h2 className="text-[17px] font-bold">エージェントに lineage skill を追加しますか？</h2>
      </div>
      <p className="mb-4 text-[12px] leading-relaxed text-muted">
        お使いのエージェント CLI に、記録の検索や自動化の実行に必要な手順書（SKILL.md）を
        置きます。あとから設定画面で追加・削除できます。
      </p>

      <div className="mb-4 overflow-hidden rounded-[10px] border border-line">
        {candidates.map((candidate) => (
          <label
            key={candidate.target.id}
            className="flex cursor-pointer items-center gap-3 border-b border-line px-[14px] py-3 last:border-b-0 hover:bg-[#fafaf8]"
          >
            <input
              type="checkbox"
              className="h-[15px] w-[15px] accent-[#7063b6]"
              checked={chosen.includes(candidate.target.id)}
              onChange={() => toggle(candidate.target.id)}
            />
            <span className="flex flex-col gap-[3px]">
              <b className="text-[12px]">{candidate.target.label}</b>
              <small className="font-mono text-[9px] text-[#8f918a]">{candidate.path}</small>
            </span>
          </label>
        ))}
      </div>

      <label className="mb-5 flex cursor-pointer items-center gap-2 text-[11px] text-muted">
        <input
          type="checkbox"
          className="h-[13px] w-[13px] accent-[#7063b6]"
          checked={suppress}
          onChange={(e) => setSuppress(e.target.checked)}
        />
        今後この確認を表示しない
      </label>

      <div className="flex justify-end gap-2.5">
        <button className={secondaryButton} onClick={() => void dismiss()}>
          あとで
        </button>
        <button
          className={primaryButton}
          disabled={chosen.length === 0}
          onClick={() => void install(chosen, candidates)}
        >
          追加する
        </button>
      </div>
    </>
  );
}

function DoneBody({
  installed,
  close,
}: {
  installed: AgentSkillStatus[];
  close: () => Promise<void>;
}) {
  return (
    <>
      <h2 className="mb-2 text-[17px] font-bold">skill を追加しました</h2>
      <p className="mb-4 text-[12px] text-muted">
        {installed.map((s) => s.target.label).join("、")} で使えます。
        エージェントを起動し直すと読み込まれます。
      </p>
      <div className="flex justify-end">
        <button className={primaryButton} onClick={() => void close()}>
          閉じる
        </button>
      </div>
    </>
  );
}
