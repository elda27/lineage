---
name: lineage-dev
description: Develop the Lineage record-platform in this repo following its DDD layout and dual deploy targets. Use when adding/changing a feature, use case, entity, repository, table schema, Hono/Workers route, or Tauri local wiring — anything touching core/domain, core/application, core/infrastructure, worker/, src-tauri/, db/schema.sql, or the Lineage hash-chain.
---

# lineage-dev

このリポジトリ（記録特化 Excel + Lineage プラットフォーム）の機能を、
DDD レイヤ構成・デュアルデプロイ・Lineage 真正性を壊さずに継続開発するための手順。

正本ドキュメント: `doc/MINIMAL_ARCHITECTURE.md`、ルール: `AGENT.md`。
作業前にこの2つを必ず読むこと。

## 4つの不変条件（変更前にこれを満たすか確認）

1. **デュアルデプロイ / 接続**: Tauri と Hono+Cloudflare Workers の両方で動く。frontend(特に Tauri)は
   ローカル接続(SQLite, 認証なし=`LocalAppClient`)とクラウド接続(D1, JWT 認証アリ=`HttpAppClient`)の両モードを取りうる。
   認証の有無は接続モードで決まる。新ユースケースは Local/Http 両 ApplicationPort 実装に配線する。
2. **永続化1スキーマ**: ローカル=SQLite(`@tauri-apps/plugin-sql`)、クラウド=D1。スキーマは `db/schema.sql` 1本。
3. **Lineage 真正性**: Lineage は append-only。`content_hash`/`prev_hash` の hash-chain を切らない。`LineageLedger` はローカル/クラウド共通。
4. **DDD 依存方向**: presentation/infrastructure → application → domain。`core/domain` は外側を import しない。

## 機能追加のワークフロー

注文に応じて必要な層だけ触る。Lineage を生む操作かどうかで分岐する。

### 1. ドメインから設計する
- `core/domain/<aggregate>/` にエンティティ・値オブジェクトを追加。
- 永続化が要るなら `core/domain/ports/` に Repository インターフェースを定義（実装はまだ書かない）。
- domain では DB / fetch / Tauri / Workers の API を一切 import しない。純粋な TS のみ。

### 2. ユースケースを書く
- `core/application/<UseCase>.ts`。コンストラクタで port(interface) を受け取る。
- 具体的な DB 実装には依存しない。

### 3. Lineage を生むなら hash-chain を通す（最重要）
変換(memo, derived_from, attachment など)を記録する操作は必ず:
```
const prev = await lineageRepo.lastLink(workspaceId);
const link = ledger.appendNext(prev, { source, target, relationType, actor, createdAt });
// document/asset の insert と link の append を同一トランザクションで確定
//   - D1: c.env.DB.batch([...])
//   - SQLite: BEGIN/COMMIT
await lineageRepo.append(link);
```
- link は **更新・削除しない**（append-only）。
- `seq` は連番、`prev_hash` は直前の `content_hash`、先頭は genesis。
- ハッシュ対象は正規化(キー順固定)した JSON。`canonicalize` を必ず通す。

### 4. インフラ実装を両方そろえる
- `core/infrastructure/persistence/sqlite/` と `.../d1/` の **両方** に port 実装を追加。
- SQL 文はほぼ共通。実行ハンドルだけ違う（plugin-sql の `Database` か `D1Database`）。
- スキーマ変更時は `db/schema.sql` を更新し、対応する D1 マイグレーションも用意する。

### 5. 両ターゲットへ配線（composition root）
- クラウド: `worker/index.ts` に Hono ルートを足し、`D1*Repository` を組み立てて application を呼ぶ。
  認証が要るルートは JWT ミドルウェア配下に置く。
- ローカル: `src/app-client/LocalAppClient.ts` に `Sqlite*Repository` で同じ application を配線。
- UI が使う `src/app-client/ApplicationPort.ts`（インターフェース）にメソッドを追加し、
  Local/Http 両実装をそろえる。

### 6. UI
- `src/presentation/` に追加。必ず `ApplicationPort` 越しに呼ぶ。Local か Http かを UI は知らない。

## 完了前チェックリスト
- [ ] `core/domain` が外側の層を import していない。
- [ ] ユースケースが port(interface) のみに依存している。
- [ ] Lineage を生む変更で hash-chain を切っていない（append-only / prev_hash 連結 / 同一 tx）。
- [ ] sqlite と d1 の port 実装が両方そろっている。
- [ ] スキーマ変更を `db/schema.sql` と D1 マイグレーションの両方へ反映した。
- [ ] `ApplicationPort` の Local/Http 両実装と、worker ルートが配線済み。
- [ ] `pnpm build` が通る。Lineage に触れたら `VerifyLineage` 相当の検証が通る。
- [ ] Tauri deploy を壊していない。

## よくある誤り
- domain から `@tauri-apps/*` や `fetch` を直接呼ぶ → 必ず port 経由。
- ローカルとクラウドでハッシュ計算や正規化を分岐 → 1本に保つ。
- link を上書きして履歴を「修正」する → append-only。訂正は新しい link を足す。
