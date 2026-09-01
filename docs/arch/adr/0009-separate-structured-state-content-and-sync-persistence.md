---
id: ADR-0009
title: 永続化を structured state / content / sync の契約へ分離する
status: accepted
date: 2026-09-02
area:
  - data
  - application
  - integration
scope:
  - application
owners:
  - lineage
related:
  - ADR-0004
  - ADR-0005
  - ADR-0008
supersedes: []
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/23
---

# ADR-0009: 永続化を structured state / content / sync の契約へ分離する

## Context

structured metadata、note body、binary attachment、automation state、sync cursor は、容量、transaction、
versioning、retention、共有範囲が異なる。これらを一つの SQLite / D1 schema と repository API に
集約すると、local と server が同じ schema を使うことが設計上の制約になり、content backend や
sync protocol を独立して変更できない。

SQLite は local application に適しているが、DB file 自体を server へ複製する責務まで持たせるべき
ではない。また v0.0.9 で全 content を一度に別 backend へ移すと、data migration の危険が大きい。

## Decision

永続化の論理契約を次の三つへ分離する。

```text
Structured State
Content
Sync
```

### Structured State

identity、workspace、canonical record index、tag assignment、automation run、publication state、
entity revision など query と transaction を必要とする構造化状態を扱う。local implementation の
source of truth は SQLite とする。server implementation は PostgreSQL 等を選択でき、local SQLite
と同一 table/schema を持つ必要はない。

local SQLite schema は Rust 側の versioned migration chain が単独で所有する。すべての local
entry point は repository query より前に migration を完了する。

### Content

note body、large text、image、PDF、generated artifact、attachment を `ContentStore` port で扱う。
domain/canonical record は physical path や provider URL ではなく、次のような logical reference を持つ。

```text
ContentRef {
  content_id
  digest
  media_type
  byte_length
}
```

physical location、inline / file / Git / object storage の選択は ContentStore implementation が所有する。
v0.0.9 では既存 text を失わず boundary を導入するため、SQLite-backed text store と
filesystem-backed binary storeを明示的な初期 implementation として利用できる。これは暗黙 fallback
ではなく、media type と policy により composition root が選択する backend である。model file は
user content とは別の managed model cache とし、ContentStore や Git に格納しない。

### Sync

local/server 間では DB file、table row、schema を複製せず、typed domain mutation/event を同期する。
local projection と pending mutation outbox は同一 SQLite transaction で更新する。sync cursor、
delivery state、operation idempotency は Sync contract が所有する。

Structured State repository、ContentStore、Sync port は別 interface とし、一つの concrete database
を使用する場合でも責務を結合しない。canonical identity と content digest は backend location より
長く存続する。

## Options considered

### Option 1: local と server で同じ monolithic schema を維持する

採用しない。DB product と deployment topology が domain contract を制約する。

### Option 2: v0.0.9 ですべての body と attachment を filesystem/object storage へ移す

採用しない。boundary 導入と大規模 data migration を同時に行う必要はなく、失敗時の影響が大きい。

### Option 3: port を分離し、既存 data を保ったまま backend を段階的に置き換える

採用する。logical ownership を先に固定し、安全に backend を移行できる。

## Consequences

### Positive

- SQLite と server DB の schema を独立して進化させられる。
- note body や binary content を別 backend へ移せる。
- sync protocol が storage product に依存しない。
- content digest と Lineage provenance を physical URI から独立して保持できる。

### Negative

- 一つの user operation が state と content の複数 store にまたがる場合、明示的な commit protocol と
  failure cleanup が必要になる。
- 初期期間は既存 column と新しい logical port が併存する migration step を管理する必要がある。
- backup / restore は SQLite だけでなく content store と sync metadata を一貫して扱う必要がある。

### Follow-up

- `rusqlite_migration` による local SQLite migration chain を導入する。
- `ContentStore` と `ContentRef` を定義し、physical URI を domain API から隠す。
- local/server 共用 `db/schema.sql` 前提を廃止する。
- backup manifest に structured state、content、digest、schema version を含める。

## References

- [Issue #23](https://github.com/elda27/lineage/issues/23)
- [Issue #28](https://github.com/elda27/lineage/issues/28)
- [ADR-0004](./0004-rust-owned-fullos-delta-mutations.md)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
