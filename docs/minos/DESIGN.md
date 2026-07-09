minos Design Doc

本書は minos の技術設計を定義する。
思想は docs/minos/CONCEPT.md、ユースケースは docs/minos/USECASE.md を参照。

設計ゴールは次の4点。

1. 起動体感 100ms 以内(常駐 + ウィンドウ show/hide)
2. 入力完了までのキーストロークを最小にする(補完・自動収集・キーボード完結)
3. SQLite 1 ファイルを複数アプリ(minos / ビュワー / 将来の分析アプリ)で共有する
4. Lineage 構想(docs/concept/MINIMAL_ARCHITECTURE.md)の共通スキーマと
   hash-chain に互換であること

---

1. 技術スタック

| 関心事           | 採用技術                                       |
|------------------|------------------------------------------------|
| 言語             | Rust (edition 2024)                            |
| UI               | GPUI + gpui-component(現行 Cargo.toml 準拠) |
| DB               | SQLite(rusqlite, bundled)                    |
| ハッシュ         | sha2(SHA-256)                                |
| OS 連携          | windows crate(Win32 / WinRT API)             |
| 配布             | WiX による Windows MSI(justfile の msi ターゲット) |

Windows を先行ターゲットとする。OS 依存機能は trait で抽象化し(4章)、
将来の macOS / Linux 対応はその trait の実装追加で行う。

---

2. プロセス構成: 常駐 + show/hide

minos はタスクトレイ常駐のシングルプロセスとして動く。

OS 起動時(または手動起動)
↓
常駐(ウィンドウ非表示・タスクトレイ)
↓
グローバルホットキー押下
↓
コンテキスト取得(直前アプリ・スクショ) → ウィンドウ show
↓
入力・登録(または Esc で中断)
↓
ウィンドウ hide(プロセスは残る)

コールドスタートではなく show/hide 方式を採る理由:

- プロセス起動・GPU 初期化・フォントロードをホットキー押下時に払わない。
  体感 100ms はウィンドウ表示とフォーカス移動だけなら達成可能だが、
  プロセス起動込みでは達成できない
- グローバルホットキーの受信自体に常駐プロセスが必要
- スキーマ・補完候補を事前にメモリへロードしておける

終了はタスクトレイメニューから行う。ウィンドウを閉じる(Esc)は hide であり終了ではない。

---

3. Windows 連携

3.1 グローバルホットキー

- RegisterHotKey で登録し、メッセージループで WM_HOTKEY を受信する
- デフォルトは未定(設定で変更可能にする)。修飾キー込みの組み合わせとする

---

3.2 直前フォアグラウンドアプリの検出

ホットキー受信時、自ウィンドウを表示する「前」に取得する
(表示後では minos 自身がフォアグラウンドになるため)。

WM_HOTKEY 受信
↓
GetForegroundWindow          … 直前アプリの HWND
↓
GetWindowTextW               … ウィンドウタイトル
↓
GetWindowThreadProcessId     … PID
↓
OpenProcess + QueryFullProcessImageNameW … プロセス名(実行ファイル名)
↓
自ウィンドウ show

プロセス名(拡張子除去・小文字化)を自動タグおよび source_app 列の値に使う。

---

3.3 スクリーンショット

直前アプリの HWND に対して、ウィンドウ単位のキャプチャを行う。

- 第一候補: Windows.Graphics.Capture(WinRT)。オクルージョンに強く高品質
- 簡易実装: PrintWindow / BitBlt。MVP の初期実装ではこちらでも良い

PNG にエンコードし、添付ファイル置き場(7章)へ保存する。
スクショは設定で無効化できる(9章)。

---

3.4 OS 抽象化境界

ホットキー・フォアグラウンド検出・スクショは以下の trait に閉じ込める。
結合テスト(.claude/skills/usecase-tests)ではこの trait をモックする。

```rust
pub trait ForegroundContextSource {
    /// ホットキー押下時点のフォアグラウンドアプリ情報
    fn capture_context(&self) -> ForegroundContext;
}

pub struct ForegroundContext {
    pub process_name: String,        // 例: "chrome"
    pub window_title: String,
    pub screenshot_png: Option<Vec<u8>>,
}
```

---

4. データモデル

