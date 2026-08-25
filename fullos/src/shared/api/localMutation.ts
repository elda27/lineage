import { invoke } from "@tauri-apps/api/core";

import type {
  AutomationRuleInput,
  AutomationRulePatch,
} from "@core/domain/automation/AutomationRule";
import type { TagPatch } from "@core/domain/tag/TagDefinition";

/** `local_mutation_apply` が返す Rust 側の適用結果。 */
export type LocalMutationResult = {
  operationId: string;
  status: "applied" | "duplicate" | "conflict";
  entityKind: string;
  entityId: string;
  revision: number;
  recordedAt: string;
};

export type MemoStatePatch = {
  done?: boolean;
  archived?: boolean;
  trashed?: boolean;
};

export type LocalMutationOperation =
  | { type: "memo_state_patch"; memoId: string; patch: MemoStatePatch }
  | { type: "archive_completed_tasks"; labels: string[] }
  | { type: "tag_patch"; tagId: string; patch: TagPatch }
  | { type: "tag_delete"; tagId: string }
  | { type: "automation_rule_create"; input: Omit<AutomationRuleInput, "id"> }
  | { type: "automation_rule_patch"; ruleId: string; patch: AutomationRulePatch }
  | { type: "automation_rule_delete"; ruleId: string }
  | { type: "setting_set"; key: string; value: string };

export type LocalMutationRequest = {
  operationId: string;
  workspaceId: "local";
  baseRevision?: number;
  operation: LocalMutationOperation;
};

export type LocalMutationOptions = {
  /** A retry must reuse the same id so Rust can return `duplicate` safely. */
  operationId?: string;
  /** Revision observed by the caller; a stale value produces `conflict`. */
  baseRevision?: number;
};

/** 厳密な再送が必要な呼び出し側は、初回に作った request を保持する。 */
export function createLocalMutationRequest(
  operation: LocalMutationOperation,
  options: LocalMutationOptions = {},
): LocalMutationRequest {
  return {
    operationId: options.operationId ?? crypto.randomUUID(),
    workspaceId: "local",
    operation,
    ...(options.baseRevision === undefined ? {} : { baseRevision: options.baseRevision }),
  };
}

/** 作成済み request を送り、再送時も同じ冪等キーを維持する。 */
export function applyLocalMutationRequest(
  request: LocalMutationRequest,
): Promise<LocalMutationResult> {
  return invoke<LocalMutationResult>("local_mutation_apply", { request });
}

/** 応答前のsidecar終了に備え、同じ request（同じ operationId）で1回だけ再送する。 */
export async function applyLocalMutationRequestWithRetry(
  request: LocalMutationRequest,
): Promise<LocalMutationResult> {
  try {
    return await applyLocalMutationRequest(request);
  } catch {
    return applyLocalMutationRequest(request);
  }
}

/**
 * Rust 側の差分更新 command を型付きで呼ぶ。
 *
 * `operationId` は再送の冪等性キー。初回は省略してよいが、再送時は同じ値を渡す。
 * `undefined` の patch field は JSON に含めず、Rust 側の「変更なし」として扱う。
 */
export async function applyLocalMutation(
  operation: LocalMutationOperation,
  options: LocalMutationOptions = {},
): Promise<LocalMutationResult> {
  const request = createLocalMutationRequest(operation, options);
  return applyLocalMutationRequestWithRetry(request);
}

/** ApplicationPort の void 書き込み用。競合だけは保存失敗として UI へ返す。 */
export async function applyLocalMutationOrThrow(
  operation: LocalMutationOperation,
  options: LocalMutationOptions = {},
): Promise<void> {
  const result = await applyLocalMutation(operation, options);
  if (result.status === "conflict") {
    throw new Error("別の更新があるため保存できませんでした。最新の状態を読み直してください。");
  }
}
