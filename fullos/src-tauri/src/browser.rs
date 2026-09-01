//! ブラウザ方式のバックエンド。
//!
//! 生成AI の Web UI を別の WebView で開き、プロンプトを入力欄に流し込んで、
//! 応答を DOM から読み取る。
//!
//! # なぜ IPC を使わないのか
//!
//! この WebView が読み込むのは第三者のページである。Tauri v2 では capability の
//! ACL が効くのは `core:*` とプラグインのコマンドだけで、アプリが
//! `generate_handler!` で定義したコマンドは対象外になる。つまり
//! `withGlobalTauri` を有効にすると、開いたページの JavaScript から
//! `credential_set` や `automation_run` をそのまま呼べてしまう。
//!
//! そのため IPC は一切与えず、**ウィンドウタイトルだけを通信路にする**。
//! ページ側にできるのは自分のタイトルを書き換えることだけで、これは元から
//! どのページにもできることなので、新しい権限を渡したことにならない。
//!
//! やり取りは次の3語だけの素朴な約束にしてある。
//!
//! - `lineage-ready:<チャンク数>` … 応答を取り終えた
//! - `lineage-chunk:<番号>:<base64>` … `emit(番号)` を eval した結果
//! - `lineage-error:<base64>` … 入力欄が見つからないなどの失敗
//!
//! タイトルには長さの上限があるため、応答は base64 にして分割して受け取る。

use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// AI を開く WebView のラベル。ログインの Cookie を保つため使い回す。
const WINDOW_LABEL: &str = "ai-browser";

/// タイトルを見に行く間隔。
const POLL_INTERVAL: Duration = Duration::from_millis(120);

// 1チャンクの文字数（base64 後）はページ側の BRIDGE_JS が決める。
// タイトルの長さ制限に余裕を持たせるため 1500 文字にしてある。

const READY: &str = "lineage-ready:";
const CHUNK: &str = "lineage-chunk:";
const ERROR: &str = "lineage-error:";

/// どのページのどこを触るか。既定値は TypeScript 側が持ち、設定画面から編集できる。
///
/// サイトの改修でセレクタは必ず壊れるので、Rust に埋め込まず外から渡す形にしてある。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    /// 開く URL。
    pub url: String,
    /// プロンプトを書き込む入力欄。
    pub composer: String,
    /// 送信ボタン。見つからないときは入力欄で Enter を送る。
    pub send: String,
    /// 応答1件ぶんの要素。最後の1つを答えとして読む。
    pub answer: String,
    /// 応答がこの時間だけ変化しなければ、生成が終わったとみなす（ミリ秒）。
    pub quiet_ms: u64,
    /// 入力欄が現れるまで待つ上限（ミリ秒）。初回はここでログインを待つ。
    pub login_timeout_ms: u64,
    /// 応答を待つ上限（ミリ秒）。
    pub answer_timeout_ms: u64,
}

/// ブラウザ方式で1件実行し、応答テキストを返す。
#[tauri::command]
pub async fn browser_agent_run(
    app: AppHandle,
    profile: BrowserProfile,
    prompt: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || run(&app, &profile, &prompt))
        .await
        .map_err(|error| format!("処理を実行できません: {error}"))?
}

fn run(app: &AppHandle, profile: &BrowserProfile, prompt: &str) -> Result<String, String> {
    let window = open_window(app, profile)?;

    // 前回の実行のタイトルが残っていると、それを今回の結果と取り違える。
    window
        .set_title("lineage")
        .map_err(|error| format!("ウィンドウを初期化できません: {error}"))?;

    window
        .eval(&send_script(profile, prompt))
        .map_err(|error| format!("ページに指示を送れません: {error}"))?;

    // 入力欄の出現待ち（＝ログイン待ち）と応答待ちの合計。
    let budget = Duration::from_millis(profile.login_timeout_ms + profile.answer_timeout_ms)
        + Duration::from_secs(10);

    let chunks = wait_for_ready(&window, budget)?;
    collect(&window, chunks)
}

/// AI のページを開く。既にあれば作り直さない（ログイン状態を保つため）。
fn open_window(app: &AppHandle, profile: &BrowserProfile) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        // ページを開いたまま別の提供元へ切り替えたときのために、URL を合わせておく。
        let url = profile
            .url
            .parse()
            .map_err(|_| format!("URL を解釈できません: {}", profile.url))?;
        window
            .navigate(url)
            .map_err(|error| format!("ページを開けません: {error}"))?;
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(window);
    }

    let url = profile
        .url
        .parse()
        .map_err(|_| format!("URL を解釈できません: {}", profile.url))?;

    WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::External(url))
        .title("lineage")
        .inner_size(980.0, 800.0)
        // 橋渡しは読み込みのたびに入れ直す必要がある（ページ遷移で消えるため）。
        .initialization_script(BRIDGE_JS)
        .build()
        .map_err(|error| format!("ブラウザのウィンドウを開けません: {error}"))
}

