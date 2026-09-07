import type { AgentSkillStore, AgentSkillWrite } from "../../domain/ports/AgentSkillStore";
import {
  AGENT_SKILL_TARGETS,
  SKILL_DOCUMENT_FILE,
  SKILL_NAME,
  SKILL_VERSION_FILE,
  skillState,
  type AgentSkillStatus,
  type AgentSkillTarget,
  type SkillManifest,
} from "../../domain/skill/AgentSkill";
import {
  LINEAGE_SKILL_VERSION,
  SKILL_INSTALLED_BY,
  renderLineageSkill,
} from "../../domain/skill/LineageSkill";

export type AgentSkillSync = {
  /** 走査した全配布先の状態（更新を反映したあとの値）。 */
  statuses: AgentSkillStatus[];
  /**
   * 追加してよいか確認したい配布先。
   *
   * エージェント CLI が入っていて、かつ skill がまだ無いものだけ。
   */
  askable: AgentSkillStatus[];
  /** 黙って最新へ入れ替えた配布先。 */
  updated: AgentSkillStatus[];
};

/**
 * 配布先を走査し、古い skill を最新へ入れ替えるユースケース。
 *
 * 「追加」と「更新」を分けているのは、利用者に確認すべきことが違うため。
 *
 * - 追加は、利用者が使っているツールの設定ディレクトリに新しいものを置く操作なので、
 *   毎回ではなくとも一度は確認が要る（呼び出し側がダイアログを出す）。
 * - 更新は、既に置くことへ同意が得られているものの中身を差し替えるだけなので、
 *   ここで黙って済ませる。古い手順書が残り続けるほうが利用者にとって害になる。
 *
 * この切り分けにより「起動時の確認を今後出さない」設定を入れても、
 * 既に入っている skill は最新に保たれる。
 */
export class SyncAgentSkills {
  constructor(
    private readonly store: AgentSkillStore,
    private readonly targets: AgentSkillTarget[] = AGENT_SKILL_TARGETS,
  ) {}

  async execute(): Promise<AgentSkillSync> {
    const scans = await this.store.scan(this.targets);
    const document = renderLineageSkill(await this.store.agentosPath());

    const statuses: AgentSkillStatus[] = this.targets.map((target) => {
      const scan = scans.find((s) => s.id === target.id);
      const installedVersion = scan?.installedVersion ?? null;
      return {
        target,
        agentPresent: scan?.agentPresent ?? false,
        installedVersion,
        state: skillState(installedVersion, LINEAGE_SKILL_VERSION),
        path: scan?.path ?? "",
      };
    });

    const outdated = statuses.filter((status) => status.state === "outdated");
    if (outdated.length > 0) {
      await this.store.write(outdated.map((status) => skillFiles(status.target, document)));
    }

    return {
      statuses: statuses.map((status) =>
        status.state === "outdated"
          ? { ...status, state: "current", installedVersion: LINEAGE_SKILL_VERSION }
          : status,
      ),
      // 未導入のエージェントの設定ディレクトリは作らない。使っていないツールの
      // 設定が勝手に増えるのは、利用者にとって説明のつかない変化になる。
      askable: statuses.filter((status) => status.agentPresent && status.state === "missing"),
      updated: outdated,
    };
  }
}

/**
 * 選ばれた配布先へ skill を書き込むユースケース。
 *
 * 対象は呼び出し側（ダイアログ・設定画面）が決める。ここは書くだけ。
 */
export class InstallAgentSkills {
  constructor(
    private readonly store: AgentSkillStore,
    private readonly targets: AgentSkillTarget[] = AGENT_SKILL_TARGETS,
  ) {}

  async execute(targetIds: string[]): Promise<void> {
    const chosen = this.targets.filter((target) => targetIds.includes(target.id));
    if (chosen.length === 0) return;

    const document = renderLineageSkill(await this.store.agentosPath());
    await this.store.write(chosen.map((target) => skillFiles(target, document)));
  }
}

/** 配布先1つぶんの書き込み内容（本文と版）。 */
function skillFiles(target: AgentSkillTarget, document: string): AgentSkillWrite {
  const manifest: SkillManifest = {
    name: SKILL_NAME,
    version: LINEAGE_SKILL_VERSION,
    installedAt: new Date().toISOString(),
    installedBy: SKILL_INSTALLED_BY,
  };
  return {
    target,
    files: [
      { name: SKILL_DOCUMENT_FILE, content: document },
      { name: SKILL_VERSION_FILE, content: `${JSON.stringify(manifest, null, 2)}\n` },
    ],
  };
}
