import { useCallback, useEffect, useState } from "react";

import type {
  AutomationRule,
  AutomationRuleInput,
  AutomationRun,
} from "@core/domain/automation/AutomationRule";
import { appClient } from "@/shared/api/appClient";

export type LoadState = "loading" | "ready" | "error";

/**
 * 自動化ルールの一覧と、その編集。
 *
 * 保存・削除のあとは一覧を読み直す。楽観的に画面だけ書き換えると、保存に失敗したときに
 * 「画面には出ているのに実行されないルール」ができてしまう。
 */
export function useAutomationRules() {
  const [rules, setRules] = useState<AutomationRule[]>([]);
  const [status, setStatus] = useState<LoadState>("loading");

  const reload = useCallback(async () => {
    try {
      const client = await appClient();
      setRules(await client.listAutomationRules());
      setStatus("ready");
    } catch (error) {
      console.error("自動化ルールを読み込めませんでした", error);
      setStatus("error");
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const save = useCallback(
    async (input: AutomationRuleInput) => {
      const client = await appClient();
      await client.saveAutomationRule(input);
      await reload();
    },
    [reload],
  );

  const remove = useCallback(
    async (id: string) => {
      const client = await appClient();
      await client.deleteAutomationRule(id);
      await reload();
    },
    [reload],
  );

  /** 有効・停止の切り替え。ルール全体を保存し直す。 */
  const toggle = useCallback(
    async (rule: AutomationRule) => {
      await save({ ...rule, enabled: !rule.enabled });
    },
    [save],
  );

  return { rules, status, reload, save, remove, toggle };
}

/** 実行履歴。実行のたびに読み直せるよう reload を返す。 */
export function useAutomationRuns() {
  const [runs, setRuns] = useState<AutomationRun[]>([]);
  const [status, setStatus] = useState<LoadState>("loading");

  const reload = useCallback(async () => {
    try {
      const client = await appClient();
      setRuns(await client.listAutomationRuns());
      setStatus("ready");
    } catch (error) {
      console.error("実行履歴を読み込めませんでした", error);
      setStatus("error");
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { runs, status, reload };
}

/**
 * 提供元ごとの API キーの登録状況。
 *
 * 値そのものは持たない（読み出す口を用意していない）。画面に出せるのは
 * 「登録済み / 未登録」だけで十分なので、平文を webview に持ってこない。
 */
export function useCredentialStatus(providers: string[]) {
  const [registered, setRegistered] = useState<Record<string, boolean>>({});

  const key = providers.join(",");
  const reload = useCallback(async () => {
    try {
      const client = await appClient();
      setRegistered(await client.credentialStatus(key.split(",")));
    } catch (error) {
      console.error("資格情報の状態を取得できませんでした", error);
    }
  }, [key]);

  useEffect(() => {
    void reload();
  }, [reload]);

  return { registered, reload };
}
