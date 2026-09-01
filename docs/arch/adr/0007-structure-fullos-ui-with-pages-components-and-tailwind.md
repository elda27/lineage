---
id: ADR-0007
title: FullOS UI を pages / components / features に分け、画面 styling を Tailwind に統一する
status: accepted
date: 2026-09-02
area:
  - application
  - platform
  - quality
scope:
  - fullos
owners:
  - lineage
related:
  - ADR-0005
supersedes: []
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/23
---

# ADR-0007: FullOS UI を pages / components / features に分け、画面 styling を Tailwind に統一する

## Context

FullOS の route-level screen、再利用 UI primitive、feature hook、domain-specific widget が同じ
`features/*/ui` や `shared/ui` に混在すると、画面遷移、再利用範囲、business behavior の所有者を
判別しにくい。また raw CSS class と Tailwind utility が併存すると、theme token を経由しない色や
余白が増え、dark theme を含む全画面の一貫性を保ちにくい。

UI を一つの巨大 component library に集約することも、feature 固有 UI の意味を失わせるため避ける
必要がある。

## Decision

FullOS frontend は次の責務を top-level に持つ。

```text
src/
  app/                  # routing と composition
  pages/                # route-level screen
  components/
    base/               # 意味を持たない UI primitive
    containers/         # 複数画面で再利用する composite
  features/             # feature hook/use case と feature-local UI
  domain/
  infra/
```

- `pages` は Home、Search、Settings、Automation、Tag Explorer など route または navigation item
  に対応する screen component を所有する。
- `components/base` は Button、Input、Toggle、Surface、Dialog、Typography など domain-neutral な
  primitive を所有する。
- `components/containers` は複数ページで再利用される layout/composite を所有するが、特定 feature
  の business rule を持たない。
- feature に固有で再利用範囲が feature 内に閉じる component は
  `features/<feature>/components` に置ける。route-level component は置かない。
- `app` は route selection、global provider、dependency composition に限定する。

画面・component 固有の layout と decoration は Tailwind CSS utility に統一する。`index.css` には
次だけを残す。

- Tailwind v4 の import と semantic theme token (`@theme`)
- reset / document-wide base rule (`@layer base`)
- utility では不自然な keyframes と global accessibility rule

`.settings-card` のような page-specific selector は追加しない。繰り返し利用され、意味が安定した
値は semantic token または component へ昇格する。arbitrary value は一律禁止しないが、同じ意味の
値を複数箇所へ複製しない。

theme は `light` / `dark` / `system` の選択を app shell で解決し、root element の class または
data attribute に一度だけ反映する。component は `white` や `black` のような mode 固有色ではなく、
background、surface、foreground、muted、border、accent など semantic token を参照する。

## Options considered

### Option 1: 現在の feature ごとの `ui` directory を route と component の両方に使う

採用しない。route ownership と再利用範囲が曖昧なまま残る。

### Option 2: CSS Modules または component ごとの raw CSS に統一する

採用しない。既に Tailwind を利用しており、semantic token と utility を中心にした方が変更範囲を
追いやすい。

### Option 3: pages / reusable components / feature-local UI を分け、Tailwind に統一する

採用する。画面責務と再利用責務を分けながら、feature cohesion を維持できる。

## Consequences

### Positive

- route-level screen の一覧と所有者が明確になる。
- primitive と feature-specific UI を誤って共有化しにくくなる。
- semantic token を通じて light/dark theme を一貫して適用できる。
- page-specific CSS の増殖を防げる。

### Negative

- 既存 import path と component 配置を広範囲に変更する必要がある。
- Tailwind class が長くなる component では primitive 抽出の判断が必要になる。
- theme token の命名を継続して管理する必要がある。

### Follow-up

- route-level component を `src/pages` へ移す。
- `shared/ui/kit.tsx` を `components/base` と `components/containers` へ分解する。
- page-specific raw CSS を Tailwind utility へ移行する。
- dark theme の regression test で root theme、system theme、再起動後の復元を確認する。

## References

- [Issue #23](https://github.com/elda27/lineage/issues/23)
- [Issue #27](https://github.com/elda27/lineage/issues/27)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
