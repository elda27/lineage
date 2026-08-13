import type { Account } from "@core/domain/account/Account";
import type {
  AutomationRule,
  AutomationRuleInput,
  AutomationRun,
} from "@core/domain/automation/AutomationRule";
import type { BrowserProfile } from "@core/domain/automation/BrowserProfile";
import type { MetaSuggestion } from "@core/domain/meta/MetaTag";
import type { Memo } from "@core/domain/memo/Memo";
import type { StorageUsage } from "@core/domain/storage/StorageUsage";

/**
 * UI が依存する唯一のインターフェース。
 *
 * 実装はローカル接続（LocalAppClient / SQLite）とクラウド接続（HttpAppClient / D1）の2つ。
 * UI はどちらに繋がっているかを知らない（docs/concept/MINIMAL_ARCHITECTURE.md 1.）。
 */
export interface ApplicationPort {
  /** 記録を新しい順に取得する。 */
  listMemos(limit?: number): Promise<Memo[]>;

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
}
