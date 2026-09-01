---
id: ADR-0011
title: local / server 同期は DB ではなく versioned domain mutation を交換する
status: accepted
date: 2026-09-02
area:
  - data
  - integration
  - domain
scope:
  - application
owners:
  - lineage
related:
  - ADR-0004
  - ADR-0009
supersedes: []
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/23
---

# ADR-0011: local / server 同期は DB ではなく versioned domain mutation を交換する

## Context

SQLite file や table row の replication は local/server schema を固定し、domain-level permission、
idempotency、conflict、partial retry を storage mechanism に隠す。Git push/pull も canonical state と
content revision の一部には利用できるが、すべての entity と tenant semantics を扱う protocol には
ならない。

offline-first operation を維持しながら、一つの authoritative revision と明示的な conflict を扱える
sync contract が必要である。

## Decision

local/server sync protocol は versioned, typed domain mutation/event を交換する。最低限、mutation は
次を持つ。

```text
Mutation {
  protocol_version
  operation_id
  actor_id
  workspace_id
  entity_kind
  entity_id
  operation
  payload
  base_revision
  created_at
}
```

`operation_id` は end-to-end idempotency key とする。同じ ID と同じ payload の再送は二重適用せず、
異なる payload での再利用は拒否する。transport retry は同じ operation ID を維持する。

local operation は一つの SQLite transaction で local projection と outbox を更新する。sync worker は
outbox を server へ push し、server が permission と base revision を検証して canonical revision を
返す。client は cursor を用いて remote change を pull し、projection を更新する。protocol version は
SQLite/server schema version から独立させる。

authority は workspace mode ごとに明示する。

- local-only workspace: local structured state が authoritative。
- sync-enabled workspace: server が canonical revision を採番し、local state は offline-capable projection。
  offline mutation は pending proposal として outbox に保持され、server acceptance までは canonical
  revision を主張しない。

conflict policy は entity kind ごとに定義し、全 entity へ global last-write-wins を適用しない。

- immutable Lineage record: append-only、競合する rewrite は拒否
- tag assignment: operation semantics に基づく add/remove set
- task/state transition: base revision を検証し、許可された transition のみ適用
- note content: content revision を保持し、automatic 3-way merge が安全な場合だけ提案する。それ以外は
  user-visible conflict として保持
- publication: append-only state transition と idempotent delivery command

conflict や permission failure を成功として扱う fallback を禁止する。pending mutation と user data を
保持し、原因、current revision、必要な次操作を返す。

## Options considered

### Option 1: SQLite / server database replication

採用しない。schema、DB product、permission model が密結合になる。

### Option 2: Git remote をすべての entity の sync protocol にする

採用しない。non-content state、authorization、outbox、target delivery の semantics を表現しにくい。

### Option 3: typed mutation、outbox、cursor、domain-specific conflict policy

採用する。storage backend から独立した offline-first sync を構成できる。

## Consequences

### Positive

- local と server の database schema を独立して変更できる。
- retry と duplicate を operation ID で安全に扱える。
- conflict と permission failure を利用者へ明示できる。
- entity ごとに適切な merge/transition rule を選べる。

### Negative

- outbox、cursor、canonical revision、conflict UI が必要になる。
- local projection は server state と一時的に異なるため、pending status を model に含める必要がある。
- protocol compatibility と migration を DB migration とは別に管理する必要がある。

### Follow-up

- ADR-0004 の local mutation record を sync outbox contract へ写像する。
- server commit/pull endpoint の protocol versioning を設計する。
- entity kind ごとの conflict matrix と property test を追加する。
- backup/restore 時に pending outbox と cursor を保全する。

## References

- [Issue #23](https://github.com/elda27/lineage/issues/23)
- [ADR-0004](./0004-rust-owned-fullos-delta-mutations.md)
- [ADR-0009](./0009-separate-structured-state-content-and-sync-persistence.md)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
