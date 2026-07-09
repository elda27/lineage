# docs

本リポジトリのドキュメント索引。

## minos/ — 超軽量DB登録アプリ minos(現行開発対象)

| ドキュメント | 内容 |
|---|---|
| [minos/CONCEPT.md](minos/CONCEPT.md) | 思想・価値。一瞬で開き一瞬で入力する、入力特化アプリの設計思想 |
| [minos/USECASE.md](minos/USECASE.md) | 具体シナリオ。todo/メモの雑登録(自動タグ・スクショ)と後続フロー |
| [minos/DESIGN.md](minos/DESIGN.md) | 技術設計。Rust/GPUI、Windows連携、共通SQLiteスキーマ、hash-chain |

## concept/ — Lineage プラットフォーム構想

Record-Centric Lineage Platform(表計算 + 記録 + 履歴管理)の構想ドキュメント。
minos はこの構想のデータモデルと hash-chain を踏襲した入力コンポーネントである。

| ドキュメント | 内容 |
|---|---|
| [concept/PARTIAL_SPEC.md](concept/PARTIAL_SPEC.md) | MVP プロダクト仕様。課題・ドメインモデル(Asset/Lineage/Table/Row/Cell) |
| [concept/FUTURE_SPEC.md](concept/FUTURE_SPEC.md) | 将来ビジョン(Lineage Projection System) |
| [concept/MINIMAL_ARCHITECTURE.md](concept/MINIMAL_ARCHITECTURE.md) | 技術アーキテクチャの正本。共通スキーマ(schema.sql)と LineageLedger(hash-chain)仕様 |
