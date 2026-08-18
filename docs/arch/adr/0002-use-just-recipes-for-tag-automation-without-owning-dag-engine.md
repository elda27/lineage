---
id: ADR-0002
title: タグ自動化の定義に just recipe を用い、Lineage は DAG engine を持たない
status: accepted
date: 2026-08-18
area:
  - application
  - integration
  - quality
scope:
  - application
owners:
  - elda27
related:
  - ADR-0001
supersedes: []
supersededBy: null
discussion: null
---

# ADR-0002: タグ自動化の定義に just recipe を用い、Lineage は DAG engine を持たない

## Context

タグに紐づく自動化を、組み込み処理だけでなくユーザ定義処理にも拡張したい。

要件は以下である。

- 単純な変換は GUI から利用できること。
- 上級ユーザは任意の複雑な処理や依存関係を記述できること。
- AI が自動化定義を生成・編集しやすいこと。
- Windows / Linux など複数 OS 上で標準的な shell 環境を利用できること。
- 独自 DAG engine / DSL / parser / runtime の実装負荷を避けること。
- 入力や処理定義が変わらない限り不要な再実行を避けること。
- 将来 DAG engine を導入する場合にも、今回の冪等性設計を再利用できること。

また自動化処理から外部 script / executable / project を利用できるが、その外部処理が Lineage 管理下にある場合と、ユーザが完全に外部管理している場合では変更検知責務が異なる。

## Decision

### 1. `just` recipe をユーザ定義 automation の正本とする

Lineage は独自の DAG engine や Pipeline IR を持たない。

タグに紐づくユーザ定義 automation は `just` recipe として記述する。recipe dependency による DAG 構成、shell / PowerShell / CLI / Python 等の呼び出しは `just` と実行環境へ委譲する。

Lineage は recipe 内部の各 step を domain object として所有しない。

Lineage が所有するのは以下である。

- tag と recipe の binding
- source record
- run
- artifact
- provenance
- processing fingerprint
- input fingerprint
- execution key
- execution status

GUI の簡易変換も別 execution engine を作らず、Lineage の built-in command を呼ぶ recipe へ収束させる。

### 2. recipe 名を永続 ID とみなさない

`build-tag-<tag-name>` を人間に分かりやすい entry point convention として利用できるが、タグ名変更によって binding が壊れないよう、永続的な関連付けは stable tag id と recipe binding で管理する。

### 3. 冪等性は処理 fingerprint と入力 fingerprint の組で判定する

成功済み run の再利用判定は、概念上以下の execution key で行う。

```text
execution_key = hash(
  tag_id
  + processing_fingerprint
  + input_fingerprint
)
```

同一 execution key の成功済み run が存在する場合は原則として再実行しない。

Force rebuild は既存 cache を削除せず、cache 判定を明示的に無視して新しい run を作る。強制実行だったことを run に記録する。

### 4. Git は Lineage 管理下の処理変更検知に用いる

Lineage 管理下の justfile、automation definition、script 等は Git 管理する。

Git revision / diff を変更候補の検出に用い、必要に応じて recipe dependency を再解釈して、影響する recipe の processing fingerprint を更新する。

raw Git diff 自体を冪等性の正本とはせず、最終的には Lineage 管理下の処理定義から processing fingerprint を得る。

Lineage 管理下の外部 script は同じ processing revision の対象に含める。

### 5. Lineage 管理外の外部処理変更は自動検知しない

ユーザが完全に外部管理する script / executable / project の内容変更は Lineage の責務外とする。

Lineage はその変更を自動 invalidate しない。外部処理を変更したユーザは Rebuild / Force rebuild を明示的に実行する。

これにより Lineage が外部ファイル監視、任意言語の dependency resolution、環境差分解析まで責務を拡大することを避ける。

### 6. `modified_at` は候補抽出に利用し、最終判定は input fingerprint で行う

record の `updated_at` / `modified_at` は incremental rebuild の変更候補抽出に利用する。

ただし timestamp のみを冪等性判定の正本にはしない。

実際に automation が参照する入力値から input fingerprint を生成し、処理対象が実質的に変化したかを判定する。

### 7. fingerprint model は将来の DAG 実装でも再利用可能にする

将来 Lineage 自身が DAG engine を持つ必要が生じた場合も、同じ原則を node 単位へ拡張する。

```text
node_execution_key = hash(
  node_definition_fingerprint
  + node_input_fingerprint
)
```

したがって今回の冪等性機構は、将来の incremental DAG execution の cache 基盤として再利用する。

## Options considered

### Option 1: Lineage が独自 DAG engine / Pipeline IR を持つ

- 利点: typed input/output、retry、permission、node cache、UI 可視化などを Lineage が完全に制御できる。
- 欠点: DAG parser、runtime、dependency resolution、validation、UI editor 等の大きな実装・保守負荷が発生する。
- 欠点: ユーザや AI が既存ツールの知識を使えず、Lineage 固有 DSL を学ぶ必要がある。
- 採用しなかった理由: 現時点の目的に対して実装負荷が過大であり、`just` で必要な複雑性を十分に表現できるため。

### Option 2: `just` は executor のみにし、Lineage が DAG 定義を持つ

- 利点: execution は既存ツールへ委譲しつつ、Lineage が構造を理解できる。
- 欠点: DAG 定義と just recipe が二重管理になり、同期・変換ロジックが必要になる。
- 採用しなかった理由: 実装負荷低減という主要目的を損ない、ユーザが直接 just を記述する利点も弱くなるため。

### Option 3: 外部 script の変更も Lineage が自動追跡する

- 利点: ユーザが Rebuild を意識せず済む。
- 欠点: 任意 project の依存関係、環境、生成物、package manager まで追跡範囲が広がる。
- 採用しなかった理由: 管理境界が不明瞭になり、Lineage の責務が過度に拡大するため。

### Option 4: `modified_at` のみで再実行判定する

- 利点: 実装が単純で高速。
- 欠点: 値が元に戻った場合、無関係な列だけ変わった場合、migration 等で timestamp だけ更新された場合にも再実行が発生する。
- 採用しなかった理由: incremental execution の正確性が低いため。

## Consequences

### Positive

- Lineage 独自 DAG engine の実装を避けられる。
- ユーザは just の dependency 機能を使って複雑な DAG を構成できる。
- AI は既存の just / shell / PowerShell / Python 等の知識を使って自動化を生成できる。
- GUI の簡易変換と高度なユーザ定義処理を同じ execution model へ統一できる。
- fingerprint ベースの cache は将来 DAG engine を導入しても再利用できる。
- Lineage 管理範囲と外部管理範囲の責務境界が明確になる。

### Negative

- Lineage は recipe 内部の node / step を完全には理解しない。
- 外部管理処理の変更は自動では検知されない。
- processing fingerprint の recipe 単位影響判定には Git 情報と just dependency の解釈が必要になる。
- arbitrary shell command を許容するため、permission / security model は別途設計が必要である。

### Follow-up

- Tag Explorer から recipe binding、test run、Rebuild、Force rebuild を操作できるようにする。
- automation run に processing/input/output fingerprint と forced execution の情報を保持できるようにする。
- just recipe 実行時の permission / secret / sandbox 境界を別途決定する。
- AI による just recipe 生成・修正時の review / approval UX を設計する。

## References

- `docs/arch/design-doc/2026-08-18-tag-explorer-and-tag-automation.md`
- `justfile`
- `db/schema.sql`
- ADR-0001
