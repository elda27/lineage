import { useEffect, useState } from "react";

import {
  API_KEY_PROVIDERS,
  BROWSER_PROVIDERS,
  DEFAULT_BROWSER_PROFILES,
  providerLabel,
  type BrowserProfile,
} from "@core/domain/automation/BrowserProfile";
import { appClient } from "@/shared/api/appClient";
import {
  Icon,
  secondaryButton,
  SettingRow,
  smallPrimaryButton,
  toggleKnob,
  toggleTrack,
} from "@/shared/ui/kit";
import { useCredentialStatus } from "@/features/automation/service/useAutomation";

/**
 * 設定画面の自動化セクション。
 *
 * API キーは OS の資格情報ストアに入り、DB には残らない。読み出す口も用意していないので、
 * ここに出せるのは「登録済みかどうか」だけになる。
 */
export function AutomationSettings() {
  return (
    <>
      <ApiKeySection />
      <ScheduleSection />
      <BrowserSection />
      <LineageSection />
    </>
  );
}

/**
 * 定期実行の登録。
 *
 * agentos は常駐しないので、「メタ情報マッチ」と「スケジュール」のルールは、
 * 誰かが定期的に起動しないと動かない。その登録をここから明示的に行う
 * （アプリが黙って OS にタスクを作らないようにするため）。
 */
function ScheduleSection() {
  const [enabled, setEnabled] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    appClient()
      .then((client) => client.scheduleStatus())
      .then(setEnabled)
      .catch((failure) => console.error("定期実行の状態を取得できませんでした", failure));
  }, []);

  const change = async (next: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const client = await appClient();
      if (next) await client.registerSchedule();
      else await client.unregisterSchedule();
      setEnabled(await client.scheduleStatus());
    } catch (failure) {
      setError(`${failure}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="mt-[30px]">
      <h2 className="mb-[3px] text-[13px] font-bold">自動化 / 定期実行</h2>
      <p className="mb-2.5 text-[10px] text-muted">
        「メタ情報マッチ」と「スケジュール」のルールを動かすために、15分ごとに agentos
        を起動します。
      </p>
      <div className="overflow-hidden rounded-[10px] border border-line bg-white">
        <SettingRow
          title="定期実行を有効にする"
          desc="Windows のタスクスケジューラに lineage-automation を登録します"
        >
          <button
            role="switch"
            aria-checked={enabled}
            disabled={busy}
            className={`${toggleTrack} ${enabled ? "bg-[#7063b6]" : "bg-[#d8d8d2]"}`}
            onClick={() => void change(!enabled)}
          >
            <i className={`${toggleKnob} ${enabled ? "translate-x-4" : ""}`} />
          </button>
        </SettingRow>
        {error && <p className="px-[18px] pb-[15px] text-[10px] text-[#a35]">{error}</p>}
      </div>
    </section>
  );
}

function ApiKeySection() {
  const { registered, reload } = useCredentialStatus(API_KEY_PROVIDERS);

  return (
    <section className="mt-[30px]">
      <h2 className="mb-[3px] text-[13px] font-bold">自動化 / APIキー</h2>
      <p className="mb-2.5 text-[10px] text-muted">
        OSの資格情報ストアに保存します。データベースには残りません。
      </p>
      <div className="overflow-hidden rounded-[10px] border border-line bg-white">
        {API_KEY_PROVIDERS.map((provider) => (
          <ApiKeyRow
            key={provider}
            provider={provider}
            registered={registered[provider] ?? false}
            onChanged={reload}
          />
        ))}
      </div>
    </section>
  );
}

function ApiKeyRow({
  provider,
  registered,
  onChanged,
}: {
  provider: string;
  registered: boolean;
  onChanged: () => Promise<void>;
}) {
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const client = await appClient();
      await client.setCredential(provider, secret);
      // 入力欄に鍵を残さない。画面を離れるまで平文が DOM に残る理由がない。
      setSecret("");
      await onChanged();
    } catch (failure) {
      setError(`${failure}`);
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      const client = await appClient();
      await client.deleteCredential(provider);
      await onChanged();
    } catch (failure) {
      setError(`${failure}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="border-b border-line px-[18px] py-[15px] last:border-b-0">
      <div className="flex items-center gap-3">
        <div className="flex flex-1 flex-col gap-[3px]">
          <b className="text-[12px]">{providerLabel(provider)}</b>
          <small className={`text-[9px] ${registered ? "text-[#578170]" : "text-[#8f918a]"}`}>
            {registered ? "登録済み" : "未登録"}
          </small>
        </div>
        {registered && (
          <button className={secondaryButton} onClick={() => void remove()} disabled={busy}>
            <Icon name="trash" size={14} />
            削除
          </button>
        )}
      </div>
      <div className="mt-2.5 flex gap-2">
        <input
          type="password"
          className="min-w-0 flex-1 rounded-lg border border-[#deded8] px-3 py-2 text-[12px] outline-none focus:border-[#9a92cc]"
          value={secret}
          onChange={(event) => setSecret(event.target.value)}
          placeholder={registered ? "新しいキーで上書き" : "sk-ant-…"}
          aria-label={`${providerLabel(provider)} のAPIキー`}
        />
        <button
          className={smallPrimaryButton}
          onClick={() => void save()}
          disabled={busy || !secret.trim()}
        >
          保存
        </button>
      </div>
      {error && <p className="mt-2 text-[10px] text-[#a35]">{error}</p>}
    </div>
  );
}

