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

-- 利用者の設定。minos の動作と fullos の表示の両方に効く（docs/ui.md「fullos」4.）ため、
-- minos が書いた値を fullos の設定画面から編集できるよう同じテーブルに置く。
CREATE TABLE IF NOT EXISTS settings (
  workspace_id TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, key)
);

CREATE INDEX IF NOT EXISTS idx_links_workspace_seq ON links(workspace_id, seq);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_kind, target_id);
CREATE INDEX IF NOT EXISTS idx_documents_workspace_created ON documents(workspace_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_meta_tags_workspace ON meta_tags(workspace_id, usage_count DESC);
CREATE INDEX IF NOT EXISTS idx_document_meta_document ON document_meta(document_id);
CREATE INDEX IF NOT EXISTS idx_document_meta_label ON document_meta(label);
