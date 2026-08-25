---
id: ADR-0004
title: FullOS の書き込みを Rust 所有の差分 mutation API に集約する
status: accepted
date: 2026-08-25
area:
  - application
  - integration
  - data
  - platform
scope:
  - application
owners:
  - lineage
related:
  - ADR-0001
supersedes:
  - ADR-0001
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/31
---

# ADR-0004: FullOS の書き込みを Rust 所有の差分 mutation API に集約する

## Context

FullOS の WebView は SQLite を `tauri-plugin-sql` で読み書きしていた。
この経路は capability の設定に依存し、書き込み権限の不整合を実行時まで検出できない。
また、複数 SQL で構成される更新を WebView 側から実行すると、途中まで反映された状態や、
将来の同期で再利用できない全体スナップショットが生じる。

組み込みタグの状態を `document_states` に分離し、削除を論理削除とする判断は維持する。
変更するのは、その状態や FullOS 管理データを書き込む責務の境界である。

## Decision

FullOS のローカル書き込みは、WebView から Rust の mutation API を呼び出す経路に集約する。
WebView は SQL、テーブル名、SQL の bind 値を受け取る API を持たない。
`tauri-plugin-sql` はローカル読み出し専用とし、書き込み用の capability を要求しない。
対象はメモ状態、タグ、設定、自動化ルールなどの可変な状態・構成である。新規 document と
append-only の lineage を確定する既存の capture / automation result 経路は、引き続き Rust の
専用 application service と transaction を使う。

### 差分 mutation 契約

API は entity 全体を置き換える full update ではなく、変更部分だけを表す typed patch/delta を受け取る。
Tauri command の最小の外形は次のとおりとする。

```text
MutationRequest {
  operationId: string,
  workspaceId: string,
  baseRevision: integer?,
  operation: MutationOperation
}
```

`MutationOperation` は entity ID と typed patch を含むドメイン操作として表現する。例として、
メモ状態の `memo_state_patch`、`archive_completed_tasks`、タグや自動化ルールの変更がある。
未変更、値の設定、値の消去を
区別できる形にし、JSON の省略と `null` を同じ意味として扱わない。

作成操作では entity ID と `baseRevision` を省略でき、entity ID は Rust が冪等キーから
決定的に割り当てる。`baseRevision` が指定された更新では、Rust が現在値を読み、値が一致する
場合だけ per-entity revision を1増やして確定する。不一致は `conflict` とし、上書きや暗黙の
full update を行わない。revision は Rust が単調増加で採番し、WebView の時計や revision を
信用しない。transport 契約上は互換ローカル adapter のため省略を許すが、revision を返す
read model と同期・remote adapter では既存 entity の更新時に必須とする。

`operationId` は mutation の冪等キーとする。同じ operationId の再送は二重適用しない。
適用済みなら `duplicate`、競合済みなら最初の `conflict` とその時点の revision を返す。
競合を解消して再送する場合は、最新 revision を読み直したうえで新しい operationId を使う。
同じ operationId を別の payload へ使い回した要求は拒否する。応答には operationId、
entity kind/id、`applied` / `duplicate` / `conflict` の状態、revision、適用または競合の確定時刻を含める。
呼び出し adapter は request を先に確定し、transport retry でも同じ operationId を再利用する。

### トランザクションと将来の同期

1つの mutation request は、検証、対象 entity の更新、revision の増加を1トランザクションで
確定する。タグ本体と binding のように複数テーブルへまたがる変更も同じ境界に含め、
一部だけ成功した状態を作らない。`document_states.deleted_at` による論理削除を使い、
`documents` や append-only の `links` は物理削除しない。

FullOS は mutation sidecar の起動を直列化する。SQLite 側でも busy timeout を設定して
`BEGIN IMMEDIATE` 相当で writer 予約を取得し、minos や別 agentos プロセスとの短い競合を
待ってから transaction を開始する。

mutation の operationId、entity、operation kind、status、patch payload、baseRevision、作成日時を
`local_mutations` へ保存する。将来 outbox へ渡すのは `status = 'applied'` の delta だけとし、
`conflict` 行は operationId の予約と診断に使う。revision は `entity_revisions` で entity ごとに
管理する。ネットワーク同期や conflict merge 自体はこの ADR の範囲外だが、同期単位は
DB ファイルや全体行ではなくこの mutation とする。

## Options considered

### Option 1: WebView に `sql:allow-execute` を追加する

採用しない。Issue #31 の ACL エラーだけは隠せるが、書き込み責務、トランザクション、
revision、冪等性を WebView と SQL 文字列へ分散させる。

### Option 2: entity 全体を Rust へ送る full update

採用しない。未変更フィールドの上書き、同時編集のロストアップデート、将来の差分同期との
不整合を招くためである。

### Option 3: Rust の typed delta mutation API に集約する

採用する。UI と SQLite の境界を固定しながら、Rust が domain validation、revision、
transaction、idempotency を一元的に管理できる。将来 HTTP/API の PATCH や outbox へ写像できる。

## Consequences

### Positive

- WebView の SQLite 書き込み ACL に依存しない。
- 複数 SQL の更新を Rust のトランザクションへ集約できる。
- operationId と revision により再送と競合を明示的に扱える。
- 将来の local/server sync を mutation log/outbox 単位で追加できる。
- `document_states` の分離と論理削除、Lineage の append-only 性を維持できる。

### Negative

- TypeScript の `ApplicationPort` と Rust command の入出力契約を同期して保守する必要がある。
- per-entity revision と冪等キーを保存するスキーマ移行が必要になる。
- plugin-sql の読み出しと Rust の書き込みは別接続になるため、sidecar の直列化、WAL、
  busy timeout、immediate transaction の方針を維持する必要がある。

## References

- [Issue #31](https://github.com/elda27/lineage/issues/31)
- [ADR-0001](./0001-store-builtin-tag-state-outside-document-meta.md)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