4.1 共通スキーマ(Lineage 構想を踏襲)

docs/concept/MINIMAL_ARCHITECTURE.md の db/schema.sql をそのまま使う。
minos はこのスキーマに対する書き込みクライアントである。

```sql
CREATE TABLE IF NOT EXISTS workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  owner_user_id TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS table_assets (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  schema_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rows (
  id TEXT PRIMARY KEY,
  table_id TEXT NOT NULL,
  row_index INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cells (
  id TEXT PRIMARY KEY,
  row_id TEXT NOT NULL,
  column_key TEXT NOT NULL,
  raw_value TEXT,
  computed_value TEXT,
  formula TEXT,
  value_type TEXT,
  updated_at TEXT NOT NULL,
  UNIQUE(row_id, column_key)
);

CREATE TABLE IF NOT EXISTS documents (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  title TEXT NOT NULL,
  body_text TEXT,
  blob_uri TEXT,
  document_type TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Lineage は append-only。真正性のため content_hash / prev_hash を持つ(hash-chain)。
CREATE TABLE IF NOT EXISTS links (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  seq INTEGER NOT NULL,            -- workspace 内の連番(順序確定)
  source_kind TEXT NOT NULL,
  source_id TEXT NOT NULL,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  relation_type TEXT NOT NULL,
  actor TEXT,                      -- minos は "local"
  created_at TEXT NOT NULL,
  content_hash TEXT NOT NULL,      -- このレコードの正規化ハッシュ
  prev_hash TEXT NOT NULL,         -- 直前 link の content_hash(鎖)
  UNIQUE(workspace_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_rows_table ON rows(table_id);
CREATE INDEX IF NOT EXISTS idx_cells_row ON cells(row_id);
CREATE INDEX IF NOT EXISTS idx_links_workspace_seq ON links(workspace_id, seq);
CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_kind, source_id);
```

relation_type の初期セット: memo_for / attachment_for / references / derived_from

---

4.2 todo の表現

- todo は table_assets の1レコード(name = "todo")
- 1件の登録 = rows 1行 + cells(title / status / tags / source_app / created_at)
- tags は cells の1カラムで、raw_value に JSON 配列を格納する
  (例: `["chrome", "調査"]`。value_type = "tags")
- スクショは documents(document_type = "attachment"、blob_uri = ローカルファイルパス)
  として保存し、links に relation_type = "attachment_for"、
  source = row、target = document の link を追記する

登録1件のトランザクション:

rows insert
+ cells insert(全列)
+ documents insert(スクショがあれば)
+ links append(attachment_for、hash-chain)
→ すべて同一トランザクション(BEGIN / COMMIT)

---

4.3 schema_json のフォーム定義拡張

table_assets.schema_json には列定義に加え、
minos がフォームを生成するための UI ヒントを持たせる。

```json
{
  "columns": [
    { "key": "title",      "type": "text",     "required": true, "label": "やること" },
    { "key": "status",     "type": "text",     "default": "open", "hidden": true },
    { "key": "tags",       "type": "tags",     "complete": "history",
      "auto": "foreground_app" },
    { "key": "source_app", "type": "text",     "auto": "foreground_process", "hidden": true },
    { "key": "created_at", "type": "datetime", "auto": "now", "hidden": true }
  ],
  "attachments": [
    { "key": "screenshot", "auto": "foreground_screenshot", "optional": true }
  ]
}
```

- required: 入力必須。未入力なら登録できない
- default: 初期値
- hidden: フォームに表示しない(自動入力のみ)
- complete: 補完ソース。"history" は過去の同カラムの値から集計(8章)
- auto: 自動収集元。now / foreground_app / foreground_process / foreground_screenshot

このヒント語彙は minos が解釈する拡張であり、
スキーマ本体(型と列)は Lineage 構想の schema_json と互換に保つ。
未知のキーは他アプリが無視できる(前方互換)。

---

5. Lineage 真正性(hash-chain)

MINIMAL_ARCHITECTURE.md 4章の LineageLedger を Rust で実装する。
ハッシュ計算仕様を TS 版と一致させ、どのアプリからも同一ロジックで検証可能にする。

