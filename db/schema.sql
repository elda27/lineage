-- Lineage 共通スキーマ（SQLite / Cloudflare D1）
--
-- docs/concept/MINIMAL_ARCHITECTURE.md の「3. 永続化」に対応する。
-- D1 は SQLite 互換なので、ローカル(minos / fullos)とクラウドで同じ1本のスキーマを使う。
--
-- 追加・変更する場合はここを正本とし、D1 マイグレーションにも同じ内容を反映すること。

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

-- "rows" は SQL のキーワードと紛らわしいため常に引用符付きで参照する。
CREATE TABLE IF NOT EXISTS "rows" (
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

-- 記録(メモ)。minos で入力された内容は document_type = 'memo' として保存される。
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

-- Lineage 台帳。append-only。content_hash / prev_hash による hash-chain で真正性を担保する。
CREATE TABLE IF NOT EXISTS links (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  seq INTEGER NOT NULL,            -- workspace 内の連番（順序確定）
  source_kind TEXT NOT NULL,
  source_id TEXT NOT NULL,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  relation_type TEXT NOT NULL,
  actor TEXT,                      -- 誰が（cloud は JWT sub、local は "local"）
  created_at TEXT NOT NULL,
  content_hash TEXT NOT NULL,      -- このレコードの正規化ハッシュ
  prev_hash TEXT NOT NULL,         -- 直前 link の content_hash（鎖）
  UNIQUE(workspace_id, seq)
);

-- ここから下は minos（クイック入力）が使うメタ情報。
--
-- meta_tags は「ユーザが過去に入力したメタ情報」の学習結果であり、
-- 入力補完の候補集合になる。shorthand は fullos で設定する短縮文字列
-- （例: label='タスク', shorthand='task' なら `#t` で候補に出る）。
CREATE TABLE IF NOT EXISTS meta_tags (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  label TEXT NOT NULL,
  shorthand TEXT,
  usage_count INTEGER NOT NULL DEFAULT 0,
  last_used_at TEXT,
  created_at TEXT NOT NULL,
  UNIQUE(workspace_id, label)
);

-- document に付与されたメタ情報。source は 'auto'（自動付与）か 'user'（ユーザ入力）。
CREATE TABLE IF NOT EXISTS document_meta (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  label TEXT NOT NULL,
  value TEXT,
  source TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(document_id, label)
);

-- 組み込みタグ（docs/ui.md「組み込みタグ」）の機能が付ける、記録ごとの状態。
--
-- 「タスクの完了」「アーカイブ」「ゴミ箱」は利用者が打ったメタ情報ではなく操作の結果
-- なので、document_meta とは分けて持つ。行が無い記録は「未完了・未アーカイブ・
-- ゴミ箱でない」という既定の状態として扱うので、状態を変えたときだけ行ができる。
--
-- 削除は行の物理削除ではなく deleted_at で表す。documents は links から参照されており、
-- 消すと hash-chain の指す先が失われるため（docs/concept/MINIMAL_ARCHITECTURE.md 4.）。
CREATE TABLE IF NOT EXISTS document_states (
  document_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  -- 完了フラグ。1 = 完了。
  done INTEGER NOT NULL DEFAULT 0,
  done_at TEXT,
  -- アーカイブ済みなら日時。一覧から外れ、検索したときだけ出る。
  archived_at TEXT,
  -- ゴミ箱に入れた日時。一覧にも検索結果にも出ない。
  deleted_at TEXT,
  updated_at TEXT NOT NULL
);

-- 利用者の設定。minos の動作と fullos の表示の両方に効く（docs/ui.md「fullos」4.）ため、
-- minos が書いた値を fullos の設定画面から編集できるよう同じテーブルに置く。
CREATE TABLE IF NOT EXISTS settings (
  workspace_id TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, key)
);

-- ここから下は自動化（docs/ui.md「自動化画面」）が使う。
--
-- 自動化は「プロンプト ＋ 対象メモ」を生成AIに渡し、その結果を新しい document として
-- 残す。結果は memo から derived_from で辿れる（＝ lineage に乗る）ので、
-- 「何から何が作られたか」は自動生成物についても追跡できる。
CREATE TABLE IF NOT EXISTS automation_rules (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  -- {{memo.title}} / {{memo.body}} / {{memo.metas}} / {{now}} を含むテンプレート。
  prompt TEXT NOT NULL,
  -- 'api_key'（ローカルの鍵で HTTP 直呼び）か 'browser'（WebView 操作）。
  backend_kind TEXT NOT NULL,
  -- JSON。api_key: {"provider","model","effort"} / browser: {"provider"}
  backend_config TEXT NOT NULL,
  -- 'manual' / 'meta_match' / 'schedule'
  trigger_kind TEXT NOT NULL,
  -- JSON。meta_match: {"metas":[{"label","value"}]} / schedule: {"cron","metas"}
  trigger_config TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- 実行1回分。結果の本文は documents(document_type = 'automation_result') 側にある。
--
-- UNIQUE は張らない。二重実行の抑止は「成功済み/実行中の run が無いものだけ拾う」
-- という取り出し方で行う。制約にすると手動での再実行と失敗後の再試行まで塞いでしまう。
CREATE TABLE IF NOT EXISTS automation_runs (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  rule_id TEXT NOT NULL,
  source_document_id TEXT NOT NULL,
  result_document_id TEXT,
  -- 'running' / 'succeeded' / 'failed' / 'refused'
  status TEXT NOT NULL,
  backend_kind TEXT NOT NULL,
  error TEXT,
  started_at TEXT NOT NULL,
  finished_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_links_workspace_seq ON links(workspace_id, seq);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_kind, target_id);
CREATE INDEX IF NOT EXISTS idx_documents_workspace_created ON documents(workspace_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_meta_tags_workspace ON meta_tags(workspace_id, usage_count DESC);
CREATE INDEX IF NOT EXISTS idx_document_meta_document ON document_meta(document_id);
CREATE INDEX IF NOT EXISTS idx_document_meta_label ON document_meta(label);
CREATE INDEX IF NOT EXISTS idx_document_states_workspace ON document_states(workspace_id);
CREATE INDEX IF NOT EXISTS idx_automation_rules_workspace ON automation_rules(workspace_id, enabled);
CREATE INDEX IF NOT EXISTS idx_automation_runs_rule ON automation_runs(rule_id, source_document_id);
CREATE INDEX IF NOT EXISTS idx_automation_runs_started ON automation_runs(workspace_id, started_at DESC);
