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
    └─ LocalAppClient（Application Service を in-process 呼び出し, TS）
          └─ SqliteAssetRepository
                └─ @tauri-apps/plugin-sql → SQLite ファイル
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
- domain / application 層は両モードで「同一の TS ソース」を再利用する。
- frontend は「ApplicationPort」越しに呼ぶ。実装は2つ。
    - LocalAppClient … in-process でアプリケーションサービスを直接呼ぶ（認証なし）
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
| API 層        | in-process 呼び出し          | Hono REST                    |
| DB            | SQLite(@tauri-apps/plugin-sql)| Cloudflare D1               |
| スキーマ      | schema.sql（共通）           | schema.sql（共通）           |
| ファイル      | ローカル FS                  | R2                           |
| 認証          | なし（単一利用者）           | Supabase Auth (JWT) アリ     |
| Lineage 台帳  | 同一の hash-chain ロジック   | 同一の hash-chain ロジック    |

---

2. DDD を意図したフォルダ構成

レイヤを物理ディレクトリで分離する。依存方向は外→内（presentation/infrastructure → application → domain）。
domain は他のどの層にも依存しない。

lineage/
├─ src/                         # フロントエンド（presentation）
│   ├─ presentation/            # React UI（画面・コンポーネント）
│   │   ├─ pages/               #   /tables, /tables/:id, /rows/:id
│   │   └─ components/
│   └─ app-client/              # ApplicationPort と2実装
│       ├─ ApplicationPort.ts   #   UI が依存するインターフェース
│       ├─ LocalAppClient.ts    #   Tauri: in-process 呼び出し
│       └─ HttpAppClient.ts     #   Cloud: fetch
│
├─ core/                        # フレームワーク非依存の中核（両ターゲット共有）
│   ├─ domain/                  # ★ 何にも依存しない
│   │   ├─ asset/               #   Asset エンティティ・値オブジェクト
│   │   ├─ lineage/             #   Lineage エンティティ + LineageLedger(真正性)
│   │   ├─ table/               #   Table/Row/Cell
│   │   ├─ shared/              #   Hash, Id, Clock などの値オブジェクト
│   │   └─ ports/               #   Repository インターフェース（実装は infra）
│   ├─ application/             # ユースケース（アプリケーションサービス）
│   │   ├─ CreateTable.ts
│   │   ├─ AppendRow.ts
│   │   ├─ WriteMemo.ts         #   行メモ→document→lineage を1トランザクションで
│   │   ├─ AttachFile.ts
│   │   └─ VerifyLineage.ts     #   hash-chain 検証
│   └─ infrastructure/          # ports の実装（外側）
│       ├─ persistence/
│       │   ├─ sqlite/          #   SqliteAssetRepository ほか（plugin-sql / better-sqlite3）
│       │   └─ d1/              #   D1AssetRepository ほか
│       ├─ crypto/              #   WebCrypto を使う Sha256Hasher
│       └─ storage/             #   LocalFileStorage / R2Storage
│
├─ worker/                      # クラウドエントリ（Cloudflare Workers）
│   ├─ index.ts                 #   Hono アプリ。auth → application を呼ぶだけ
│   └─ wrangler.toml
│
├─ src-tauri/                   # ローカルエントリ（Tauri Rust shell）
│   └─ ...                      #   plugin-sql を有効化。Rust 側は薄い
│
├─ db/
│   └─ schema.sql               # ★ SQLite/D1 共通スキーマ（migration の起点）
│
└─ doc/

依存ルール:
- domain は import で application/infrastructure/presentation を参照しない。
- application は domain と ports(interface) のみに依存する。
- worker / src-tauri / src は「組み立て役(composition root)」であり、
  ここで具体的な Repository 実装を application に注入する。

ローカル側の Rust クレート（lineage-core）:

ローカルのデスクトップ側は Rust で書かれた3つの実行ファイルからなり、
ドメイン・ユースケース・永続化は lineage-core クレート1本を共有する。

lineage-core/           # domain / application / infrastructure（上と同じ層構成）
minos/                  # クイック入力（gpui）。lineage-core に依存
agentos/                # 自動化の実行（CUI・常駐しない）。lineage-core に依存
fullos/src-tauri/       # Tauri シェル。agentos.exe を同梱して呼び出す

fullos が lineage-core を直接リンクしないのは、tauri-plugin-sql(sqlx) と
rusqlite がどちらも native の sqlite3 をリンクしていて同居できないため。
結果として `links` への追記は minos と agentos の2か所だけになり、
どちらも同じ lineage-core のコードを通る（4章の不変条件を保つ）。
fullos の webview は plugin-sql で読み出しと、lineage を生まない行
（automation_rules / settings）の書き込みだけを行う。

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
import { D1AssetRepository, D1LineageRepository } from "../core/infrastructure/persistence/d1";
import { Sha256Hasher } from "../core/infrastructure/crypto/Sha256Hasher";
import { WriteMemo } from "../core/application/WriteMemo";

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

6. ローカル実装: Tauri + SQLite（plugin-sql）

Rust 側は薄く、SQLite アクセスは JS から @tauri-apps/plugin-sql で行う。

src-tauri/Cargo.toml に依存追加:
  tauri-plugin-sql = { version = "2", features = ["sqlite"] }

src-tauri/src/lib.rs:
  tauri::Builder::default()
    .plugin(tauri_plugin_sql::Builder::default().build())

LocalAppClient（src/app-client/LocalAppClient.ts）

import Database from "@tauri-apps/plugin-sql";
import { SqliteAssetRepository, SqliteLineageRepository } from "../../core/infrastructure/persistence/sqlite";
import { Sha256Hasher } from "../../core/infrastructure/crypto/Sha256Hasher";
import { WriteMemo } from "../../core/application/WriteMemo";

export async function createLocalAppClient(): Promise<ApplicationPort> {
  const db = await Database.load("sqlite:lineage.db"); // schema.sql を初回適用
  const assets = new SqliteAssetRepository(db);
  const lineage = new SqliteLineageRepository(db);
  const hasher = new Sha256Hasher();
  return {
    writeMemo: (i) => new WriteMemo(assets, lineage, hasher).execute(i),
    verifyLineage: (id) => new VerifyLineage(lineage, hasher).execute(id),
    // ...他ユースケースも同様に in-process で呼ぶ
  };
}

ローカルでもクラウドでも WriteMemo / LineageLedger は同一コードなので、
真正性(hash-chain)の振る舞いは完全に一致する。

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
3. Tauri plugin-sql + LocalAppClient で WriteMemo を通す
4. VerifyLineage（真正性チェック）を UI から呼べるようにする
5. Hono + D1 で同じ application を Workers に載せる（HttpAppClient）
6. Supabase Auth（クラウドのみ）
7. R2 添付 / CSV import / 簡易 formula
