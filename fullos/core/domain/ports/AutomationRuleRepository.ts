import type { AutomationRule, AutomationRun } from "../automation/AutomationRule";

/** 自動化ルールの参照ポート。書き込みは Rust の mutation API を通す。 */
export interface AutomationRuleRepository {
  /** workspace のルールを作成順に返す。無効なものも含む（一覧で切り替えるため）。 */
  all(workspaceId: string): Promise<AutomationRule[]>;
}

/** 実行履歴の読み出し。書き込みは agentos の担当。 */
export interface AutomationRunRepository {
  recent(workspaceId: string, limit: number): Promise<AutomationRun[]>;
}
