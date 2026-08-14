import type { Account } from "@core/domain/account/Account";
import type {
  AutomationRule,
  AutomationRuleInput,
  AutomationRun,
} from "@core/domain/automation/AutomationRule";
import type { BrowserProfile } from "@core/domain/automation/BrowserProfile";
import type { MetaSuggestion } from "@core/domain/meta/MetaTag";
import type { Memo } from "@core/domain/memo/Memo";
import type { AgentSkillPreference } from "@core/domain/skill/AgentSkill";
import type { AgentSkillSync } from "@core/app/skill/SyncAgentSkills";
import type { StorageUsage } from "@core/domain/storage/StorageUsage";

/**
 * UI が依存する唯一のインターフェース。
 *
 * 実装はローカル接続（LocalAppClient / SQLite）とクラウド接続（HttpAppClient / D1）の2つ。
 * UI はどちらに繋がっているかを知らない（docs/concept/MINIMAL_ARCHITECTURE.md 1.）。
 */
export interface ApplicationPort {
  /**
   * 記録を取得する。
   *
   * ゴミ箱のものは含まない。組み込みタグ（`#タスク` / `#メモ`）の付いた記録が先に、
   * 同じ優先度では新しい順に並ぶ。アーカイブ済みも含めて返すので、
   * 一覧から外すかどうかは画面側が決める。
   */
  listMemos(limit?: number): Promise<Memo[]>;

  /** 完了フラグを切り替える（組み込みタグ `#タスク` のチェック）。 */
  setMemoDone(memoId: string, done: boolean): Promise<void>;

  /** アーカイブする／戻す。アーカイブすると一覧から外れ、検索したときだけ出る。 */
  setMemoArchived(memoId: string, archived: boolean): Promise<void>;

  /**
   * ゴミ箱へ入れる。
   *
   * documents の行は消さない。links の指す先が失われると hash-chain を辿れなくなるため、
   * 削除は論理削除で表す。
   */
  trashMemo(memoId: string): Promise<void>;

  /**
   * 完了したタスクをまとめてアーカイブする。fullos を閉じるときに呼ぶ。
   *
   * チェックした瞬間ではなく閉じるときにまとめるのは、開いている間は取り消せる
   * ようにしておくため（docs/ui.md「組み込みタグ」）。
   */
  archiveCompletedTasks(): Promise<void>;

  /**
   * 検索バーで `#` を打ったときのメタ情報の補完候補。
   * `query` は `#` を除いた入力文字列（空なら「よく使う順」）。
   */
  suggestMetaTags(query: string, limit?: number): Promise<MetaSuggestion[]>;

  /**
   * ストレージ使用量。割り当て上限を持たない接続では null を返す。
   * UI は null のとき使用量を表示しない。
   */
  storageUsage(): Promise<StorageUsage | null>;

  /**
   * ログイン中のアカウント。認証を持たない接続では null を返す。
   * UI は null のときアカウント欄を表示しない。
   */
  currentAccount(): Promise<Account | null>;

  /** 自動化ルールを作成順に取得する（無効なものも含む）。 */
  listAutomationRules(): Promise<AutomationRule[]>;

  /** 自動化ルールを作成・更新する。 */
  saveAutomationRule(input: AutomationRuleInput): Promise<AutomationRule>;

  deleteAutomationRule(id: string): Promise<void>;

  /**
   * 記録に対して実行できるルール。メモの隣の「Action」ボタンが使う。
   *
   * 判定は実行エンジン側（agentos）で行う。UI 側の `matchesMemo` は予告表示用で、
   * 実際に出す候補はこの結果に従う。
   */
  matchAutomationRules(memoId: string): Promise<AutomationRule[]>;

  /**
   * ルールを記録に対して実行する。
   *
   * バックエンドの違い（APIキー／ブラウザ）は実装側が吸収する。UI は待つだけでよい。
   * 応答に数分かかりうる点にだけ注意する。
   */
  runAutomation(ruleId: string, memoId: string): Promise<AutomationRun>;

  /** 実行履歴を新しい順に取得する。 */
  listAutomationRuns(limit?: number): Promise<AutomationRun[]>;

  /**
   * 提供元ごとの API キーが登録済みか。
   *
   * 値そのものは返さない。設定画面に必要なのは「登録済みかどうか」だけで、
   * 平文を webview に持ってくる理由がない。
   */
  credentialStatus(providers: string[]): Promise<Record<string, boolean>>;

  setCredential(provider: string, secret: string): Promise<void>;

  deleteCredential(provider: string): Promise<void>;

  /** hash-chain の検証。自動化が鎖を壊していないことを確認できる。 */
  verifyLineage(): Promise<{ ok: boolean; detail: string }>;

  /**
   * ブラウザ方式のセレクタ上書き。
   *
   * 提供元のサイトが改修されるとセレクタは壊れる。再ビルドせずに直せるよう、
   * 設定として保存できるようにしてある。未設定なら null（＝既定値を使う）。
   */
  browserProfileOverrides(): Promise<Record<string, Partial<BrowserProfile>> | null>;

  saveBrowserProfileOverrides(overrides: Record<string, Partial<BrowserProfile>>): Promise<void>;

  /**
   * 定期実行が OS のスケジューラに登録されているか。
   *
   * agentos は常駐しないので、メタ情報マッチとスケジュールのルールを動かすには
   * 定期的な起動が要る。登録は利用者が明示的に有効にしたときだけ行う。
   */
  scheduleStatus(): Promise<boolean>;

  registerSchedule(): Promise<void>;

  unregisterSchedule(): Promise<void>;

  /**
   * エージェント CLI に配った skill の状態を調べ、古いものを最新へ入れ替える。
   *
   * 追加（まだ skill を持たないエージェントへ置く）はここでは行わない。利用者への
   * 確認が要るので、候補だけ返して判断は UI に委ねる。
   */
  syncAgentSkills(): Promise<AgentSkillSync>;

  /** 選ばれたエージェント CLI へ skill を配る。 */
  installAgentSkills(targetIds: string[]): Promise<void>;

  /** 起動時の確認ダイアログを出すかどうかの設定。 */
  agentSkillPreference(): Promise<AgentSkillPreference>;

  saveAgentSkillPreference(preference: AgentSkillPreference): Promise<void>;
}
