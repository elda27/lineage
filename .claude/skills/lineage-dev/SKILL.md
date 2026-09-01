---
name: lineage-dev
description: Develop the Lineage record-platform in this repo following its DDD layout and dual deploy targets. Use when adding/changing a feature, use case, entity, repository, table schema, Hono/Workers route, or Tauri local wiring — anything touching core/domain, core/app, core/infra, worker/, src-tauri/, db/schema.sql, or the Lineage hash-chain.
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
2. **永続化contract共有 / migration分離**: ローカル SQLite は `lineage-core/src/infra/sqlite_migrations/` の append-only chain、クラウド D1 は `db/schema.sql` と D1 migration が所有する。
3. **Lineage 真正性**: Lineage は append-only。`content_hash`/`prev_hash` の hash-chain を切らない。`LineageLedger` はローカル/クラウド共通。
4. **DDD 依存方向**: features(presentation)/infra → app → domain。`core/domain` は外側を import しない。

## 機能追加のワークフロー

注文に応じて必要な層だけ触る。Lineage を生む操作かどうかで分岐する。

### 1. ドメインから設計する
- `core/domain/<aggregate>/` にエンティティ・値オブジェクトを追加。
- 永続化が要るなら `core/domain/ports/` に Repository インターフェースを定義（実装はまだ書かない）。
- domain では DB / fetch / Tauri / Workers の API を一切 import しない。純粋な TS のみ。

### 2. ユースケースを書く
- `core/app/<機能>/<UseCase>.ts`。コンストラクタで port(interface) を受け取る。
- 具体的な DB 実装には依存しない。
- Rust 側も同じ規則: `lineage-core/src/app/<機能>/<use_case>.rs` に置き、
  機能の `mod.rs` で `pub use` して `lineage_core::app::<機能>::<UseCase>` で呼べるようにする。
  機能の名前は domain 側の集約（`domain::automation` ほか）と1対1に対応させる。

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
- `core/infra/persistence/sqlite/` と `.../d1/` の **両方** に port 実装を追加。
- domain contract は共有するが、SQL と migration は各 persistence adapter が所有する。
- ローカル schema 変更は番号付き migration を追加し、公開済み migration を編集しない。
- D1 schema 変更は `db/schema.sql` と対応する D1 migration を更新する。

### 5. 両ターゲットへ配線（composition root）
- クラウド: `worker/index.ts` に Hono ルートを足し、`D1*Repository` を組み立てて application を呼ぶ。
  認証が要るルートは JWT ミドルウェア配下に置く。
- ローカル: `src/shared/api/LocalAppClient.ts` に `Sqlite*Repository` で同じ application を配線。
- UI が使う `src/shared/api/ApplicationPort.ts`（インターフェース）にメソッドを追加し、
  Local/Http 両実装をそろえる。

### 6. UI（features）
- `src/features/<機能>/ui/` に画面を、`src/features/<機能>/service/` にその画面のための
  hooks・表示用モデルを置く。既存の機能に属さないなら新しい `features/<機能>/` を作る。
- 機能をまたぐもの（ApplicationPort・共通部品・書式）は `src/shared/{api,ui,format.ts}`。
- 必ず `ApplicationPort` 越しに呼ぶ。Local か Http かを UI は知らない。
- import は別名で書く（`@core/*` = core/、`@/*` = src/）。

## 完了前チェックリスト
- [ ] `core/domain` が外側の層を import していない。
- [ ] ユースケースが port(interface) のみに依存している。
- [ ] Lineage を生む変更で hash-chain を切っていない（append-only / prev_hash 連結 / 同一 tx）。
- [ ] sqlite と d1 の port 実装が両方そろっている。
- [ ] ローカル schema 変更は新しい append-only migration、D1 変更は `db/schema.sql` と D1 migration に反映した。
- [ ] `ApplicationPort` の Local/Http 両実装と、worker ルートが配線済み。
- [ ] `pnpm build` が通る。Lineage に触れたら `VerifyLineage` 相当の検証が通る。
- [ ] Tauri deploy を壊していない。

## よくある誤り
- domain から `@tauri-apps/*` や `fetch` を直接呼ぶ → 必ず port 経由。
- ローカルとクラウドでハッシュ計算や正規化を分岐 → 1本に保つ。
- link を上書きして履歴を「修正」する → append-only。訂正は新しい link を足す。