/// `lineage-ready:<n>` が出るまでタイトルを見張る。
fn wait_for_ready(window: &WebviewWindow, budget: Duration) -> Result<usize, String> {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        // 利用者がウィンドウを閉じたら、待ち続けずに失敗として返す。
        if window.is_visible().is_err() {
            return Err("ブラウザのウィンドウが閉じられました".to_string());
        }

        let title = window.title().unwrap_or_default();
        if let Some(rest) = title.strip_prefix(READY) {
            return rest
                .parse::<usize>()
                .map_err(|_| format!("応答の件数を解釈できません: {rest}"));
        }
        if let Some(rest) = title.strip_prefix(ERROR) {
            return Err(decode(rest).unwrap_or_else(|| "ページ側で失敗しました".to_string()));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err("応答を待ちましたが、時間内に終わりませんでした".to_string())
}

/// チャンクを順に取り出して連結する。
fn collect(window: &WebviewWindow, chunks: usize) -> Result<String, String> {
    let mut encoded = String::new();
    for index in 0..chunks {
        window
            .eval(&format!("window.__lineage.emit({index})"))
            .map_err(|error| format!("応答を取り出せません: {error}"))?;

        let expected = format!("{CHUNK}{index}:");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if Instant::now() >= deadline {
                return Err(format!("応答の {index} 件目を取り出せませんでした"));
            }
            let title = window.title().unwrap_or_default();
            if let Some(rest) = title.strip_prefix(&expected) {
                encoded.push_str(rest);
                break;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    decode(&encoded).ok_or_else(|| "応答を復号できませんでした".to_string())
}

fn decode(encoded: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// ページ側へ渡す実行指示。
fn send_script(profile: &BrowserProfile, prompt: &str) -> String {
    // 引数は JSON にして埋め込む。プロンプトには引用符も改行も入るため、
    // 文字列連結で組み立てると容易に壊れる。
    let payload = serde_json::json!({
        "prompt": prompt,
        "composer": profile.composer,
        "send": profile.send,
        "answer": profile.answer,
        "quietMs": profile.quiet_ms,
        "loginTimeoutMs": profile.login_timeout_ms,
        "answerTimeoutMs": profile.answer_timeout_ms,
    });
    format!("window.__lineage.run({payload})")
}

/// ページに仕込む橋渡し。
///
/// Tauri の API には触れない（触らせない）。できるのは DOM の操作と
/// `document.title` の書き換えだけ。
const BRIDGE_JS: &str = r#"
(() => {
  if (window.__lineage) return;

  const CHUNK_SIZE = 1500;
  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
  const encode = (text) =>
    btoa(String.fromCharCode(...new TextEncoder().encode(text)));

  let chunks = [];

  const report = (marker, value) => {
    document.title = marker + value;
  };

  async function waitFor(selector, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const found = document.querySelector(selector);
      if (found) return found;
      await sleep(300);
    }
    return null;
  }

  // React などが value のセッターを監視しているため、直接代入しても
  // 状態が更新されない。プロトタイプのセッターを呼んでから input を発火させる。
  function fill(element, text) {
    if (element.tagName === "TEXTAREA" || element.tagName === "INPUT") {
      const setter = Object.getOwnPropertyDescriptor(
        Object.getPrototypeOf(element),
        "value",
      )?.set;
      if (setter) setter.call(element, text);
      else element.value = text;
    } else {
      element.focus();
      element.innerText = text;
    }
    element.dispatchEvent(new Event("input", { bubbles: true }));
  }

  // 生成が終わったかどうかは、応答の文字列が一定時間伸びなくなったかで見る。
  // 「生成中」を表す印はサイトごとに違ううえ、改修で最も先に変わるため。
  async function readAnswer(selector, previousCount, quietMs, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    let text = "";
    let lastChange = Date.now();
    while (Date.now() < deadline) {
      const nodes = document.querySelectorAll(selector);
      if (nodes.length > previousCount) {
        const current = nodes[nodes.length - 1].innerText || "";
        if (current !== text) {
          text = current;
          lastChange = Date.now();
        } else if (text && Date.now() - lastChange > quietMs) {
          return text;
        }
      }
      await sleep(300);
    }
    if (text) return text;
    throw new Error("応答が時間内に得られませんでした");
  }

  window.__lineage = {
    emit(index) {
      report("lineage-chunk:", index + ":" + (chunks[index] || ""));
    },

    async run(options) {
      try {
        chunks = [];
        const composer = await waitFor(options.composer, options.loginTimeoutMs);
        if (!composer) {
          throw new Error(
            "入力欄が見つかりません。ログインが必要か、セレクタが古い可能性があります",
          );
        }

        const previousCount = document.querySelectorAll(options.answer).length;
        fill(composer, options.prompt);
        await sleep(300);

        const sendButton = document.querySelector(options.send);
        if (sendButton && !sendButton.disabled) {
          sendButton.click();
        } else {
          composer.dispatchEvent(
            new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
          );
        }

        const answer = await readAnswer(
          options.answer,
          previousCount,
          options.quietMs,
          options.answerTimeoutMs,
        );

        const encoded = encode(answer);
        for (let at = 0; at < encoded.length; at += CHUNK_SIZE) {
          chunks.push(encoded.slice(at, at + CHUNK_SIZE));
        }
        report("lineage-ready:", String(chunks.length));
      } catch (error) {
        report("lineage-error:", encode(String((error && error.message) || error)));
      }
    },
  };
})();
"#;
