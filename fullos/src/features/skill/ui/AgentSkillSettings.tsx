import { useCallback, useEffect, useState } from "react";

import type { AgentSkillStatus } from "@core/domain/skill/AgentSkill";
import { LINEAGE_SKILL_VERSION } from "@core/domain/skill/LineageSkill";
import { appClient } from "@/shared/api/appClient";
import { SettingRow, smallPrimaryButton, toggleKnob, toggleTrack } from "@/shared/ui/kit";

/**
 * 設定画面の skill セクション。
 *
 * 起動時の確認は「今後表示しない」で止められるので、そのあとに追加したくなったときの
 * 入口がここになる。止めていても更新は自動で走るため、ここに出るのは
 * 「まだ置いていない」か「最新が置いてある」かのどちらかになる。
 */
export function AgentSkillSettings() {
  const [statuses, setStatuses] = useState<AgentSkillStatus[]>([]);
  const [ask, setAsk] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const client = await appClient();
    const [sync, preference] = await Promise.all([
      client.syncAgentSkills(),
      client.agentSkillPreference(),
    ]);
    setStatuses(sync.statuses);
    setAsk(!preference.suppressed);
  }, []);

  useEffect(() => {
    load().catch((failure) => setError(`${failure}`));
  }, [load]);

  const install = async (targetId: string) => {
    setBusy(targetId);
    setError(null);
    try {
      const client = await appClient();
      await client.installAgentSkills([targetId]);
      await load();
    } catch (failure) {
      setError(`${failure}`);
    } finally {
      setBusy(null);
    }
  };

  const changeAsk = async (next: boolean) => {
    setAsk(next);
    try {
      const client = await appClient();
      await client.saveAgentSkillPreference({ suppressed: !next });
    } catch (failure) {
      setError(`${failure}`);
    }
  };

  return (
    <section className="mt-[30px]">
      <h2 className="mb-[3px] text-[13px] font-bold">エージェント skill</h2>
      <p className="mb-2.5 text-[10px] text-muted">
        エージェント CLI に記録の扱い方（SKILL.md v{LINEAGE_SKILL_VERSION}）を配置します。
        配置済みのものは起動時に自動で最新へ更新されます。
      </p>
      <div className="overflow-hidden rounded-[10px] border border-line bg-white">
        <SettingRow
          title="起動時に追加を確認する"
          desc="skill を持っていないエージェントが見つかったときにダイアログを出します"
        >
          <button
            role="switch"
            aria-checked={ask}
            className={`${toggleTrack} ${ask ? "bg-[#7063b6]" : "bg-[#d8d8d2]"}`}
            onClick={() => void changeAsk(!ask)}
          >
            <i className={`${toggleKnob} ${ask ? "translate-x-4" : ""}`} />
          </button>
        </SettingRow>
        {statuses.map((status) => (
          <SettingRow
            key={status.target.id}
            title={status.target.label}
            desc={
              status.agentPresent
                ? status.path
                : `${status.target.label} は見つかりませんでした（${status.path}）`
            }
          >
            {status.state === "current" ? (
              <span className="text-[10px] text-[#8f918a]">
                v{status.installedVersion} 配置済み
              </span>
            ) : (
              <button
                className={smallPrimaryButton}
                disabled={busy === status.target.id}
                onClick={() => void install(status.target.id)}
              >
                {busy === status.target.id ? "配置中…" : "追加"}
              </button>
            )}
          </SettingRow>
        ))}
        {error && <p className="px-[18px] pb-[15px] text-[10px] text-[#a35]">{error}</p>}
      </div>
    </section>
  );
}
