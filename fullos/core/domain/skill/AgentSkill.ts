/**
 * エージェント CLI へ配る skill の置き場所と、その版の比較。
 *
 * fullos は「記録を持っている側」で、Codex や Claude Code のようなエージェント CLI は
 * 「その記録を使いたい側」になる。両者をつなぐのに毎回 CLI の使い方を説明させるのは無駄なので、
 * 使い方をまとめた skill を各エージェントの設定ディレクトリへ置く。
 *
 * ここは domain なので Tauri / fs / DB に依存しない。実際にファイルを書くのは
 * `AgentSkillStore` の実装（Tauri コマンド越しの Rust）で、ここが持つのは
 * 「どこに」「どの版が入っているべきか」の知識だけ。
 */

/** skill の名前。ディレクトリ名にもなる。 */
export const SKILL_NAME = "lineage";

/** skill 本文のファイル名。4つのエージェントはいずれも SKILL.md 形式を読む。 */
export const SKILL_DOCUMENT_FILE = "SKILL.md";

/**
 * 版を記録するファイル名。skill のディレクトリ内に置く。
 *
 * 「入っているか」ではなく「どの版が入っているか」を知りたいので、本文とは別に持つ。
 * 本文を読んで差分を取る方法もあるが、利用者が手で直した skill を
 * 「壊れている」と誤判定してしまう。
 */
export const SKILL_VERSION_FILE = "version.json";

/** 配布先のエージェント CLI 1つぶん。 */
export type AgentSkillTarget = {
  /** 設定の保存や Rust への受け渡しに使う識別子。 */
  id: string;
  /** 画面に出す名前。 */
  label: string;
  /**
   * skill を置くディレクトリ。ホームディレクトリからの相対パスを区切って持つ。
   *
   * 区切って持つのは、Rust 側でホームと結合するときにパスの区切り文字を
   * OS に任せられるようにするため。`..` を含む要素は Rust 側で拒否する。
   */
  directory: string[];
  /**
   * そのエージェント CLI が導入済みかを判定するパス（ホームからの相対）。
   *
   * 導入していないエージェントの設定ディレクトリを勝手に作ると、
   * 利用者から見て「使っていないツールの設定が増えた」ようにしか見えない。
   * 存在するものにだけ配る。
   */
  marker: string[];
};

/**
 * 配布先の一覧。
 *
 * どのエージェントも「設定ディレクトリ／skills／<名前>／SKILL.md」の形に収束しているので、
 * 違うのは設定ディレクトリの名前だけになる。先方がレイアウトを変えたらこの表だけ直す。
 */
export const AGENT_SKILL_TARGETS: AgentSkillTarget[] = [
  {
    id: "codex",
    label: "Codex CLI",
    directory: [".codex", "skills", SKILL_NAME],
    marker: [".codex"],
  },
  {
    id: "github-copilot",
    label: "GitHub Copilot CLI",
    directory: [".copilot", "skills", SKILL_NAME],
    marker: [".copilot"],
  },
  {
    id: "gemini-cli",
    label: "Gemini CLI",
    directory: [".gemini", "skills", SKILL_NAME],
    marker: [".gemini"],
  },
  {
    id: "claude-code",
    label: "Claude Code",
    directory: [".claude", "skills", SKILL_NAME],
    marker: [".claude"],
  },
];

/** id から配布先を引く。設定に知らない id が残っていても落ちないよう undefined を返す。 */
export function findAgentSkillTarget(id: string): AgentSkillTarget | undefined {
  return AGENT_SKILL_TARGETS.find((target) => target.id === id);
}

/** `version.json` の中身。 */
export type SkillManifest = {
  name: string;
  version: string;
  /** 書き込んだ日時（ISO8601）。利用者が「いつ入ったか」を追えるように残す。 */
  installedAt: string;
  /** 書き込んだ側。手で置いた skill と区別できるようにする。 */
  installedBy: string;
};

/** 配布先ごとの状態。 */
export type AgentSkillState =
  /** skill がまだ無い。 */
  | "missing"
  /** 入っているが古い。 */
  | "outdated"
  /** 最新が入っている。 */
  | "current";

export type AgentSkillStatus = {
  target: AgentSkillTarget;
  /** エージェント CLI 本体の設定ディレクトリがあるか。 */
  agentPresent: boolean;
  /** 入っている skill の版。未導入・版が読めないときは null。 */
  installedVersion: string | null;
  state: AgentSkillState;
  /** 実際の配置先（表示用の絶対パス）。 */
  path: string;
};

/**
 * 版を比べる。a が新しければ正、古ければ負、同じなら 0。
 *
 * 扱うのは自分たちが振る番号だけなので、semver の完全な規則までは要らない。
 * `1.2.3` の数値部分を左から比べ、同じなら prerelease（`-` 以降）が
 * 付いているほうを古いとみなす。読めない値は 0 として扱い、
 * 「壊れた version.json は最古」＝更新対象になるようにする。
 */
export function compareVersions(a: string, b: string): number {
  const core = (version: string) =>
    version
      .split(/[-+]/)[0]
      .split(".")
      .map((part) => Number.parseInt(part, 10) || 0);

  const left = core(a);
  const right = core(b);
  for (let i = 0; i < Math.max(left.length, right.length); i += 1) {
    const diff = (left[i] ?? 0) - (right[i] ?? 0);
    if (diff !== 0) return diff > 0 ? 1 : -1;
  }

  const prerelease = (version: string) => (/[-+]/.test(version) ? 0 : 1);
  return prerelease(a) - prerelease(b);
}

/** 入っている版と配布する版から状態を決める。 */
export function skillState(installedVersion: string | null, latest: string): AgentSkillState {
  if (!installedVersion) return "missing";
  return compareVersions(installedVersion, latest) < 0 ? "outdated" : "current";
}

/**
 * 起動時の確認ダイアログについての設定。
 *
 * `suppressed` は「二度と出さない」。利用者が明示的にチェックしたときだけ立てる。
 */
export type AgentSkillPreference = {
  suppressed: boolean;
};

/** 設定テーブルのキー。 */
export const AGENT_SKILL_PREFERENCE_KEY = "agent_skill.prompt";

export const DEFAULT_AGENT_SKILL_PREFERENCE: AgentSkillPreference = { suppressed: false };

/** 設定テーブルの JSON をほどく。壊れていれば既定（＝確認する）に倒す。 */
export function parseAgentSkillPreference(value: string | null): AgentSkillPreference {
  if (!value) return DEFAULT_AGENT_SKILL_PREFERENCE;
  try {
    const parsed: unknown = JSON.parse(value);
    if (typeof parsed !== "object" || parsed === null) return DEFAULT_AGENT_SKILL_PREFERENCE;
    return { suppressed: (parsed as AgentSkillPreference).suppressed === true };
  } catch {
    return DEFAULT_AGENT_SKILL_PREFERENCE;
  }
}
