Minimal Architecture

本書は MVP の技術アーキテクチャを定義する。

設計ゴールは次の4点。

1. Tauri でのデスクトップ deploy を維持する（ローカルファースト）
2. 同じドメインコードを Hono + Cloudflare Workers にも deploy できる
3. 真正性(authenticity)を担保するため Lineage を改ざん検知可能な形で記録する
4. 永続化はローカルで SQLite、Cloudflare で D1（どちらも SQLite 互換なのでスキーマは1つ）

---

1. デプロイ構成（デュアルターゲット）

本システムは「同一のドメイン／アプリケーション層(TypeScript)」を共有し、
インフラ層(永続化・認証・トランスポート)だけを差し替える hexagonal 構成をとる。

重要: 認証の有無は「デプロイ先(Tauri か Web か)」ではなく
「frontend がどの ApplicationPort 実装を使うか(= 接続モード)」で決まる。
Tauri アプリも Web ビルドも、ローカル接続とクラウド接続の両方を取りうる。

接続モードL: ローカル接続（in-process / 単一利用者）
  Frontend（主に Tauri webview）
    └─ LocalAppClient
          ├─ 読み出し: SqliteRepository → @tauri-apps/plugin-sql → SQLite ファイル
          └─ 書き込み: invoke(local_mutation_apply) → Rust mutation API → SQLite
      認証: なし。データはローカルに閉じる。

接続モードC: クラウド接続（HTTP / マルチ利用者・認証アリ）
  Frontend（Tauri webview でも、Cloudflare Pages の Web ビルドでも可）
    └─ Supabase Auth ログイン → access_token(JWT)
          └─ HttpAppClient（fetch で REST 呼び出し, Bearer 付与）
                └─ Cloudflare Workers（Hono）
                      - JWT verify
                      - Application Service（同じ TS コード）
                          └─ D1AssetRepository
                                └─ D1（SQLite 互換, serverless）
                      - R2（添付・エクスポート）

ポイント:
- domain / app 層は両モードで「同一の TS ソース」を再利用する。
- frontend は「ApplicationPort」越しに呼ぶ。実装は2つ。
    - LocalAppClient … 読み出しは in-process、書き込みは Rust mutation command を呼ぶ（認証なし）
    - HttpAppClient  … Workers の REST API を fetch する（JWT を付ける＝認証アリ）
- Tauri アプリは「ローカル接続のみ」「クラウド接続のみ」「両方(切替/同期)」のいずれも構成可能。
  クラウドに接続する Tauri アプリは HttpAppClient を使うので、Web と同様に認証が必要になる。
- 切替は設定/環境変数で行い、UI コードは ApplicationPort しか見ないので変更不要。

デプロイ・ターゲット（成果物）:
- Tauri デスクトップアプリ（`pnpm tauri build`）… 中身は上記モードL/Cを切替可能
- Cloudflare Workers（`wrangler deploy`）… クラウド接続時のバックエンド
- （任意）Cloudflare Pages の Web フロントエンド … 常にモードC

スタック対応表（接続モード別）

| 関心事        | ローカル接続(モードL)         | クラウド接続(モードC)        |
|---------------|------------------------------|------------------------------|
| 代表的な実体  | Tauri デスクトップ           | Tauri or Web → Workers       |
| API 層        | 読み出し in-process / 書き込み Rust mutation command | Hono REST                    |
| DB            | SQLite(@tauri-apps/plugin-sql)| Cloudflare D1               |
| スキーマ      | schema.sql（共通）           | schema.sql（共通）           |
| ファイル      | ローカル FS                  | R2                           |
| 認証          | なし（単一利用者）           | Supabase Auth (JWT) アリ     |
| Lineage 台帳  | 同一の hash-chain ロジック   | 同一の hash-chain ロジック    |

---

2. DDD を意図したフォルダ構成

2026-09-28: 実装の正本は [architecture overview](../arch/README.md) とaccepted ADRを参照する。

- `lineage-core/src/domain`: 共有モデル・不変条件・純粋な補完/lineage/mutationルール。
- `lineage-store/src/features`: 共有保存・検証のユースケース。
- `lineage-store/src/ports`: repository・transaction・clock・ID取得の契約。
- `lineage-store/src/infra`: SQLite・時計・UUID・SHA-256の実装。
- `minos/src/domain` / `features` / `infra`: 入力・設定・補完UI・OS連携。
- `agentos/src/domain` / `features` / `infra`: 自動化実行・推論・資格情報。
- FullOSのUIは`src/pages`、`src/components`、`src/features`、ドメインとadapterは`core`に配置。

