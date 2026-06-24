import {
  createContext,
  useContext,
  useEffect,
  useState,
  ReactNode,
} from "react";
import { ApplicationPort } from "../app-client/ApplicationPort";
import { createAppClient } from "../app-client";

const Ctx = createContext<ApplicationPort | null>(null);

// ApplicationPort を非同期に生成（ローカル接続は SQLite ロードを伴う）して配布する。
export function AppClientProvider({ children }: { children: ReactNode }) {
  const [client, setClient] = useState<ApplicationPort | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    createAppClient()
      .then((c) => alive && setClient(c))
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, []);

  if (error) {
    return (
      <div className="state state--error">
        接続の初期化に失敗しました: {error}
      </div>
    );
  }
  if (!client) {
    return <div className="state">接続を初期化しています…</div>;
  }
  return <Ctx.Provider value={client}>{children}</Ctx.Provider>;
}

export function useAppClient(): ApplicationPort {
  const c = useContext(Ctx);
  if (!c) throw new Error("useAppClient must be used within AppClientProvider");
  return c;
}
