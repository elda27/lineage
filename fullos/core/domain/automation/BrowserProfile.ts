/**
 * ブラウザ方式で開くページと、そこで触る要素の指定。
 *
 * サイトの改修でセレクタは必ず壊れる。Rust に埋め込むと直すたびに再ビルドが要るので、
 * 既定値をここに置き、利用者が設定画面から上書きできるようにしてある
 * （上書きは `settings` テーブルの `automation.browser_profiles`）。
 *
 * ここは domain なので DB / Tauri / fetch には依存しない。
 */

export type BrowserProfile = {
  /** 開く URL。 */
  url: string;
  /** プロンプトを書き込む入力欄。 */
  composer: string;
  /** 送信ボタン。見つからないときは入力欄で Enter を送る。 */
  send: string;
  /** 応答1件ぶんの要素。最後の1つを答えとして読む。 */
  answer: string;
  /** 応答がこの時間だけ変化しなければ、生成が終わったとみなす（ミリ秒）。 */
  quietMs: number;
  /** 入力欄が現れるまで待つ上限（ミリ秒）。初回はここでログインを待つ。 */
  loginTimeoutMs: number;
  /** 応答を待つ上限（ミリ秒）。 */
  answerTimeoutMs: number;
};

/** 設定テーブルで上書きを持つキー。 */
export const BROWSER_PROFILES_SETTING_KEY = "automation.browser_profiles";

/**
 * 待ち時間の既定値。
 *
 * ログイン待ちを長めにとってあるのは、初回だけ利用者が手でログインするため。
 * その間ずっと入力欄が現れないので、短いと「壊れている」ように見えてしまう。
 */
const DEFAULT_TIMINGS = {
  quietMs: 2500,
  loginTimeoutMs: 180_000,
  answerTimeoutMs: 300_000,
} as const;

/**
 * 提供元ごとの既定値。
 *
 * セレクタは各サービスの画面構成に依存しており、先方の更新で動かなくなる。
 * 動かなくなったら設定画面から直す前提で、複数の候補をカンマで並べてある
 * （`querySelector` は最初に一致したものを返すので、新旧どちらでも拾える）。
 */
export const DEFAULT_BROWSER_PROFILES: Record<string, BrowserProfile> = {
  chatgpt: {
    url: "https://chatgpt.com/",
    composer: "#prompt-textarea, div[contenteditable='true']",
    send: "button[data-testid='send-button'], button[aria-label*='送信'], button[aria-label*='Send']",
    answer: "div[data-message-author-role='assistant']",
    ...DEFAULT_TIMINGS,
  },
  claude: {
    url: "https://claude.ai/new",
    composer: "div[contenteditable='true'].ProseMirror, div[contenteditable='true']",
    send: "button[aria-label*='Send'], button[aria-label*='送信']",
    answer: "div[data-testid='assistant-message'], div.font-claude-message",
    ...DEFAULT_TIMINGS,
  },
  gemini: {
    url: "https://gemini.google.com/app",
    composer: "div.ql-editor[contenteditable='true'], div[contenteditable='true']",
    send: "button[aria-label*='Send'], button[aria-label*='送信']",
    answer: "model-response, message-content",
    ...DEFAULT_TIMINGS,
  },
};

/** ブラウザ方式で選べる提供元。 */
export const BROWSER_PROVIDERS = Object.keys(DEFAULT_BROWSER_PROFILES);

/** APIキー方式で選べる提供元。実装があるのは Anthropic だけ。 */
export const API_KEY_PROVIDERS = ["anthropic"];

/** 提供元の表示名。 */
export function providerLabel(provider: string): string {
  switch (provider) {
    case "anthropic":
      return "Anthropic (Claude)";
    case "chatgpt":
      return "ChatGPT";
    case "claude":
      return "Claude";
    case "gemini":
      return "Gemini";
    default:
      return provider;
  }
}

/**
 * 保存された上書きを既定値に重ねる。
 *
 * 上書きは項目ごとに部分的でよい（URL だけ、セレクタ1つだけ、など）。
 * 設定が壊れていても既定値で動くほうがよいので、解釈できない値は無視する。
 */
export function resolveBrowserProfile(
  provider: string,
  overrides: Partial<Record<string, Partial<BrowserProfile>>> | null,
): BrowserProfile {
  const base = DEFAULT_BROWSER_PROFILES[provider];
  if (!base) {
    throw new Error(`ブラウザ方式に未対応の提供元です: ${provider}`);
  }
  return { ...base, ...overrides?.[provider] };
}

/** 設定テーブルに入っている JSON をほどく。壊れていれば上書きなしとして扱う。 */
export function parseBrowserProfileOverrides(
  value: string | null,
): Record<string, Partial<BrowserProfile>> | null {
  if (!value) return null;
  try {
    const parsed = JSON.parse(value);
    return typeof parsed === "object" && parsed !== null ? parsed : null;
  } catch {
    return null;
  }
}
