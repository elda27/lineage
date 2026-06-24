import { ApplicationPort } from "./ApplicationPort";
import { createLocalAppClient } from "./LocalAppClient";
import { createHttpAppClient } from "./HttpAppClient";

export type { ApplicationPort } from "./ApplicationPort";

const DEFAULT_WORKSPACE_ID = "default-workspace";

// 接続モードを env で選択して ApplicationPort を返す。UI はこの結果だけを使う。
//   VITE_APP_MODE=local  … ローカル接続(SQLite, 認証なし) … 既定
//   VITE_APP_MODE=cloud  … クラウド接続(Workers/D1, JWT)
export async function createAppClient(): Promise<ApplicationPort> {
  const workspaceId = import.meta.env.VITE_WORKSPACE_ID ?? DEFAULT_WORKSPACE_ID;
  const mode = import.meta.env.VITE_APP_MODE ?? "local";

  if (mode === "cloud") {
    const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "";
    return createHttpAppClient({ baseUrl, workspaceId });
  }

  return createLocalAppClient(workspaceId);
}