アプリ → lineage-store → lineage-coreの依存を守る。保存を伴う呼び出しは
`lineage_store::features`を使用する。coreにfeatures/infraを戻さない。
FullOSはrusqlite/sqlxのnative sqlite3競合を避けてRunner経由で書き込む。
以下のクラウド・単一schemaの説明は初期設計の記録であり、将来方針はADR-0009を優先する。

fullos の webview は plugin-sql で読み出す。書き込みは SQL を直接実行せず、Rust 側の
差分 mutation API（Tauri command）を `invoke` して行う。Rust API は entity 全体を
置き換える full update ではなく、変更部分だけの typed patch/delta を受け取り、
検証・revision 採番・トランザクション・冪等性を所有する。

組み込みタグの状態（完了・アーカイブ・ゴミ箱）は `document_states` に持ち、記録そのものは
変えず見せ方だけを変えるので鎖には載らない。削除も `deleted_at` を立てる論理削除にして、
`links` の指す先が消えないようにする。この状態更新を含む FullOS 管理データの書き込みは
Rust API の責務であり、plugin-sql の書き込み capability を前提にしない。

---

3. 永続化（SQLite ⇄ D1、スキーマは1つ）

D1 は SQLite 互換なので schema.sql を両方で使う。
domain は AssetRepository などのインターフェースだけを知り、
SQL は infrastructure 側に閉じ込める。

db/schema.sql（抜粋・共通）

