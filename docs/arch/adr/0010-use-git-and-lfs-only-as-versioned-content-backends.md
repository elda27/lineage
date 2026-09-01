---
id: ADR-0010
title: Git / Git LFS を workspace 単位の versioned content backend に限定する
status: accepted
date: 2026-09-02
area:
  - data
  - integration
  - operations
scope:
  - application
owners:
  - lineage
related:
  - ADR-0002
  - ADR-0009
supersedes: []
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/23
---

# ADR-0010: Git / Git LFS を workspace 単位の versioned content backend に限定する

## Context

automation definition、prompt、template、managed script、user/AI editable text は、diff、review、
revision、merge に価値があり Git と相性がよい。large/binary content は Git LFS pointer によって
repository size と object storage を分離できる。

しかし Git remote を Lineage 全体の sync engine とみなすと、tenant authorization、partial delivery、
revocation、target-specific processing、domain conflict、audit を commit/push semantics だけで扱う
ことになる。Git と application sync の責務を区別する必要がある。

## Decision

Git と Git LFS は `ContentStore` / versioning backend の一候補としてのみ利用する。

Git repository の ownership unit は workspace とする。automation definition、prompt、template、
managed script、Git-backed content は、その workspace の repository と revision policy に従う。
user 全体や複数 tenant を一つの暗黙 repository にまとめない。

Git へ格納してよい主な対象は次のとおりである。

- justfile と managed automation recipe
- prompt、template、configuration sample
- Lineage が管理する script
- text content のうち、履歴・diff・merge を利用者が求めるもの
- content manifest と pointer

Git LFS は large/binary content を pointer と外部 object に分離する backend として選択できる。
Lineage 自身は Git LFS server を実装せず、利用する remote/service を adapter と configuration で
指定する。LFS は binary semantic merge を提供しないため、domain conflict policy の代替にしない。

次は Git repository に格納しない。

- SQLite database と WAL
- OS credential store の secret
- API key、access token、private key
- downloaded local LLM model
- ephemeral cache、lock、delivery retry state
- tenant permission の authoritative state

Git commit SHA と domain entity revision は別 identity とする。両者の mapping は provenance として
保存できるが、一方を他方の代用にしない。

Git remote は local/server sync、tenant fan-out、authorization の authoritative transport にしない。
それらは versioned mutation/publication protocol が所有する。

## Options considered

### Option 1: Git を Lineage 全体の sync engine にする

採用しない。authorization、partial delivery、revocation、domain-specific conflict を表現できない。

### Option 2: Git を利用せず、すべて DB/object storage に置く

採用しない。text automation asset の review、diff、AI/user editing に有用な機能を再実装することになる。

### Option 3: workspace 単位の versioned content backend として利用する

採用する。Git の強みを利用しつつ、application sync と tenant semantics を混同しない。

## Consequences

### Positive

- automation asset を通常の Git workflow で review・rollback できる。
- binary は必要に応じて Git LFS/object backend へ分離できる。
- secret、DB、model file が誤って repository に入る境界を明示できる。
- multi-tenant sync が Git topology に拘束されない。

### Negative

- workspace ごとの repository lifecycle、credential、backup を管理する必要がある。
- Git revision と domain revision の対応を追跡する metadata が必要になる。
- offline merge conflict は content type ごとの policy と UI を別途実装する必要がある。

### Follow-up

- managed repository path と ownership を workspace configuration に定義する。
- `.gitignore` / pre-commit validation で secret、DB、model cache を拒否する。
- GitContentStore / LfsContentStore adapter は Sync port から独立させる。
- repository failure は明示的に報告し、別 backend へ自動 fallback しない。

## References

- [Issue #23](https://github.com/elda27/lineage/issues/23)
- [ADR-0002](./0002-use-just-recipes-for-tag-automation-without-owning-dag-engine.md)
- [ADR-0009](./0009-separate-structured-state-content-and-sync-persistence.md)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
