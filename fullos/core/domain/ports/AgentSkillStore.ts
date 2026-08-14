import type { AgentSkillTarget } from "../skill/AgentSkill";

/** 走査1件ぶんの結果。 */
export type AgentSkillScan = {
  id: string;
  /** エージェント CLI 本体の設定ディレクトリがあるか。 */
  agentPresent: boolean;
  /** 置かれている `version.json` の版。無い・読めないときは null。 */
  installedVersion: string | null;
  /** 配置先の絶対パス（表示用）。 */
  path: string;
};

/** 書き込み1件ぶん。 */
export type AgentSkillWrite = {
  target: AgentSkillTarget;
  files: { name: string; content: string }[];
};

/**
 * エージェント CLI の設定ディレクトリを読み書きする口。
 *
 * 実体は利用者のホーム以下のファイルで、webview からは触れない。実装は Tauri
 * コマンド越しの Rust になるが、ユースケースはそれを知らない。
 */
export interface AgentSkillStore {
  /** 配布先を走査する。 */
  scan(targets: AgentSkillTarget[]): Promise<AgentSkillScan[]>;

  /** skill を書き込む（ディレクトリが無ければ作る）。 */
  write(writes: AgentSkillWrite[]): Promise<void>;

  /**
   * skill 本文に埋め込む `agentos` の絶対パス。
   *
   * 見つからない環境では実行ファイル名だけを返す（PATH 上にある可能性は残る）。
   */
  agentosPath(): Promise<string>;
}
