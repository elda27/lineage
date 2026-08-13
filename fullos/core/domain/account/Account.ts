/**
 * ログイン中の利用者と、その接続先ワークスペース。
 *
 * アカウントが存在するのは認証のあるクラウド接続だけである。
 * ローカル接続は認証なしの単一利用者なので ApplicationPort は null を返す
 * （docs/concept/MINIMAL_ARCHITECTURE.md 1.）。
 * ここは domain なので DB / Tauri / fetch には一切依存しない。
 */
export type Account = {
  displayName: string;
  /** 接続先のワークスペース名。 */
  workspaceName: string;
};

/** アバターに出す頭文字。サロゲートペア（絵文字など）で割れないように1文字取る。 */
export function accountInitial(account: Account): string {
  return ([...account.displayName.trim()][0] ?? "").toUpperCase();
}

/**
 * 呼びかけに使う名前（「山田 太郎」→「山田」）。
 *
 * 表示名の先頭要素を使う。区切りが無ければ表示名そのままになる。
 */
export function greetingName(account: Account): string {
  return account.displayName.trim().split(/\s+/)[0] ?? "";
}
