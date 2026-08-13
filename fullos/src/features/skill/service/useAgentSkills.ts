import { useCallback, useEffect, useRef, useState } from "react";

import type { AgentSkillStatus } from "@core/domain/skill/AgentSkill";
import { appClient } from "@/shared/api/appClient";

/** 起動時の確認ダイアログの状態機械。 */
export type AgentSkillPrompt =
  /** まだ調べていない、または出す理由が無い。 */
  | { kind: "idle" }
  /** 追加してよいか尋ねている。 */
  | { kind: "asking"; candidates: AgentSkillStatus[] }
  /** 書き込み中。 */
  | { kind: "installing" }
  /** 追加できた。 */
  | { kind: "done"; installed: AgentSkillStatus[] }
  | { kind: "error"; message: string };

const messageOf = (error: unknown) => (error instanceof Error ? error.message : String(error));

/**
 * 起動時に skill を配る流れ。
 *
 * 起動のたびに走るのは「走査」と「古いものの入れ替え」までで、これは黙って行う
 * （既に置くことへ同意が得られているものの中身を新しくするだけなので）。
 * 未導入のエージェントへ新しく置くときだけ、このフックが確認を求める。
 *
 * 「今後表示しない」を選んだ場合に止まるのも確認だけで、入れ替えは続く。
 * 止めてしまうと、二度と更新されない古い手順書が利用者の手元に残る。
 */
export function useAgentSkills() {
  const [prompt, setPrompt] = useState<AgentSkillPrompt>({ kind: "idle" });
  // 「今後表示しない」のチェック状態。ダイアログを閉じるときにまとめて保存する。
  const [suppress, setSuppress] = useState(false);

  const sync = useCallback(async () => {
    try {
      const client = await appClient();
      const [{ askable }, preference] = await Promise.all([
        client.syncAgentSkills(),
        client.agentSkillPreference(),
      ]);
      if (preference.suppressed || askable.length === 0) return;
      setPrompt({ kind: "asking", candidates: askable });
    } catch (error) {
      // Tauri の外（ブラウザで vite dev を開いた場合）やホームが読めない環境では
      // 何もできない。利用者が求めた操作ではないので、黙って諦める。
      console.error("agent skill の確認に失敗しました", error);
    }
  }, []);

  const install = useCallback(
    async (targetIds: string[], candidates: AgentSkillStatus[]) => {
      setPrompt({ kind: "installing" });
      try {
        const client = await appClient();
        await client.installAgentSkills(targetIds);
        if (suppress) await client.saveAgentSkillPreference({ suppressed: true });
        setPrompt({
          kind: "done",
          installed: candidates.filter((c) => targetIds.includes(c.target.id)),
        });
      } catch (error) {
        setPrompt({ kind: "error", message: messageOf(error) });
      }
    },
    [suppress],
  );

  const dismiss = useCallback(async () => {
    setPrompt({ kind: "idle" });
    if (!suppress) return;
    try {
      const client = await appClient();
      await client.saveAgentSkillPreference({ suppressed: true });
    } catch (error) {
      console.error("設定を保存できませんでした", error);
    }
  }, [suppress]);

  // 起動時に一度だけ。StrictMode の二重実行は ref で防ぐ（useUpdater と同じ）。
  const started = useRef(false);
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    void sync();
  }, [sync]);

  return { prompt, suppress, setSuppress, install, dismiss };
}
