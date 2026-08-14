import type {
  AgentSkillScan,
  AgentSkillStore,
  AgentSkillWrite,
} from "../../domain/ports/AgentSkillStore";
import type { AgentSkillTarget } from "../../domain/skill/AgentSkill";

/**
 * `@tauri-apps/api` の invoke のうち、ここで必要な部分だけ。
 *
 * プラグインの型を直接持ち込まないのは SqlHandle と同じ理由で、
 * テストや Tauri の外（vite dev をブラウザで開いた場合）で差し替えられるようにするため。
 */
export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

/**
 * エージェント CLI の設定ディレクトリを Rust 側（src-tauri/src/skill.rs）で読み書きする実装。
 *
 * webview からホーム以下のファイルは触れないので、パスの解決も書き込みも Rust に委ねる。
 * ここが持つのは受け渡しの形だけで、`..` の拒否などの検査は Rust 側が行う
 * （webview を信用しない側に検査を置く）。
 */
export class TauriAgentSkillStore implements AgentSkillStore {
  constructor(private readonly invoke: InvokeFn) {}

  scan(targets: AgentSkillTarget[]): Promise<AgentSkillScan[]> {
    return this.invoke<AgentSkillScan[]>("agent_skill_scan", {
      locations: targets.map((target) => ({
        id: target.id,
        directory: target.directory,
        marker: target.marker,
      })),
    });
  }

  write(writes: AgentSkillWrite[]): Promise<void> {
    return this.invoke("agent_skill_install", {
      installs: writes.map((write) => ({
        id: write.target.id,
        directory: write.target.directory,
        files: write.files,
      })),
    });
  }

  agentosPath(): Promise<string> {
    return this.invoke<string>("agent_skill_agentos_path");
  }
}
