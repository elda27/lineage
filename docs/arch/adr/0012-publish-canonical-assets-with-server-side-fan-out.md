---
id: ADR-0012
title: canonical asset を複製せず publication と server-side fan-out で複数 tenant へ提供する
status: accepted
date: 2026-09-02
area:
  - domain
  - data
  - integration
  - security
scope:
  - application
owners:
  - lineage
related:
  - ADR-0009
  - ADR-0011
supersedes: []
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/23
---

# ADR-0012: canonical asset を複製せず publication と server-side fan-out で複数 tenant へ提供する

## Context

一つの source asset を複数 tenant で利用する場合、tenant ごとに source をコピーすると identity、
revision、修正、revocation、provenance が分岐する。また client が複数 target へ独立 write すると、
一部成功・一部失敗を client-side distributed transaction として処理する必要がある。

source ownership を一つに保ち、共通 automation と tenant-specific automation の両方を安全に実行する
domain model が必要である。

## Decision

source asset は一つの owner workspace / tenant と一つの canonical revision を持つ。別 tenant へは
asset copy ではなく、revision を明示した `Publication` を作成する。

```text
Publication {
  publication_id
  source_asset_id
  source_revision
  source_tenant_id
  target_tenant_id
  access_scope
  processing_policy
  automation_binding
  state
}
```

`access_scope` は少なくとも `source-and-result` と `result-only` を区別する。target tenant は grant が
明示した範囲を超えて source content を読めない。result-only publication では server-side processor
だけが source を読み、target は derived artifact と許可された provenance のみを受け取る。

processing は次の両方を表現できる。

1. source owner が一度 common automation を実行し、その canonical result を複数 target へ配信する。
2. publication ごとに target-specific automation/policy を適用し、target-specific artifact を生成する。

client は publication command を authoritative service へ一度だけ commit する。server-side fan-out
worker が target ごとの delivery record、idempotency key、attempt、last error、next retry、delivered
revision を管理する。一 target の失敗で成功済み target を rollback せず、失敗 target だけを
bounded retry する。恒久 error は明示的な failed state と corrective action を持つ。

publication state transition と derived artifact は source asset、source revision、automation revision、
processor、target、output digest を provenance edge で結ぶ。

revocation は future access、未実行 processing、今後の delivery を停止する。既に materialize された
artifact は自動で物理削除せず、target retention policy と法的/監査要件に従う。source owner が purge
を要求できる場合は、通常の revoke と別の明示的・監査可能な operation として定義する。

## Options considered

### Option 1: source asset を target tenant ごとにコピーする

採用しない。identity と revision が分岐し、修正・revocation・provenance が不明確になる。

### Option 2: client が全 target へ直接 write する

採用しない。一部失敗、retry、credential、authorization を client が担うことになる。

### Option 3: publication reference と server-side fan-out を使う

採用する。一つの Source of Truth と target ごとの delivery lifecycle を両立できる。

## Consequences

### Positive

- source revision と ownership を一つに保てる。
- target ごとに access scope と automation を変えられる。
- partial failure を per-target delivery state として安全に retry できる。
- source から output までの Lineage/provenance を追跡できる。

### Negative

- publication、grant、delivery、artifact retention の state machine が必要になる。
- result-only processing では server-side isolation と secret management が必要になる。
- revocation と purge の違いを UI/API で明確に伝える必要がある。

### Follow-up

- Publication / Delivery / DerivedArtifact の domain contract を定義する。
- permission test で source-and-result / result-only の情報境界を検証する。
- fan-out worker の idempotency と retry policy を設計する。
- purge operation と retention policy は実装前に別 ADR または security review で確定する。

## References

- [Issue #23](https://github.com/elda27/lineage/issues/23)
- [ADR-0009](./0009-separate-structured-state-content-and-sync-persistence.md)
- [ADR-0011](./0011-sync-versioned-domain-mutations-not-databases.md)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
