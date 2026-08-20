---
id: ADR-0004
title: WebView の SQL を読み取り専用とし、データ変更を Rust の mutation 境界へ集約する
status: accepted
date: 2026-08-20
area:
  - application
  - data
  - security
  - quality
scope:
  - application
owners:
  - lineage
related:
  - ADR-0001
  - ADR-0002
supersedes: []
supersededBy: null
discussion: null
---

# ADR-0004: WebView の SQL を読み取り専用とし、データ変更を Rust の mutation 境界へ集約する

## Context

fullos はローカルの SQLite を UI から検索・一覧表示する必要があり、WebView から `tauri-plugin-sql` を利用すると、読み取りモデルを小さな IPC API に分解せず柔軟に組み立てられる。

一方、WebView に SQL の書き込み権限まで与えると、状態変更・設定変更・タグ変更などの mutation が application layer を迂回して直接テーブルへ到達できる。個々の SQL が parameter binding を利用しており SQL injection を避けられていたとしても、書き込み経路が複数存在すると、論理削除、状態遷移、`updated_at`、複数テーブル更新の transaction、lineage/hash-chain、同期や自動化などの不変条件を一部の経路だけが実施しない状態を作りやすい。

したがって、この判断で重視するのは「SQL を WebView に書かせること自体が危険だから禁止する」という単純な security rule ではない。データを変更する入口を application boundary に限定し、アプリケーション全体の整合性を維持することが主目的である。

また、すべての読み取りを Rust command に変換すると、一覧、検索、フィルタ、集計のたびに専用 IPC を増やす必要があり、ローカルアプリとしての実装量と UI 開発コストが増える。

## Decision

fullos の WebView に許可する SQL capability は `sql:default` に限定する。WebView に `sql:allow-execute` を付与しない。

`sql:default` による `load` / `select` / `close` は read model の構築に利用してよい。WebView から SQLite に対する INSERT / UPDATE / DELETE を直接実行してはならない。

データを変更する操作は、次の規則に従う。

1. UI は `setMemoDone`、`trashMemo`、`saveAutomationRule`、`updateTag` のような意味を持つ application operation を呼ぶ。
2. Tauri command の Rust 側が mutation を受け取り、SQLite の書き込みを実行する。
3. Rust command は WebView から任意 SQL、SQL fragment、table name、column name を受け取る汎用 SQL executor として設計してはならない。
4. 複数テーブルを一つの操作として変更する場合は Rust 側で transaction を張り、操作単位で atomic に確定する。
5. lineage/hash-chain の追加・変更を伴う操作は、既存の lineage application boundary を通し、単純なテーブル mutation として実装してはならない。
6. 読み取り SQL でも値は parameter binding を利用し、ユーザー入力を SQL 文字列へ連結しない。

この境界は fullos のローカル SQLite アクセスに適用する。将来クラウド永続化や別の UI client を追加する場合も、「read model は用途に応じて最適化してよいが、mutation は application operation を通す」という原則を維持する。

## Options considered

### Option 1: WebView は読み取り専用 SQL、mutation は Rust に集約する

採用案。

読み取りでは SQL の表現力とローカル実行の利便性を維持しつつ、書き込みだけを意味のある application operation に限定できる。UI の検索・一覧機能に専用 IPC を大量に追加せず、データ整合性を守る境界を明確にできる。

欠点として、読み取りと書き込みでアクセス経路が異なり、Rust 側にも SQLite 接続コードが必要になる。

### Option 2: 読み取り・書き込みのすべてを Rust command に集約する

最も強い境界を作れる。DB schema を WebView から完全に隠し、将来 SQLite 以外へ移行する場合も UI への影響を抑えやすい。

一方で、一覧、検索、フィルタ、集計の read model ごとに IPC API と serialization を定義する必要がある。ローカル UI の変更頻度に対して abstraction cost が高いため採用しない。

### Option 3: WebView に `sql:allow-execute` を付与して CRUD を直接実行する

実装量が最も少なく、単純な CRUD では経路も短い。parameter binding により SQL injection リスクも抑制できる。

しかし、書き込み側の application rule を UI repository ごとに複製しやすく、別 client や automation が増えたときに同じ不変条件を守る保証が弱い。性能上の利点も、ローカル SQLite の通常の mutation 頻度ではこの整合性上の欠点を上回らないため採用しない。

## Consequences

### Positive

- mutation の入口が限定され、論理削除、状態遷移、timestamp、transaction などの不変条件を一箇所で維持しやすい。
- WebView に任意の SQL 書き込み能力を与えないため、UI 側の不具合や脆弱性が DB 全体の任意 mutation に直結しにくい。
- 読み取りは `sql:default` を利用できるため、検索・一覧・集計の開発速度とローカル実行性能を維持できる。
- capability 設定そのものが architecture boundary の検査点になり、WebView から直接 mutation を追加すると権限エラーとして顕在化する。
- 複数テーブル mutation を Rust 側 transaction にまとめられる。

### Negative

- fullos は read 用の plugin-sql 接続と Rust 側 mutation の双方を扱うため、永続化コードが二つの実行面に存在する。
- 新しい mutation を追加するたびに Tauri command を追加する必要がある。
- frontend から直接 SQL を書く場合より、小さな変更でも Rust/TypeScript 間の interface 更新が必要になる。
- schema 変更時は read SQL と Rust mutation の双方が同じ schema contract に従っていることを検証する必要がある。

### Follow-up

- capability review では `sql:allow-execute` を原則禁止し、必要になった場合は本 ADR を置き換える新しい ADR を要求する。
- WebView からの直接 `execute()` 呼び出しを静的解析またはテストで検出できるようにする。
- mutation command が raw SQL を受け取らないこと、複数テーブル更新が transaction になっていることをレビュー項目へ追加する。
- lineage/hash-chain を伴う mutation は通常の state mutation と分け、既存の lineage 書き込み境界を利用する。

## References

- Tauri SQL plugin capability: `sql:default`, `sql:allow-execute`
- `docs/concept/MINIMAL_ARCHITECTURE.md`
- ADR-0001: 組み込みタグの状態を document_states に分離し、削除は論理削除とする
- ADR-0002: タグ自動化の定義に just recipe を用い、Lineage は DAG engine を持たない
