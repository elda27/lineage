# AGENT.md

このリポジトリで作業する AI エージェント / 開発者向けのガイド。
仕様の詳細は `doc/` を、開発手順の定型は skill `lineage-dev`（`.claude/skills/lineage-dev/`）を参照。

## プロダクト概要

「スマホでも PC でも使える記録特化 Excel」。表計算データと人間の記録(メモ)を統合し、
「何から何が作られたか」という変換履歴(Lineage)を**真正性を担保した形で**残す。

- `doc/MINIMAL_ARCHITECTURE.md` — MVP の技術アーキテクチャ（正本）
- `doc/PARTIAL_SPEC.md` — Record-Centric な MVP 仕様
- `doc/FUTURE_SPEC.md` — 将来構想(Lineage Projection System)

## 譲れない設計原則

1. **デュアルデプロイ / デュアル接続**: Tauri デスクトップと Hono+Cloudflare Workers の両方に deploy できること。
   さらに frontend(特に Tauri)は **ローカル接続(SQLite, 認証なし)** と **クラウド接続(D1, 認証アリ)** の
   両モードを取りうる。認証の有無はデプロイ先ではなく接続モード(= 使う ApplicationPort 実装)で決まる:
   `LocalAppClient`=認証なし、`HttpAppClient`=JWT 必須。どちらも同じ `core/`(domain/application)を再利用する。
2. **永続化の差し替え**: ローカルは SQLite(`@tauri-apps/plugin-sql`)、クラウドは Cloudflare D1。
   D1 は SQLite 互換なので **スキーマは `db/schema.sql` の1本**。SQL は infrastructure 層に閉じる。
3. **Lineage の真正性**: Lineage は append-only。`content_hash` / `prev_hash` による hash-chain で
   改ざんを検知可能にする。ハッシュ計算ロジック(`LineageLedger`)はローカル/クラウドで完全に同一。
4. **DDD のレイヤ分離**: 依存方向は presentation/infrastructure → application → domain。
   `core/domain` は他のどの層にも import 依存しない。

## フォルダ構成（DDD）

```
src/                  presentation: React UI と ApplicationPort(Local/Http の2実装)
core/
  domain/             エンティティ・値オブジェクト・LineageLedger・ports(interface)
  application/        ユースケース(CreateTable, WriteMemo, VerifyLineage ...)
  infrastructure/     ports の実装: persistence/{sqlite,d1}, crypto, storage
worker/               Cloudflare Workers エントリ(Hono)。composition root
src-tauri/            Tauri Rust shell。plugin-sql を有効化。composition root
db/schema.sql         SQLite/D1 共通スキーマ(migration の起点)
doc/                  仕様
```

依存ルール（PR で必ず守る）:
- `core/domain` は application / infrastructure / presentation / worker / src-tauri を import しない。
- `core/application` は domain と ports(interface) のみに依存する。具体的な DB 実装に依存しない。
- 具体実装の注入は composition root（`worker/`, `src-tauri/` 側の `src/app-client/`）でのみ行う。

## 新機能を追加するときの定石

1. `core/domain` にエンティティ／値オブジェクト／必要なら port(interface) を足す。
2. `core/application` にユースケースを足す（domain と port にだけ依存）。
3. Lineage を生む操作なら、必ず `LineageLedger.appendNext` を通して link を追記する
   （prev_hash → content_hash の鎖を切らない。link は更新・削除しない）。
4. `core/infrastructure/persistence/{sqlite,d1}` の両方に port 実装を足す。SQL はほぼ共通。
   スキーマ変更時は `db/schema.sql` と D1 マイグレーションの両方を更新する。
5. 公開するなら `worker/index.ts`（Hono ルート）と `src/app-client/*`（ApplicationPort）の両方に配線する。
6. UI は `src/presentation` に足し、必ず `ApplicationPort` 越しに呼ぶ（Local/Http を意識しない）。

## コマンド

```bash
pnpm install
pnpm dev              # Vite 開発サーバ（フロント）
pnpm build            # tsc && vite build
pnpm tauri dev        # ローカル(デスクトップ)で起動
pnpm tauri build      # デスクトップ配布ビルド

# クラウド(Workers)
wrangler dev                          # ローカルで Workers をエミュレート
wrangler d1 migrations apply lineage  # D1 にスキーマ適用
wrangler deploy                       # Cloudflare へ deploy
```

（`wrangler` / `@cloudflare/workers-types` / `hono` / `jose` / `@tauri-apps/plugin-sql` は
 まだ未導入。`doc/MINIMAL_ARCHITECTURE.md` の実装順に沿って追加していく。）

## やってはいけないこと

- Lineage レコードの UPDATE / DELETE（append-only を破ると真正性が崩れる）。
- domain 層から DB / fetch / Tauri API を直接呼ぶこと。
- ローカルとクラウドでスキーマや Lineage のハッシュ計算を分岐させること（必ず1本に保つ）。
- Tauri deploy を壊す変更（デュアルデプロイは前提）。
