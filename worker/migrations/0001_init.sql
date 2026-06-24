-- D1 マイグレーション起点。内容は db/schema.sql と同一に保つこと（永続化1スキーマ）。
-- `wrangler d1 migrations apply lineage` で適用する。

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

CREATE TABLE IF NOT EXISTS links (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  seq INTEGER NOT NULL,
  source_kind TEXT NOT NULL,
  source_id TEXT NOT NULL,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  relation_type TEXT NOT NULL,
  actor TEXT,
  created_at TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  prev_hash TEXT NOT NULL,
  UNIQUE(workspace_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_rows_table ON rows(table_id);
CREATE INDEX IF NOT EXISTS idx_cells_row ON cells(row_id);
CREATE INDEX IF NOT EXISTS idx_links_workspace_seq ON links(workspace_id, seq);
CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_kind, source_id);
