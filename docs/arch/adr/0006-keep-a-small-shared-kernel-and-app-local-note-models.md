---
id: ADR-0006
title: lineage-core を小さな Shared Kernel とし、Note はアプリ固有モデルで表現する
status: accepted
date: 2026-09-02
area:
  - domain
  - application
  - data
scope:
  - application
owners:
  - lineage
related:
  - ADR-0003
  - ADR-0004
  - ADR-0005
supersedes: []
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/23
---

# ADR-0006: lineage-core を小さな Shared Kernel とし、Note はアプリ固有モデルで表現する

## Context

Minos の高速入力、FullOS の編集・整理、Runner の自動化では、同じ「ノート」を扱っていても必要な
状態と振る舞いが異なる。単一の shared struct に全アプリの要件を追加すると、foreground window、
title derivation、UI state、automation state などのアプリ固有知識が共有層へ集まり、変更理由が
異なるものを同時に変更することになる。

一方、永続化 ID、workspace identity、Lineage hash chain、canonical record contract まで各アプリで
独自実装すると、同じ記録の同一性と真正性がアプリごとに分岐する。

## Decision

`lineage-core` という crate 名は v0.0.9 でも維持するが、その役割を Shared Kernel に限定する。
Shared Kernel に置くのは、複数アプリで同じ意味・identity・不変条件を共有しなければ整合性が壊れる
ものだけとする。

主な所有対象は次のとおりである。

- workspace / actor / cross-application stable identifier
- canonical persisted record contract
- Lineage / provenance / hash-chain integrity
- permission や security-sensitive な共有 contract
- 複数 executable が同じ transaction rule で更新すべき mutation contract

利用者向けの概念名は `Note` とする。Minos と FullOS はそれぞれ app-local な Note model を持ち、
必要な入力、状態、validation、UX rule を独立して定義する。Runner も自動化入力・結果のために
必要な app-local model を持てる。

永続化境界では、中立な canonical representation として `DocumentAsset` を維持する。ただし
`DocumentAsset` は ID、workspace、content reference、document kind、timestamp など共有が必要な
record contract と不変条件だけを持つ。次の知識は Shared Kernel から除外する。

- Minos の foreground process / window context
- Minos の title 自動導出と空タイトル表示
- `SOURCE_KIND_MINOS` や Minos 固有 default workspace
- FullOS / Tauri / browser 固有 message と exception
- 特定画面だけで使う editing state

各アプリは app-local Note を canonical `DocumentAsset` へ明示的に map する。title derivation や
context metadata の生成は Minos feature/domain service が行い、完成した canonical input を
Shared Kernel の保存 use case へ渡す。

`capture` は入力行為を表す feature/use case 名として Minos 内に残せるが、Shared Kernel の entity
または domain module 名には使用しない。

## Options considered

### Option 1: 全アプリで単一の Note / Document struct を共有する

採用しない。共有必須でない UX と lifecycle が一つの model に集まり、optional field と条件分岐が
増えるためである。

### Option 2: 各アプリが永続化 record と Lineage を独自に持つ

採用しない。stable ID、hash-chain、mutation rule が分岐し、同一記録の整合性を保証できない。

### Option 3: app-local Note と小さな canonical record を明示的に map する

採用する。アプリ固有の進化と cross-application integrity を両立できる。

## Consequences

### Positive

- Minos と FullOS が異なる Note behavior を持てる。
- Shared Kernel から concrete app knowledge を除去できる。
- canonical record と Lineage の同一性は一箇所で維持できる。
- 将来別の UI や import tool を追加しても app-local model から同じ contract へ map できる。

### Negative

- app-local model と canonical record の mapping code が必要になる。
- 同じ語の TypeScript / Rust model が複数存在するため、命名と boundary の説明が必要になる。
- crate 名 `lineage-core` だけでは Shared Kernel の限定責務が完全には伝わらない。

### Follow-up

- `lineage-core/src/domain/capture.rs` を neutral record module と app-local Minos model に分割する。
- `DocumentAsset::memo()`、`CaptureContext`、title derivation を Minos 側へ移す。
- FullOS の `Memo` model を利用者向け `Note` 用語へ段階的に統一する。
- Shared Kernel の public API を見直し、app-specific constant と message を削除する。

## References

- [Issue #23](https://github.com/elda27/lineage/issues/23)
- [ADR-0003](./0003-share-capture-and-editing-completion-contract.md)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