function BrowserSection() {
  const [overrides, setOverrides] = useState<Record<string, Partial<BrowserProfile>>>({});
  const [provider, setProvider] = useState(BROWSER_PROVIDERS[0]);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    appClient()
      .then((client) => client.browserProfileOverrides())
      .then((stored) => setOverrides(stored ?? {}))
      .catch((error) => console.error("セレクタの設定を読めませんでした", error));
  }, []);

  const current = { ...DEFAULT_BROWSER_PROFILES[provider], ...overrides[provider] };

  const patch = (changes: Partial<BrowserProfile>) => {
    setOverrides((previous) => ({
      ...previous,
      [provider]: { ...previous[provider], ...changes },
    }));
    setSaved(false);
  };

  const save = async () => {
    const client = await appClient();
    await client.saveBrowserProfileOverrides(overrides);
    setSaved(true);
  };

  const reset = () => {
    setOverrides((previous) => {
      const next = { ...previous };
      delete next[provider];
      return next;
    });
    setSaved(false);
  };

  const fields: { key: keyof BrowserProfile; label: string }[] = [
    { key: "url", label: "URL" },
    { key: "composer", label: "入力欄のセレクタ" },
    { key: "send", label: "送信ボタンのセレクタ" },
    { key: "answer", label: "応答のセレクタ" },
  ];

  return (
    <section className="mt-[30px]">
      <h2 className="mb-[3px] text-[13px] font-bold">自動化 / ブラウザ操作</h2>
      <p className="mb-2.5 text-[10px] text-muted">
        提供元のページが変わるとセレクタは壊れます。ここで直せば、アプリの更新を待たずに復旧できます。
      </p>
      <div className="overflow-hidden rounded-[10px] border border-line bg-white p-[18px]">
        <SettingRow title="提供元" desc="設定を編集する対象">
          <select
            className="rounded-lg border border-[#deded8] px-3 py-2 text-[12px] outline-none"
            value={provider}
            onChange={(event) => setProvider(event.target.value)}
          >
            {BROWSER_PROVIDERS.map((value) => (
              <option value={value} key={value}>
                {providerLabel(value)}
              </option>
            ))}
          </select>
        </SettingRow>

        {fields.map((item) => (
          <div className="mt-3 flex flex-col gap-1.5" key={item.key}>
            <label className="text-[11px] font-semibold text-[#6e706a]" htmlFor={`bp-${item.key}`}>
              {item.label}
            </label>
            <input
              id={`bp-${item.key}`}
              className="rounded-lg border border-[#deded8] px-3 py-2 font-mono text-[11px] outline-none focus:border-[#9a92cc]"
              value={String(current[item.key])}
              onChange={(event) => patch({ [item.key]: event.target.value })}
            />
          </div>
        ))}

        <div className="mt-4 flex items-center gap-2">
          <button className={smallPrimaryButton} onClick={() => void save()}>
            保存
          </button>
          <button className={secondaryButton} onClick={reset}>
            既定に戻す
          </button>
          {saved && <span className="text-[10px] text-[#578170]">保存しました</span>}
        </div>
      </div>
    </section>
  );
}

function LineageSection() {
  const [result, setResult] = useState<{ ok: boolean; detail: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const verify = async () => {
    setBusy(true);
    try {
      const client = await appClient();
      setResult(await client.verifyLineage());
    } catch (error) {
      setResult({ ok: false, detail: `${error}` });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="mt-[30px]">
      <h2 className="mb-[3px] text-[13px] font-bold">記録の真正性</h2>
      <p className="mb-2.5 text-[10px] text-muted">
        記録と自動化の結果をつなぐ履歴（hash-chain）が壊れていないかを確認します。
      </p>
      <div className="overflow-hidden rounded-[10px] border border-line bg-white">
        <SettingRow title="履歴を検証" desc="すべての link を先頭から再計算します">
          <button className={smallPrimaryButton} onClick={() => void verify()} disabled={busy}>
            {busy ? "検証中…" : "検証する"}
          </button>
        </SettingRow>
        {result && (
          <p
            className={`px-[18px] pb-[15px] text-[11px] ${result.ok ? "text-[#578170]" : "text-[#a35]"}`}
          >
            {result.ok ? "改ざんは検出されませんでした。" : `検証に失敗しました: ${result.detail}`}
          </p>
        )}
      </div>
    </section>
  );
}
