import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import { UpdateBanner } from "./updater/UpdateBanner";
import { ActionMenu } from "./automation/ActionMenu";
import { AutomationPage } from "./automation/AutomationPage";
import { AutomationSettings } from "./automation/AutomationSettings";
import { appClient } from "./app-client/appClient";
import { accountInitial, type Account } from "../core/domain/account/Account";
import {
  bodyPreview,
  isTask,
  metaText,
  type MetaAssignment,
  type Memo as RecordedMemo,
} from "../core/domain/memo/Memo";
import { matchesMemoQuery, parseMemoQuery } from "../core/domain/memo/MemoQuery";
import {
  findActiveTagToken,
  type MetaSuggestion,
  type TagToken,
} from "../core/domain/meta/MetaTag";
import { usageRatio, type StorageUsage } from "../core/domain/storage/StorageUsage";
import { absoluteDateTime, formatBytes, longDate, relativeTime } from "./format";
import {
  cardSurface,
  eyebrow,
  heroPadding,
  Icon,
  primaryButton,
  quietButton,
  secondaryButton,
  serifTitle,
  SettingRow,
  smallPrimaryButton,
  standardPage,
  subheading,
  tagChip,
  Toggle,
  type IconName,
} from "./ui";

type Page = "home" | "search" | "automation" | "settings";

/** 画面が扱う記録。minos が保存した Memo を表示用に整えたもの。 */
type Memo = {
  id: string;
  title: string;
  body: string;
  preview: string;
  metas: MetaAssignment[];
  createdAt: string;
  type: "task" | "memo";
  done?: boolean;
};

type LoadState = "loading" | "ready" | "error";

function toView(memo: RecordedMemo): Memo {
  return {
    id: memo.id,
    title: memo.title,
    body: memo.bodyText,
    // タイトルは本文1行目なので、一覧では続きだけを見せる。
    preview: bodyPreview(memo.bodyText),
    metas: memo.metas,
    createdAt: memo.createdAt,
    type: isTask(memo) ? "task" : "memo",
  };
}

/** minos が書いたローカル DB から記録を読み込む。 */
function useRecordedMemos() {
  const [memos, setMemos] = useState<Memo[]>([]);
  const [status, setStatus] = useState<LoadState>("loading");

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.listMemos())
      .then((records) => {
        if (!active) return;
        setMemos(records.map(toView));
        setStatus("ready");
      })
      .catch((error) => {
        if (!active) return;
        console.error("記録を読み込めませんでした", error);
        setStatus("error");
      });
    return () => {
      active = false;
    };
  }, []);

  return { memos, setMemos, status };
}

/**
 * ストレージ使用量。割り当て上限を持たない接続（ローカル接続）では null のまま。
 *
 * 取得に失敗したときも null にして、メーターを黙って隠す
 * （記録一覧と違い、無くても操作の妨げにならないため）。
 */
function useStorageUsage() {
  const [usage, setUsage] = useState<StorageUsage | null>(null);

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.storageUsage())
      .then((value) => {
        if (active) setUsage(value);
      })
      .catch((error) => console.error("ストレージ使用量を取得できませんでした", error));
    return () => {
      active = false;
    };
  }, []);

  return usage;
}

/**
 * ログイン中のアカウント。認証を持たない接続（ローカル接続）では null のまま。
 *
 * 取得に失敗したときも null にして、アカウント欄を黙って隠す
 * （記録一覧と違い、無くても操作の妨げにならないため）。
 */
function useAccount() {
  const [account, setAccount] = useState<Account | null>(null);

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.currentAccount())
      .then((value) => {
        if (active) setAccount(value);
      })
      .catch((error) => console.error("アカウントを取得できませんでした", error));
    return () => {
      active = false;
    };
  }, []);

  return account;
}

/** クラウド接続のときだけ出る使用量メーター。ローカル接続では何も描画しない。 */
function StorageMeter() {
  const usage = useStorageUsage();
  if (!usage) return null;
  return (
    <div className="px-[9px] pb-5">
      <div className="mb-2 flex justify-between text-[10px] text-[#92938e]">
        <span>ストレージ</span>
        <span>
          {formatBytes(usage.usedBytes)} / {formatBytes(usage.quotaBytes)}
        </span>
      </div>
      <div className="h-[3px] rounded bg-[#deded8]">
        <i
          className="block h-full rounded bg-[#7165ba]"
          style={{ width: `${usageRatio(usage) * 100}%` }}
        />
      </div>
    </div>
  );
}

