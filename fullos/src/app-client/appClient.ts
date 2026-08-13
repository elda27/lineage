import type { ApplicationPort } from "./ApplicationPort";
import { createLocalAppClient } from "./LocalAppClient";

/**
 * UI から使う ApplicationPort。接続は1本だけ張る（DB を二重に開かない）。
 *
 * いまはローカル接続のみ。クラウド接続を足すときはここで
 * 設定に応じて HttpAppClient を返す（UI 側は変更不要）。
 */
let connection: Promise<ApplicationPort> | undefined;

export function appClient(): Promise<ApplicationPort> {
  return (connection ??= createLocalAppClient());
}
