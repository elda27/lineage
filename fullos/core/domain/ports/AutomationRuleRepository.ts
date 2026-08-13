import type {
  AutomationRule,
  AutomationRuleInput,
  AutomationRun,
} from "../automation/AutomationRule";

/**
 * 自動化ルールと実行履歴の読み書き。
 *
 * ルール自体は lineage を生まない（「何から何が作られたか」ではなく設定なので）。
 * そのため fullos が直接書いてよい。実行の記録と結果 document は agentos が書く。
 */
export interface AutomationRuleRepository {
  /** workspace のルールを作成順に返す。無効なものも含む（一覧で切り替えるため）。 */
  all(workspaceId: string): Promise<AutomationRule[]>;

  /** 新規作成または更新。id を持たない入力には新しい id を振る。 */
  save(workspaceId: string, input: AutomationRuleInput): Promise<AutomationRule>;

  remove(id: string): Promise<void>;
}

/** 実行履歴の読み出し。書き込みは agentos の担当。 */
export interface AutomationRunRepository {
  recent(workspaceId: string, limit: number): Promise<AutomationRun[]>;
}
