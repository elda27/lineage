---
id: ADR-0008
title: infrastructure を用途と trust boundary で分け、remote capability を build 時に除外可能にする
status: accepted
date: 2026-09-02
area:
  - platform
  - security
  - application
scope:
  - application
owners:
  - lineage
related:
  - ADR-0005
supersedes: []
supersededBy: null
discussion: https://github.com/elda27/lineage/issues/23
---

# ADR-0008: infrastructure を用途と trust boundary で分け、remote capability を build 時に除外可能にする

## Context

「設定で remote access を無効にした」だけでは、誤った dependency injection、将来の機能追加、
既定値の変更によって user data が端末外へ送信される経路を完全には否定できない。local-only を
要求する利用者には、runtime flag より強い保証が必要である。

一方、network access をすべて禁止すると、data plane を local に保ちながら update、license、
corporate authentication だけを利用する構成を表現できない。通信手段ではなく、用途と送信する
データの trust boundary を基準に分類する必要がある。

## Decision

infrastructure は用途を第一軸、trust boundary を第二軸として整理する。

```text
infra/
  persistence/
    local/
    remote/
  inference/
    local/
    remote/
  credentials/
    local/
  auth/
  update/
  crypto/
  clock/
```

`local` は「user data を端末外へ送信しない implementation」と定義する。localhost の runtime は
protocol が HTTP でもこの条件を満たせば local とみなせる。remote filesystem や proxy のように
端末境界を越えるものは、見かけ上の API に関係なく remote とする。

compile-time capability は用途ごとに独立して定義する。少なくとも次を別々に扱える構造にする。

```text
local-db
local-inference
remote-inference
remote-persistence
cloud-sync
control-plane-update
control-plane-auth
```

共有 crate は remote capability と関連 dependency を無条件の default feature にしない。
各 executable / distribution profile が必要な capability を明示的に有効化する。standard consumer
build は remote inference を含められるが、local-only build は remote inference、remote persistence、
cloud sync、その provider endpoint と HTTP dependency を可能な限り binary から除外する。

compile-time capability と runtime authorization/policy は別の判定とする。capability が compile
されていても policy が許可しない implementation は composition root が生成・注入しない。feature
自身は provider の選択や policy 判定を行わない。

network operation を data plane と control plane に分類する。

- data plane: note/content、artifact、LLM prompt、structured state、sync mutation
- control plane: update manifest、authentication、license、policy delivery

capability 名と policy はこの分類を失わない形で定義する。local-only という名前だけで control plane
まで暗黙に許可または禁止しない。

local provider 失敗時に remote providerへ、またはその逆へ自動 fallback しない。利用者または明示的な
policy が選択した経路が失敗した場合は、その経路の error を返す。

## Options considered

### Option 1: 一つの `online` runtime flag だけを持つ

採用しない。binary に remote path が残り、data/control plane と provider ごとの権限を表現できない。

### Option 2: `infra/local` と `infra/remote` を top-level にする

採用しない。persistence、inference、credentials 等の異なる責務が同じ directory に混在する。

### Option 3: 用途別 module、compile-time capability、runtime policy を組み合わせる

採用する。negative guarantee と柔軟な distribution profile を両立できる。

## Consequences

### Positive

- local-only build から user-data remote path を除外できる。
- remote inference と remote storage を独立して許可できる。
- provider 選択と policy を composition root に集約できる。
- local LLM と cloud LLM を同じ上位 port の明示的な実装として追加できる。

### Negative

- Cargo feature combination と distribution profile の test matrix が増える。
- optional dependency と `cfg` boundary の保守が必要になる。
- control plane が user data を含まないことを個別に review する必要がある。

### Follow-up

- Anthropic 等の remote inference dependency を `remote-inference` capability 配下へ移す。
- local LLM runtime を `local-inference` capability として追加する。
- CI で local-only build に remote provider symbol/dependency が含まれないことを検証する。
- executable ごとの standard/local-only feature set を release document に記載する。

## References

- [Issue #23](https://github.com/elda27/lineage/issues/23)
- [Issue #26](https://github.com/elda27/lineage/issues/26)
- [Architecture boundaries, storage and sync design](../design-doc/2026-08-20-architecture-boundaries-storage-and-sync.md)