/** 認証のある接続のときだけ出るアカウント欄。ローカル接続では何も描画しない。 */
function AccountButton({ account }: { account: Account | null }) {
  if (!account) return null;
  return (
    <button className="flex w-full items-center gap-[9px] border-0 border-t border-t-[#deded8] bg-transparent px-1.5 pt-[17px] text-left">
      <span className="grid h-8 w-8 place-items-center rounded-full bg-[#d9d4be] font-semibold text-[#5d5948]">
        {accountInitial(account)}
      </span>
      <span className="flex min-w-0 flex-1 flex-col">
        <b className="truncate text-[12px]">{account.displayName}</b>
        <small className="truncate text-[9px] text-[#969791]">{account.workspaceName}</small>
      </span>
      <Icon name="more" />
    </button>
  );
}

function Sidebar({
  page,
  setPage,
  account,
}: {
  page: Page;
  setPage: (p: Page) => void;
  account: Account | null;
}) {
  const items: { page: Page; label: string; icon: IconName }[] = [
    { page: "home", label: "ホーム", icon: "home" },
    { page: "search", label: "検索", icon: "search" },
    { page: "automation", label: "自動化", icon: "sparkles" },
    { page: "settings", label: "設定", icon: "settings" },
  ];
  const navItem =
    "flex h-[42px] items-center gap-3 rounded-lg border-0 px-[11px] text-left cursor-pointer";
  return (
    <aside className="fixed inset-y-0 left-0 z-[5] flex w-[226px] flex-col border-r border-[#e3e3dc] bg-[#f1f1ed] px-[15px] pt-6 pb-[18px]">
      <button
        className="flex cursor-pointer items-center gap-2.5 border-0 bg-transparent px-2 pt-[3px] pb-[30px] text-[18px] font-semibold"
        onClick={() => setPage("home")}
      >
        <span className="grid h-[29px] w-[29px] place-items-center rounded-[9px] bg-[#302e35] font-serif text-[17px] text-white">
          L
        </span>
        <span>lineage</span>
      </button>
      <nav className="flex flex-col gap-1">
        {items.map((item) => (
          <button
            key={item.page}
            className={`${navItem} ${page === item.page ? "bg-white font-semibold text-ink shadow-[0_1px_2px_#00000008]" : "bg-transparent text-[#6f716b] hover:bg-[#e9e9e4] hover:text-ink"}`}
            onClick={() => setPage(item.page)}
          >
            <Icon name={item.icon} />
            <span>{item.label}</span>
            {item.page === "search" && (
              <kbd className="ml-auto font-sans text-[11px] text-[#a0a19c]">⌘ K</kbd>
            )}
          </button>
        ))}
      </nav>
      <div className="mt-auto">
        <StorageMeter />
        <AccountButton account={account} />
      </div>
    </aside>
  );
}

/** 一度に出す補完候補の上限（minos の MAX_SUGGESTIONS と揃える）。 */
const MAX_SUGGESTIONS = 8;

/**
 * 補完対象の見直しが要るキャレット移動キー。
 * 上下は候補の選択に使うので、ここには入れない（選択位置が戻ってしまう）。
 */
const CARET_KEYS = ["ArrowLeft", "ArrowRight", "Home", "End"];

/**
 * `#` トークンを打っている間だけ、学習済みメタ情報の候補を読む。
 *
 * `query` が null なら補完は閉じている。候補が取れなくても検索自体は続けられるので、
 * 失敗しても黙って候補なしにする。
 */
function useMetaSuggestions(query: string | null) {
  const [suggestions, setSuggestions] = useState<MetaSuggestion[]>([]);

  useEffect(() => {
    if (query === null) {
      setSuggestions([]);
      return;
    }
    let active = true;
    appClient()
      .then((client) => client.suggestMetaTags(query, MAX_SUGGESTIONS))
      .then((found) => {
        if (active) setSuggestions(found);
      })
      .catch((error) => {
        console.error("メタ情報の候補を取得できませんでした", error);
        if (active) setSuggestions([]);
      });
    return () => {
      active = false;
    };
  }, [query]);

  return suggestions;
}