- links は append-only(更新・削除しない)
- content_hash = SHA-256( canonicalize(seq, source, target, relation_type, actor, created_at, prev_hash) )
- canonicalize はキー順固定の JSON 直列化(TS 版と同一のキー順・表現に合わせる)
- prev_hash は直前 link の content_hash。先頭は genesis 定数

```rust
pub struct LineageLedger;

impl LineageLedger {
    /// 直前レコードを受け取り、鎖を1つ伸ばす
    pub fn append_next(&self, prev: Option<&LineageRecord>, input: LineageInput) -> LineageRecord {
        let seq = prev.map_or(0, |p| p.seq) + 1;
        let prev_hash = prev.map_or(GENESIS_HASH.to_string(), |p| p.content_hash.clone());
        let canonical = canonicalize(&input, seq, &prev_hash); // キー順固定の JSON
        let content_hash = sha256_hex(&canonical);
        LineageRecord { seq, prev_hash, content_hash, ..input.into() }
    }

    /// 台帳全体を再計算して鎖の整合性を検証
    pub fn verify(&self, records: &[LineageRecord]) -> VerifyResult { /* TS 版 §4 と同一 */ }
}
```

登録フロー(4.2章)では、lastLink(workspace 内 seq 最大の link)を取得してから
append_next で新しい link を生成し、行・添付の insert と同一トランザクションで確定する。

---

6. DB 共有(複数アプリ構成)

6.1 ファイル配置の規約

| 対象               | パス                                        |
|--------------------|---------------------------------------------|
| DB ファイル        | %APPDATA%\lineage\lineage.db               |
| 添付(スクショ等) | %APPDATA%\lineage\attachments\<uuid>.png   |

- パスは設定で変更可能(9章)。この規約はビュワー等の別アプリと共有する
- documents.blob_uri には添付の絶対パス(または attachments/ 相対パス)を格納する

---

6.2 同時アクセス

- journal_mode = WAL とし、ビュワーが読んでいる間も minos が書き込めるようにする
- busy_timeout を設定し、瞬間的なロック競合はリトライで吸収する
- 第1弾の構成では書き込みは minos のみ、ビュワーは読み取り専用
  (ビュワーが status を更新する段階になったら、
  短トランザクション + WAL の範囲で共存できる。スキーマ的な排他は設けない)
- スキーマ適用は CREATE TABLE IF NOT EXISTS なので、どのアプリが先に起動しても良い

---

7. 入力高速化

- タグ・テキストの補完: 過去の cells から column_key ごとに値を集計し、
  頻度順に候補を出す。常駐時にメモリへロードし、登録のたびに差分更新する
- 前回テーブルの記憶: 最後に登録したテーブルを次回のデフォルト選択にする
- キーボード完結: Tab で次フィールド、Enter で登録、Esc で中断(hide)。
  マウス操作を要求しない
- 自動計算列(formula による導出)は cells スキーマ上は表現可能だが、
  第1弾では実装しない(将来拡張)

---

8. 画面

minos が持つ画面は2つだけ。一覧画面は作らない(CONCEPT.md 5章)。

8.1 クイック入力ウィンドウ(主画面)

- テーブル選択(前回選択がデフォルト。キーボードで切替可能)
- スキーマ駆動フォーム(schema_json から生成。hidden 列は出さない)
- 自動付与されたタグ・スクショ取得済みであることの表示(1行程度)
- Enter = 登録して hide、Esc = 破棄して hide

8.2 設定

- ホットキーの変更
- DB ファイルパス / 添付ディレクトリ
- スクリーンショット取得の on/off

---

9. 実装順(MVP)

1. スキーマ適用 + rusqlite での登録(rows/cells を1トランザクションで書く)
2. 常駐 + グローバルホットキー + ウィンドウ show/hide
3. todo フォーム(固定スキーマでまず動かす)
4. フォアグラウンドアプリ検出 → 自動タグ / source_app
5. タグ・値の補完
6. スクリーンショット取得 → documents + links(attachment_for)
7. LineageLedger(hash-chain)と登録トランザクションへの組み込み
8. schema_json 駆動の動的フォーム(固定スキーマを置き換える)

各段階で docs/minos/USECASE.md のシナリオに対応する結合テストを
.claude/skills/usecase-tests の規約に従って追加する。
