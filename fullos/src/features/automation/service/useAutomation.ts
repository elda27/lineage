import { useCallback, useEffect, useState } from "react";

import type {
  AutomationRule,
  AutomationRulePatch,
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
      if (input.id) {
        const current = rules.find((rule) => rule.id === input.id);
        if (!current) throw new Error("編集対象の自動化ルールが見つかりません。");
        const patch = automationRulePatch(current, input);
        if (Object.keys(patch).length > 0) {
          await client.updateAutomationRule(input.id, patch);
        }
      } else {
        await client.createAutomationRule({
          name: input.name,
          description: input.description,
          prompt: input.prompt,
          backend: input.backend,
          backendConfig: input.backendConfig,
          triggerKind: input.triggerKind,
          trigger: input.trigger,
          enabled: input.enabled,
        });
      }
      await reload();
    },
    [reload, rules],
  );

  const remove = useCallback(
    async (id: string) => {
      const client = await appClient();
      await client.deleteAutomationRule(id);
      await reload();
    },
    [reload],
  );

  /** 有効・停止の切り替え。enabled だけを差分更新する。 */
  const toggle = useCallback(
    async (rule: AutomationRule) => {
      const client = await appClient();
      await client.updateAutomationRule(rule.id, { enabled: !rule.enabled });
      await reload();
    },
    [reload],
  );

  return { rules, status, reload, save, remove, toggle };
}

/** ルール編集フォームの全体値から、変更された項目だけを抽出する。 */
function automationRulePatch(
  previous: AutomationRule,
  next: AutomationRuleInput,
): AutomationRulePatch {
  const patch: AutomationRulePatch = {};
  if (previous.name !== next.name) patch.name = next.name;
  if (previous.description !== next.description) patch.description = next.description;
  if (previous.prompt !== next.prompt) patch.prompt = next.prompt;
  if (previous.backend !== next.backend) patch.backend = next.backend;
  if (!sameJson(previous.backendConfig, next.backendConfig)) {
    patch.backendConfig = next.backendConfig;
  }
  if (previous.triggerKind !== next.triggerKind) patch.triggerKind = next.triggerKind;
  if (!sameJson(previous.trigger, next.trigger)) patch.trigger = next.trigger;
  if (previous.enabled !== next.enabled) patch.enabled = next.enabled;
  return patch;
}

function sameJson(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
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