/**
 * 検索バー。`#` に続けて打つとメタ情報を補完し、確定した `#タグ` はそのまま絞り込み条件になる
 * （docs/ui.md「検索画面」）。並び順と一致規則は minos の入力補完と同じ。
 */
function SearchBox({
  value,
  onChange,
  onSubmit,
  large = false,
  className = "",
}: {
  value: string;
  onChange: (v: string) => void;
  onSubmit?: () => void;
  large?: boolean;
  className?: string;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [token, setToken] = useState<TagToken | null>(null);
  const [highlight, setHighlight] = useState(0);
  const suggestions = useMetaSuggestions(token?.query ?? null);
  const open = token !== null && suggestions.length > 0;
  // 候補は非同期に届くので、入力が進んで件数が減っても選択位置がはみ出さないようにする。
  const selected = Math.min(highlight, suggestions.length - 1);

  // カーソルが `#` トークンの中にいるかは、入力とカーソル移動のたびに見直す。
  const syncToken = (el: HTMLInputElement) => {
    setToken(findActiveTagToken(el.value.slice(0, el.selectionStart ?? el.value.length)));
    setHighlight(0);
  };

  const accept = (suggestion: MetaSuggestion) => {
    const el = input.current;
    if (!el || !token) return;
    // 置き換えるのは `#` からカーソルまで。`#タス` が `#タスク ` になる。
    const inserted = `#${suggestion.label} `;
    const caret = token.start + inserted.length;
    onChange(
      `${value.slice(0, token.start)}${inserted}${value.slice(el.selectionStart ?? value.length)}`,
    );
    setToken(null);
    // 制御コンポーネントなので、値が反映される描画を1回待ってからキャレットを置き直す。
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(caret, caret);
    });
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    // IME 変換中の Enter は変換の確定なので、補完にも検索にも使わない。
    if (e.nativeEvent.isComposing) return;
    if (open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        setHighlight(
          (h) =>
            (Math.min(h, suggestions.length - 1) +
              (e.key === "ArrowDown" ? 1 : suggestions.length - 1)) %
            suggestions.length,
        );
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        accept(suggestions[selected]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setToken(null);
        return;
      }
    }
    if (e.key === "Enter") onSubmit?.();
  };

  return (
    <div className={`relative ${large ? "max-w-[720px]" : ""} ${className}`}>
      <div
        className={`flex items-center gap-[11px] rounded-[10px] border border-[#deded8] bg-white px-3.5 shadow-[0_1px_3px_#2f302914] focus-within:border-[#9a92cc] focus-within:shadow-[0_0_0_3px_#7165ba14] ${large ? "h-[56px]" : "h-[46px]"}`}
      >
        <Icon name="search" />
        <input
          ref={input}
          className="min-w-0 flex-1 border-0 bg-transparent text-sm text-ink outline-none placeholder:text-[#a0a19b]"
          aria-label="メモを検索"
          role="combobox"
          aria-expanded={open}
          aria-controls="meta-suggestions"
          aria-autocomplete="list"
          value={value}
          onChange={(e) => {
            onChange(e.target.value);
            syncToken(e.target);
          }}
          onClick={(e) => syncToken(e.currentTarget)}
          onKeyUp={(e) => {
            if (CARET_KEYS.includes(e.key)) syncToken(e.currentTarget);
          }}
          onBlur={() => setToken(null)}
          onKeyDown={onKeyDown}
          placeholder="メモやタスクを検索…（# でメタ情報）"
        />
        <span className="rounded-[5px] border border-[#e1e1dc] bg-[#f8f8f5] px-[7px] py-0.5 text-[11px] text-[#a0a19b]">
          ⌘ K
        </span>
      </div>
      {/* onMouseDown を止めないと、クリックより先に blur が起きて候補が消える。 */}
      {open && (
        <ul
          id="meta-suggestions"
          role="listbox"
          aria-label="メタ情報の候補"
          className="absolute z-10 mt-1.5 w-full max-w-[420px] overflow-hidden rounded-[10px] border border-line bg-white py-1 shadow-[0_10px_30px_#2f302920]"
          onMouseDown={(e) => e.preventDefault()}
        >
          {suggestions.map((suggestion, index) => (
            <li
              key={suggestion.label}
              role="option"
              aria-selected={index === selected}
              className={`flex cursor-pointer items-center gap-2.5 px-3 py-2 text-[12px] ${index === selected ? "bg-[#f1f0ed]" : ""}`}
              onMouseEnter={() => setHighlight(index)}
              onClick={() => accept(suggestion)}
            >
              <Icon name="tag" size={14} />
              <b className="font-medium">#{suggestion.label}</b>
              {suggestion.shorthand && (
                <span className="font-mono text-[10px] text-[#8f918a]">{suggestion.shorthand}</span>
              )}
              <small className="ml-auto text-[10px] text-[#a0a19c]">
                {suggestion.usageCount}回{suggestion.matched === "shorthandPrefix" && " · 短縮"}
              </small>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** メタ情報のバッジ列。`#ラベル` / `#ラベル=値` の形で出す。 */
function MetaChips({ metas }: { metas: MetaAssignment[] }) {
  return (
    <>
      {metas.map((meta) => {
        const text = metaText(meta);
        return (
          <span className={tagChip} key={text}>
            #{text}
          </span>
        );
      })}
    </>
  );
}

function MemoCard({
  memo,
  onOpen,
  onToggle,
}: {
  memo: Memo;
  onOpen: () => void;
  onToggle: () => void;
}) {
  return (
    <article
      className="group relative flex cursor-pointer gap-3.5 border-b border-[#ecece7] px-[18px] py-[17px] outline-none transition duration-150 last:border-b-0 hover:bg-[#fafaf7] focus:bg-[#fafaf7]"
      onClick={onOpen}
      tabIndex={0}
      onKeyDown={(e) => e.key === "Enter" && onOpen()}
    >
      <div
        className={`grid h-8 w-8 place-items-center rounded-lg ${memo.type === "task" ? "bg-[#ebf3ef] text-[#578170]" : "bg-[#eeecf7] text-[#7467bb]"}`}
      >
        <Icon name={memo.type === "task" ? "check" : "file"} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-[9px]">
          <h3
            className={`text-[13px] font-semibold ${memo.done ? "text-[#a0a19c] line-through" : ""}`}
          >
            {memo.title}
          </h3>
          {memo.type === "task" && (
            <button
              className={`grid h-[17px] w-[17px] cursor-pointer place-items-center rounded-[5px] border p-0 text-white ${memo.done ? "border-[#6c967f] bg-[#6c967f]" : "border-[#cfd2cb] bg-white"}`}
              aria-label="完了状態を切り替え"
              onClick={(e) => {
                e.stopPropagation();
                onToggle();
              }}
            >
              {memo.done && <Icon name="check" size={13} />}
            </button>
          )}
        </div>
        {memo.preview && (
          <p className="mt-[5px] mb-2.5 truncate text-[11px] text-muted">{memo.preview}</p>
        )}
        <div className="flex items-center gap-1.5">
          <MetaChips metas={memo.metas} />
          <span className="ml-1 flex items-center gap-1 text-[9px] text-[#a0a19c]">
            <Icon name="clock" size={13} />
            {relativeTime(memo.createdAt)}
          </span>
        </div>
      </div>
      {/* 下書き（まだ保存していない記録）には自動化を当てられない。 */}
      {!memo.id.startsWith("draft-") && (
        <div className="self-center opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100">
          <ActionMenu memoId={memo.id} />
        </div>
      )}
      <button
        className="self-center border-0 bg-transparent text-[#b0b1ab] opacity-0 group-hover:opacity-100"
        aria-label="詳細を開く"
      >
        <Icon name="chevron" />
      </button>
    </article>
  );
}

/** 読み込み中・失敗・0件をひと通り出す一覧。 */
function MemoList({
  memos,
  status,
  openMemo,
  toggleMemo,
  className = cardSurface,
  empty,
}: {
  memos: Memo[];
  status: LoadState;
  openMemo: (m: Memo) => void;
  toggleMemo: (id: string) => void;
  className?: string;
  empty: { icon: IconName; title: string; hint: string };
}) {
  const emptyBox = "p-[75px] text-center text-[#999a94]";
  const emptyTitle = "mt-[1em] mb-1 text-sm font-bold text-[#555]";
  const emptyHint = "my-[1em] text-[11px]";
  if (status === "loading")
    return (
      <div className={emptyBox}>
        <Icon name="clock" size={30} />
        <h3 className={emptyTitle}>記録を読み込んでいます…</h3>
      </div>
    );
  if (status === "error")
    return (
      <div className={emptyBox}>
        <Icon name="inbox" size={30} />
        <h3 className={emptyTitle}>記録を読み込めませんでした</h3>
        <p className={emptyHint}>
          minos のデータベース（%LOCALAPPDATA%\minos\lineage.db）を開けませんでした。
        </p>
      </div>
    );
  if (!memos.length)
    return (
      <div className={emptyBox}>
        <Icon name={empty.icon} size={30} />
        <h3 className={emptyTitle}>{empty.title}</h3>
        <p className={emptyHint}>{empty.hint}</p>
      </div>
    );
  return (
    <div className={className}>
      {memos.map((m) => (
        <MemoCard
          memo={m}
          key={m.id}
          onOpen={() => openMemo(m)}
          onToggle={() => toggleMemo(m.id)}
        />
      ))}
    </div>
  );
}

function Home({
  memos,
  status,
  query,
  setQuery,
  setPage,
  openMemo,
  toggleMemo,
  createMemo,
  enabledRuleCount,
}: {
  memos: Memo[];
  status: LoadState;
  query: string;
  setQuery: (v: string) => void;
  setPage: (p: Page) => void;
  openMemo: (m: Memo) => void;
  toggleMemo: (id: string) => void;
  createMemo: () => void;
  enabledRuleCount: number;
}) {
  const quickCard =
    "flex cursor-pointer items-center gap-3 rounded-[10px] border border-line bg-white p-[15px] text-left hover:-translate-y-px hover:border-[#cfcec7]";
  return (
    <div className="min-h-screen">
      <header className="flex h-[66px] items-center gap-2 border-b border-line px-[38px]">
        <div className="flex-1" />
        <button
          className={quietButton}
          aria-label="テーマ設定を開く"
          onClick={() => setPage("settings")}
        >
          <Icon name="moon" size={17} />
        </button>
        <button className={smallPrimaryButton} onClick={createMemo}>
          <Icon name="plus" size={16} />
          新しいメモ
        </button>
      </header>
      <div className={`mx-auto max-w-[1100px] pt-[68px] pb-[52px] ${heroPadding}`}>
        <p className={eyebrow}>{longDate(new Date())}</p>
        <p className="mb-[34px] text-muted">思考の続きを、ここから始めましょう。</p>
        <SearchBox large value={query} onChange={setQuery} onSubmit={() => setPage("search")} />
      </div>
      <section
        className={`border-t border-line bg-white pt-[42px] pb-[70px] ${heroPadding} [&>*]:mx-auto [&>*]:max-w-[940px]`}
      >
        <div className="mb-[19px] flex items-start justify-between">
          <div>
            <h2 className={subheading}>最近の記録</h2>
            <p className="text-xs text-muted">新しく追加・更新されたメモ</p>
          </div>
          <button
            className="flex cursor-pointer items-center gap-[7px] border-0 bg-transparent text-xs text-[#686a65]"
            onClick={() => setPage("search")}
          >
            すべて表示 <Icon name="arrow" size={15} />
          </button>
        </div>
        <MemoList
          memos={memos.slice(0, 4)}
          status={status}
          openMemo={openMemo}
          toggleMemo={toggleMemo}
          empty={{
            icon: "inbox",
            title: "まだ記録がありません",
            hint: "minos を Alt + Space で呼び出して、最初のメモを残しましょう。",
          }}
        />
        <div className="mt-[15px] grid grid-cols-2 gap-[13px]">
          <button className={quickCard} onClick={() => setPage("search")}>
            <span className="grid h-9 w-9 place-items-center rounded-[9px] bg-[#eeecf7] text-[#7568b8]">
              <Icon name="inbox" />
            </span>
            <span className="flex flex-1 flex-col">
              <b className="text-[12px]">すべての記録</b>
              <small className="mt-[3px] text-[9px] text-[#969791]">
                {memos.length} 件のメモとタスク
              </small>
            </span>
            <Icon name="arrow" />
          </button>
          <button className={quickCard} onClick={() => setPage("automation")}>
            <span className="grid h-9 w-9 place-items-center rounded-[9px] bg-[#e9f2ee] text-[#5a8171]">
              <Icon name="sparkles" />
            </span>
            <span className="flex flex-1 flex-col">
              <b className="text-[12px]">自動化ルール</b>
              <small className="mt-[3px] text-[9px] text-[#969791]">
                {enabledRuleCount} 件が有効
              </small>
            </span>
            <Icon name="arrow" />
          </button>
        </div>
      </section>
    </div>
  );
}

function SearchPage({
  memos,
  status,
  query,
  setQuery,
  openMemo,
  toggleMemo,
}: {
  memos: Memo[];
  status: LoadState;
  query: string;
  setQuery: (v: string) => void;
  openMemo: (m: Memo) => void;
  toggleMemo: (id: string) => void;
}) {
  const [filter, setFilter] = useState("すべて");
  // 入力は「キーワード」と「#メタ情報」に分けて解釈する（規則は core/domain/memo/MemoQuery.ts）。
  const parsed = useMemo(() => parseMemoQuery(query), [query]);
  const results = useMemo(
    () =>
      memos.filter(
        (m) =>
          (filter === "すべて" || (filter === "タスク" ? m.type === "task" : m.type === "memo")) &&
          matchesMemoQuery({ title: m.title, bodyText: m.body, metas: m.metas }, parsed),
      ),
    [memos, parsed, filter],
  );
  return (
    <div className={standardPage}>
      <div className="mb-8">
        <p className={eyebrow}>LIBRARY</p>
        <h1 className={`${serifTitle} mb-[9px] text-[34px]`}>検索</h1>
        <p className="text-[13px] text-muted">
          これまでに残したすべての記録を探せます。
          <code className="ml-1 rounded bg-[#f1f0ed] px-1 font-mono text-[11px]">#</code>{" "}
          でメタ情報を補完して絞り込めます。
        </p>
      </div>
      <SearchBox large className="max-w-none" value={query} onChange={setQuery} />
      <div className="mt-[22px] mb-[13px] flex items-center gap-[7px]">
        {["すべて", "メモ", "タスク"].map((f) => (
          <button
            className={`cursor-pointer rounded-[7px] border border-transparent px-[13px] py-1.5 text-[11px] ${filter === f ? "bg-[#ecebe7] font-semibold" : "bg-transparent"}`}
            onClick={() => setFilter(f)}
            key={f}
          >
            {f}
          </button>
        ))}
        {parsed.metas.length > 0 && (
          <span className="flex items-center gap-1.5 border-l border-line pl-[9px]">
            <MetaChips metas={parsed.metas} />
          </span>
        )}
        <span className="ml-auto text-[10px] text-[#999a94]">{results.length} 件</span>
      </div>
      <MemoList
        memos={results}
        status={status}
        openMemo={openMemo}
        toggleMemo={toggleMemo}
        empty={{
          icon: "search",
          title: "一致する記録がありません",
          hint: "キーワードや絞り込みを変えてみてください。",
        }}
      />
    </div>
  );
}

function SettingsPage() {
  const [launch, setLaunch] = useState(true),
    [copy, setCopy] = useState(true),
    [theme, setTheme] = useState("システム");
  const section = "mt-[30px]",
    sectionTitle = "mb-[3px] text-[13px] font-bold",
    sectionDesc = "mb-2.5 text-[10px] text-muted",
    settingsCard = "overflow-hidden rounded-[10px] border border-line bg-white";
  return (
    <div className={standardPage}>
      <div className="mb-8">
        <p className={eyebrow}>PREFERENCES</p>
        <h1 className={`${serifTitle} mb-[9px] text-[34px]`}>設定</h1>
        <p className="text-[13px] text-muted">minos と fullos の動作をカスタマイズします。</p>
      </div>
      <section className={section}>
        <h2 className={sectionTitle}>minos</h2>
        <p className={sectionDesc}>クイック入力の動作</p>
        <div className={settingsCard}>
          <SettingRow title="システム起動時に開始" desc="PCへのログイン時にminosを自動で起動します">
            <Toggle value={launch} setValue={setLaunch} />
          </SettingRow>
          <SettingRow
            title="直前のアプリからテキストを取得"
            desc="呼び出したとき、選択中のテキストを入力欄へコピーします"
          >
            <Toggle value={copy} setValue={setCopy} />
          </SettingRow>
          <SettingRow title="呼び出しショートカット" desc="どこからでもクイック入力を開きます">
            <kbd className="rounded-md border border-b-2 border-[#deded8] bg-[#fafaf8] px-[9px] py-[5px] font-sans text-[11px] font-medium">
              Alt + Space
            </kbd>
          </SettingRow>
        </div>
      </section>
      <section className={section}>
        <h2 className={sectionTitle}>表示</h2>
        <p className={sectionDesc}>fullos の見た目</p>
        <div className={settingsCard}>
          <SettingRow title="テーマ" desc="アプリ全体の表示テーマ">
            <div className="rounded-[7px] bg-[#f1f1ed] p-[3px]">
              {["ライト", "システム", "ダーク"].map((v) => (
                <button
                  className={`cursor-pointer rounded-[5px] border-0 px-2.5 py-[5px] text-[9px] ${theme === v ? "bg-white shadow-[0_1px_3px_#0001]" : "bg-transparent"}`}
                  onClick={() => setTheme(v)}
                  key={v}
                >
                  {v}
                </button>
              ))}
            </div>
          </SettingRow>
        </div>
      </section>
      <section className={section}>
        <h2 className={sectionTitle}>メタ情報の短縮入力</h2>
        <p className={sectionDesc}>minos で # に続けて入力する短縮文字</p>
        <div className={settingsCard}>
          {[
            ["タスク", "task", "32回使用"],
            ["仕事", "work", "24回使用"],
            ["アイデア", "idea", "18回使用"],
          ].map((t) => (
            <div
              className="flex h-[55px] items-center gap-3 border-b border-line px-[18px] last:border-b-0"
              key={t[0]}
            >
              <span className={tagChip}>#{t[0]}</span>
              <span className="border-l border-line pl-3 font-mono text-[11px]">{t[1]}</span>
              <small className="ml-auto text-[9px] text-[#999a94]">{t[2]}</small>
              <button className={quietButton}>
                <Icon name="edit" />
              </button>
            </div>
          ))}
        </div>
      </section>
      <AutomationSettings />
    </div>
  );
}
function Detail({
  memo,
  close,
  update,
  remove,
}: {
  memo: Memo;
  close: () => void;
  update: (m: Memo) => void;
  remove: () => void;
}) {
  const [editing, setEditing] = useState(false),
    [title, setTitle] = useState(memo.title),
    [body, setBody] = useState(memo.body);
  const save = (e: FormEvent) => {
    e.preventDefault();
    update({ ...memo, title, body, preview: bodyPreview(body) });
    setEditing(false);
  };
  return (
    <div
      className="fixed inset-0 z-20 flex animate-[fade_0.18s] justify-end bg-[#27272245]"
      onMouseDown={(e) => e.target === e.currentTarget && close()}
    >
      <aside className="h-full w-[min(540px,50vw)] animate-[slide_0.25s_ease] bg-white px-[38px] py-[25px] shadow-[-10px_0_40px_#0002]">
        <header className="mb-[55px] flex items-center justify-between">
          <button
            className="cursor-pointer border-0 bg-transparent text-xs text-[#777972]"
            onClick={close}
          >
            ← 戻る
          </button>
          <div className="flex">
            <button className={quietButton} onClick={() => setEditing(true)}>
              <Icon name="edit" />
            </button>
            <button className={`${quietButton} text-[#b05d5d]`} onClick={remove}>
              <Icon name="trash" />
            </button>
          </div>
        </header>
        <div className="mb-[18px] grid h-[42px] w-[42px] place-items-center rounded-[10px] bg-[#eeecf7] text-[#7063b6]">
          <Icon name={memo.type === "task" ? "check" : "file"} />
        </div>
        {editing ? (
          <form onSubmit={save} className="flex flex-col gap-3.5">
            <input
              className="rounded-lg border border-[#deded8] p-3 font-serif text-[23px] outline-none"
              autoFocus
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
            <textarea
              className="min-h-[150px] resize-y rounded-lg border border-[#deded8] p-3 text-xs leading-[1.8] outline-none"
              value={body}
              onChange={(e) => setBody(e.target.value)}
            />
            <div className="flex justify-end gap-2">
              <button type="button" className={secondaryButton} onClick={() => setEditing(false)}>
                キャンセル
              </button>
              <button className={primaryButton}>保存する</button>
            </div>
          </form>
        ) : (
          <>
            <h1 className="mb-[21px] font-serif text-[29px] font-normal leading-[1.45]">
              {memo.title}
            </h1>
            <p className="border-b border-line pb-7 text-[13px] leading-[2] text-[#686a64]">
              {memo.body}
            </p>
          </>
        )}
        <div className="mt-[25px] [&>div]:mb-[19px] [&>div]:grid [&>div]:grid-cols-[100px_1fr] [&>div]:items-start [&>div]:text-[11px] [&>div>span:first-child]:text-[#969791]">
          <div>
            <span>作成日時</span>
            <b className="font-medium">{absoluteDateTime(memo.createdAt)}</b>
          </div>
          <div>
            <span>種類</span>
            <b className="font-medium">{memo.type === "task" ? "タスク" : "メモ"}</b>
          </div>
          <div>
            <span>メタ情報</span>
            <div>
              <MetaChips metas={memo.metas} />
            </div>
          </div>
          {!memo.id.startsWith("draft-") && (
            <div>
              <span>自動化</span>
              <div className="flex">
                <ActionMenu memoId={memo.id} />
              </div>
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}

/**
 * 有効な自動化ルールの件数。ホームのカードに出すだけなので、
 * 読めなくても 0 として黙って続ける（一覧の表示を妨げない）。
 */
function useEnabledRuleCount() {
  const [count, setCount] = useState(0);

  useEffect(() => {
    let active = true;
    appClient()
      .then((client) => client.listAutomationRules())
      .then((rules) => {
        if (active) setCount(rules.filter((rule) => rule.enabled).length);
      })
      .catch((error) => console.error("自動化ルールを数えられませんでした", error));
    return () => {
      active = false;
    };
  }, []);

  return count;
}

export default function App() {
  const [page, setPage] = useState<Page>("home"),
    [selected, setSelected] = useState<Memo | null>(null);
  const [query, setQuery] = useState("");
  const { memos, setMemos, status } = useRecordedMemos();
  const enabledRuleCount = useEnabledRuleCount();
  // アカウントはサイドバーとホームの両方が使うのでここで1回だけ読む。
  const account = useAccount();
  const toggle = (id: string) =>
    setMemos((m) => m.map((v) => (v.id === id ? { ...v, done: !v.done } : v)));
  const create = () =>
    setSelected({
      id: `draft-${Date.now()}`,
      title: "無題のメモ",
      body: "ここに内容を入力してください。",
      preview: "",
      metas: [],
      createdAt: new Date().toISOString(),
      type: "memo",
    });
  return (
    <div className="flex min-h-screen">
      <UpdateBanner />
      <Sidebar page={page} setPage={setPage} account={account} />
      <main className="ml-[226px] min-h-screen w-[calc(100%-226px)]">
        {page === "home" && (
          <Home
            memos={memos}
            status={status}
            query={query}
            setQuery={setQuery}
            setPage={setPage}
            openMemo={setSelected}
            toggleMemo={toggle}
            createMemo={create}
            enabledRuleCount={enabledRuleCount}
          />
        )}{" "}
        {page === "search" && (
          <SearchPage
            memos={memos}
            status={status}
            query={query}
            setQuery={setQuery}
            openMemo={setSelected}
            toggleMemo={toggle}
          />
        )}{" "}
        {page === "automation" && <AutomationPage />} {page === "settings" && <SettingsPage />}
      </main>
      {selected && (
        <Detail
          memo={selected}
          close={() => setSelected(null)}
          update={(m) => {
            setMemos((v) =>
              v.some((x) => x.id === m.id) ? v.map((x) => (x.id === m.id ? m : x)) : [m, ...v],
            );
            setSelected(m);
          }}
          remove={() => {
            setMemos((v) => v.filter((x) => x.id !== selected.id));
            setSelected(null);
          }}
        />
      )}
    </div>
  );
}