CREATE TABLE workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  owner_user_id TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE table_assets (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  name TEXT NOT NULL,
  schema_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE rows (
  id TEXT PRIMARY KEY,
  table_id TEXT NOT NULL,
  row_index INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE cells (
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

CREATE TABLE documents (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  title TEXT NOT NULL,
  body_text TEXT,
  blob_uri TEXT,
  document_type TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Lineage は append-only。真正性のため content_hash / prev_hash を持つ（4章）。
CREATE TABLE links (
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

relation_type の初期セット:
  memo_for / attachment_for / references / derived_from

Repository インターフェース（core/domain/ports）

export interface AssetRepository {
  insertTable(t: TableAsset): Promise<void>;
  insertRow(r: Row, cells: Cell[]): Promise<void>;
  insertDocument(d: DocumentAsset): Promise<void>;
}

export interface LineageRepository {
  lastLink(workspaceId: string): Promise<LineageRecord | null>; // 鎖の末尾
  append(link: LineageRecord): Promise<void>;
  list(workspaceId: string): Promise<LineageRecord[]>;          // seq 昇順
}

infrastructure では同じインターフェースを SQLite 版 / D1 版で実装する。
SQL 文字列はほぼ共通で、実行ハンドルだけが異なる
（plugin-sql の Database か、D1Database か）。

---

4. Lineage の真正性担保（hash-chain）

目的: 「何から何が作られたか」という変換履歴を、後から改ざんできない形で残す。

方式:
- Lineage は append-only（更新・削除しない）。
- 各 link はワークスペース内で連番 seq を持つ。
- 各 link の content_hash = SHA-256( 正規化(seq, source, target, relation_type, actor, created_at, prev_hash) )。
- prev_hash には「直前 link の content_hash」を入れる（先頭は genesis 定数）。
- これにより links は1本の hash-chain になり、途中の1件でも書き換えると
  以降の全 content_hash が合わなくなる → 改ざんを検知できる。

ドメインサービス LineageLedger（core/domain/lineage）

export class LineageLedger {
  constructor(private hasher: Hasher) {}

  // 直前レコードを受け取り、鎖を1つ伸ばす
  appendNext(prev: LineageRecord | null, input: LineageInput): LineageRecord {
    const seq = (prev?.seq ?? 0) + 1;
    const prevHash = prev?.contentHash ?? GENESIS_HASH;
    const canonical = canonicalize({ ...input, seq, prevHash }); // キー順固定の JSON
    const contentHash = this.hasher.sha256Hex(canonical);
    return { ...input, seq, prevHash, contentHash };
  }

  // 台帳全体を再計算して鎖の整合性を検証
  verify(records: LineageRecord[]): VerifyResult {
    let prevHash = GENESIS_HASH;
    for (const r of records) {            // seq 昇順
      const expected = this.hasher.sha256Hex(
        canonicalize({ ...r, prevHash })
      );
      if (r.prevHash !== prevHash) return { ok: false, brokenAt: r.seq };
      if (r.contentHash !== expected) return { ok: false, brokenAt: r.seq };
      prevHash = r.contentHash;
    }
    return { ok: true };
  }
}

- Hasher は WebCrypto(crypto.subtle) で実装（Workers / Tauri webview 双方で利用可能）。
- 検証はローカル・クラウドどちらでも同一ロジックで実行できる（VerifyLineage ユースケース）。
- 将来、最終 content_hash に署名(Ed25519)を付ければ第三者検証も可能。本 MVP では hash-chain まで。

WriteMemo ユースケースの流れ（真正性込み）

1. document を1件 insert
2. LineageRepository.lastLink で鎖の末尾を取得
3. LineageLedger.appendNext で row → document の link を生成（content_hash 付与）
4. document insert と link append を同一トランザクション（D1 は batch、SQLite は tx）で確定

---

5. クラウド実装: Hono + D1 + R2（worker/index.ts）

import { Hono } from "hono";
import { cors } from "hono/cors";
import { jwtVerify, createRemoteJWKSet } from "jose";
import { D1AssetRepository, D1LineageRepository } from "@core/infra/persistence/d1";
import { Sha256Hasher } from "@core/infra/crypto/Sha256Hasher";
import { WriteMemo } from "@core/features/memo/WriteMemo";

type Env = {
  DB: D1Database;
  BUCKET: R2Bucket;
  SUPABASE_JWKS_URL: string;
  SUPABASE_JWT_ISSUER: string;
};

const app = new Hono<{ Bindings: Env; Variables: { userId: string } }>();
app.use("*", cors());

app.use("/api/*", async (c, next) => {
  const auth = c.req.header("Authorization");
  if (!auth?.startsWith("Bearer ")) return c.json({ error: "Unauthorized" }, 401);
  const jwks = createRemoteJWKSet(new URL(c.env.SUPABASE_JWKS_URL));
  const { payload } = await jwtVerify(auth.slice(7), jwks, {
    issuer: c.env.SUPABASE_JWT_ISSUER,
  });
  c.set("userId", String(payload.sub));
  await next();
});

// Worker は composition root。infra を組み立てて application を呼ぶだけ。
app.post("/api/rows/:rowId/memos", async (c) => {
  const body = await c.req.json();
  const assets = new D1AssetRepository(c.env.DB);
  const lineage = new D1LineageRepository(c.env.DB);
  const useCase = new WriteMemo(assets, lineage, new Sha256Hasher());

  const result = await useCase.execute({
    workspaceId: body.workspace_id,
    rowId: c.req.param("rowId"),
    title: body.title ?? "memo",
    bodyText: body.body_text,
    actor: c.get("userId"),
  });
  return c.json(result);
});

app.get("/api/workspaces/:id/lineage/verify", async (c) => {
  const lineage = new D1LineageRepository(c.env.DB);
  const records = await lineage.list(c.req.param("id"));
  const ledger = new LineageLedger(new Sha256Hasher());
  return c.json(ledger.verify(records));
});

export default app;

wrangler.toml

name = "lineage-api"
main = "worker/index.ts"
compatibility_date = "2026-06-01"

[[d1_databases]]
binding = "DB"
database_name = "lineage"
database_id = "xxxx"

[[r2_buckets]]
binding = "BUCKET"
bucket_name = "lineage-files"

[vars]
SUPABASE_JWKS_URL = "https://<project-ref>.supabase.co/auth/v1/.well-known/jwks.json"
SUPABASE_JWT_ISSUER = "https://<project-ref>.supabase.co/auth/v1"

D1 マイグレーション: `wrangler d1 migrations apply lineage`（db/schema.sql を起点）。

---

6. ローカル実装: Tauri + SQLite（読み出し plugin-sql / 書き込み Rust API）

FullOS のローカル接続では、SQLite の読み出しに `@tauri-apps/plugin-sql` を使い、
書き込みは Rust 側の mutation API に集約する。WebView から SQL の `INSERT` / `UPDATE` /
`DELETE` は実行しない。Rust API の内部実装は SQLite の接続競合を避ける境界に置き、
WebView からは Tauri command の入力・出力契約だけを参照する。

src-tauri/Cargo.toml に依存追加:
  tauri-plugin-sql = { version = "2", features = ["sqlite"] }

src-tauri/src/lib.rs:
  tauri::Builder::default()
    .plugin(tauri_plugin_sql::Builder::default().build())

LocalAppClient（src/shared/api/LocalAppClient.ts）

import Database from "@tauri-apps/plugin-sql";
import { invoke } from "@tauri-apps/api/core";
import { SqliteMemoRepository } from "@core/infra/persistence/sqlite";

export async function createLocalAppClient(): Promise<ApplicationPort> {
  const db = await Database.load("sqlite:lineage.db"); // schema.sql を初回適用
  const memos = new SqliteMemoRepository(db); // 読み出し専用
  return {
    listMemos: (limit) => memos.list("local", limit),
    setMemoDone: (memoId, done) =>
      invoke("local_mutation_apply", {
        request: {
          operationId: crypto.randomUUID(),
          workspaceId: "local",
          operation: {
            type: "memo_state_patch",
            memoId,
            patch: { done },
          },
        },
      }),
    // 他の書き込みも同じ typed patch/delta API を使う。
  };
}

mutation API の `operationId` はリトライ時の二重適用を防ぐ冪等キー、`baseRevision` は
Rust 側が管理する entity 単位の単調増加 revision に対する競合検出値である。指定された
`baseRevision` が一致しない更新は受け付けず、成功した更新だけを1トランザクションで
revision とともに確定する。既存ローカル UI の移行中は省略可能だが、revision を返す read
model と同期・remote adapter では必須にする。mutation の operationId、entity、status、patch、
baseRevision、作成日時は `local_mutations` に保存し、同期対象は `applied` の delta に限定する。

Lineage を生む記録の確定は引き続き lineage-core を共有する minos / agentos が担い、
FullOS の状態・設定・ルール更新は Rust mutation API が担う。

---

7. MVP で実装するもの

対象は「スマホでも PC でも使える記録特化 Excel」。

1. テーブル作成
2. CSV アップロード
3. 行・セル編集
4. 行にメモを書く
5. 添付ファイルを保存する
6. 行・セル・ドキュメントの関係を Lineage(hash-chain) として保存する
7. Lineage の検証（真正性チェック）

Projection / AI 分類は入れない（FUTURE_SPEC を参照）。

---

8. API 最小セット（クラウド）

POST  /api/tables
GET   /api/tables/:tableId
POST  /api/tables/:tableId/rows
PATCH /api/rows/:rowId/cells
POST  /api/rows/:rowId/memos          # document + lineage(memo_for) を確定
GET   /api/rows/:rowId/memos
POST  /api/attachments/presign        # R2
POST  /api/links                      # 任意の lineage を追記（append-only）
GET   /api/links?source_kind=row&source_id=xxx
GET   /api/workspaces/:id/lineage/verify   # hash-chain 検証

ローカル(Tauri)では同じ操作を ApplicationPort のメソッドとして in-process 提供する
（HTTP は介さない）。

FullOS のローカル書き込みは `local_mutation_apply` command に集約する。入力は次の
mutation 契約を基本とし、entity 全体の JSON を送る full update は行わない。

```text
{ operationId, workspaceId, baseRevision, operation }
```

`operationId` は冪等キー、`baseRevision` は Rust が採番する per-entity revision に対する
楽観的競合検出値である。Rust は検証、必要な複数テーブル更新、revision の増加を1 transaction
で確定し、競合時は `conflict` を返す。mutation の受理結果は `local_mutations` に記録し、
将来は `applied` の delta から local/server 同期用の outbox を追加できる境界にする。

---

9. 画面

/tables          テーブル一覧
/tables/:id      Excel 風グリッド
/rows/:id        行詳細（セル / メモ / 添付 / その行の lineage）

スマホでは /rows/:id を主画面にする。

---

10. 実装順

1. db/schema.sql と core/domain（Asset / Lineage / LineageLedger）
2. SqliteAssetRepository / SqliteLineageRepository（ローカル先行）
3. Rust mutation API + LocalAppClient で FullOS の差分更新を通す
4. VerifyLineage（真正性チェック）を UI から呼べるようにする
5. Hono + D1 で同じ application を Workers に載せる（HttpAppClient）
6. Supabase Auth（クラウドのみ）
7. R2 添付 / CSV import / 簡易 formula
