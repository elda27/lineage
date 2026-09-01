---
id: ADR-0005
title: アプリごとに domain / features / infra を分け、composition root で接続する
status: accepted
date: 2026-09-02
area:
  - application
  - platform
  - quality
scope:
  - application
owners:
  - lineage
related:
  - ADR-0002
  - ADR-0003
  - ADR-0004
supersedes: []
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/23
---

# ADR-0005: アプリごとに domain / features / infra を分け、composition root で接続する

## Context

Lineage は Minos、FullOS、Runner と共有ライブラリを持ち、それぞれが異なる操作、UI、OS integration を担う。
ディレクトリ名だけで application layer、画面、起動処理、共有 helper を区別しない場合、use case が
concrete infrastructure を生成したり、アプリ固有処理が共有層へ流入したりする。機能追加時に配置先と
依存方向を説明できる構造が必要である。

一方、小規模な機能まで機械的に `usecase/`、`service/`、`repository/` へ分割すると、責務よりも
階層を追うコストが大きくなる。境界は固定しつつ、境界内部は必要になるまで平坦に保つ必要がある。

## Decision

各アプリケーションと共有 kernel は、コードの性質に応じて次の top-level responsibility を持つ。

```text
domain/
features/
infra/
```

- `domain` は identity、不変条件、状態遷移、value object、port を所有し、UI・OS・DB・HTTP に依存しない。
- `features` は利用者または外部 adapter から見た操作単位を所有し、domain と注入された port を用いて
  use case を成立させる。feature は domain object の所有者ではない。
- `infra` は domain/application port の concrete implementation を所有する。DB、filesystem、keyring、
  HTTP、OS API、clock などの technical detail をここへ置く。
- composition root は binary entry point、Tauri setup、React application shell など起動境界に置き、
  policy と build capability に基づいて concrete implementation を生成し feature へ注入する。

依存方向は次のとおりとする。

```text
domain <- features <- composition root -> infra
```

`features` が concrete infra を直接生成すること、`domain` が `features` または `infra` を import
することを禁止する。`infra` は port を実装するために domain contract へ依存してよい。

既存の `app` という名称は composition root / application shell に限って使用できる。use case 集合を
表す layer 名としては `features` を使用する。共有 Rust crate の `src/app` や FullOS core の `app`
は `features` へ移行する。

feature 内部は原則として平坦に保つ。複数の公開 use case や共有処理が増え、名前だけでは責務を
判別できなくなった場合にのみ `usecase/`、`service/` 等へ分割する。空の抽象 layer や一実装しかない
委譲 wrapper は作らない。

アプリ間で直接実装を import しない。複数アプリで意味と不変条件を共有する必要がある contract のみ、
Shared Kernel を通して連携する。

## Options considered

### Option 1: 技術 layer を repository 全体で一組だけ持つ

採用しない。アプリ固有 domain と composition policy が混ざり、Shared Kernel が共通コード置き場に
戻るためである。

### Option 2: feature ごとに domain と infra を完全に囲い込む

採用しない。Note、Tag、Workspace、credential、persistence のように複数 feature が利用する概念の
ownership が分裂するためである。

### Option 3: アプリごとの top-level boundary と composition root を置く

採用する。依存方向を固定しながら、小規模な feature は平坦に保てる。

## Consequences

### Positive

- 新規コードの配置先と依存方向を説明できる。
- feature test で port の test double を注入できる。
- local / remote implementation の選択を composition root に集約できる。
- アプリ固有 domain を共有 kernel から独立して進化させられる。

### Negative

- 既存 import path と module declaration を段階的に変更する必要がある。
- React shell、Tauri command、Rust binary で composition root の形が異なるため、完全に同一の
  directory tree にはならない。
- boundary 違反を review と test で継続的に検出する必要がある。

### Follow-up

- `lineage-core/src/app` と `fullos/core/app` を `features` へ移行する。
- Minos、FullOS、Runner に不足している app-local `domain` / `features` / `infra` を導入する。
- feature が DB や provider を直接生成している箇所を composition root へ移す。
- CI で代表的な local-only / standard build を compile し、依存境界の退行を検出する。

## References

- [Issue #23](https://github.com/elda27/lineage/issues/23)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
- [ADR-0004](./0004-rust-owned-fullos-delta-mutations.md)
