# Architecture Decision Records (ADR)

## 1. 目的

ADR は、アーキテクチャ上の重要な意思決定について、背景、選択肢、採用理由、結果を記録する。
現在の仕様そのものは [`/docs/arch/README.md`](../README.md) に記載し、ADR は「なぜその構造を
選んだか」を長期間参照できる履歴とする。

対象は infrastructure に限定しない。次のような、広範囲へ影響し、後から変更するコストが高い
判断を ADR として残す。

- component / bounded context / aggregate の境界
- API、event、外部 system との interface
- data ownership、consistency、persistence、sync
- dependency direction と layer structure
- security、availability、performance、audit 等の quality attribute
- build、delivery、operation 上の長期的な制約

単一 method 内の実装、容易に変更できる命名、一時的な workaround、個別 API field などは原則として
Issue、PR、設計文書で管理する。

## 2. 新規 ADR の作成方法

1. 本 README と既存 ADR を検索し、同じ decision が存在しないことを確認する。
2. 既存の最大番号に 1 を加えた 4 桁の通し番号を使用する。欠番は許容し、番号を再利用しない。
3. [`template.md`](./template.md) をコピーし、
   `<4桁番号>-<decisionを表すkebab-case>.md` として本 directory 直下へ置く。
4. frontmatter と全 section を記載する。該当しない section は理由を明記する。
5. 本 README の「全 ADR 一覧」「現在有効な ADR」「superseded 関係」を更新する。
6. 現在の architecture が変わる場合は [`../README.md`](../README.md) と関連設計文書も更新する。

ADR は本 directory 直下に flat に配置する。classification は directory ではなく `area`、`scope`、
`status` で表す。

accepted ADR の Context / Decision / Options considered / Consequences を、後から意味が変わる形で
書き換えない。decision を変更する場合は新しい ADR を作り、旧 ADR を `superseded` にする。

ADR は議論全文や実装状況を保存する場所ではない。議論元は `discussion` と References から参照し、
他文書や code の「現在実装済み／未実装」といった陳腐化する状態は living document で管理する。

## 3. metadata

### status

| 値 | 意味 |
| --- | --- |
| `proposed` | 提案中で、まだ決定されていない |
| `accepted` | 採用され、現在有効 |
| `rejected` | 検討されたが採用されなかった |
| `deprecated` | 現在は推奨されないが、完全には置き換えられていない |
| `superseded` | 別の ADR によって置き換えられた |

これ以外の status を追加しない。

### area

| 値 | 意味 |
| --- | --- |
| `domain` | domain model、business rule、bounded context、aggregate boundary |
| `application` | application structure、use case、component split |
| `integration` | API、event、external system interface |
| `data` | data model、ownership、consistency、persistence、sync |
| `platform` | runtime、framework、language、infrastructure |
| `security` | authentication、authorization、audit、data protection |
| `quality` | performance、availability、maintainability 等 |
| `delivery` | build、release、distribution、development process |
| `operations` | operation、monitoring、backup、incident response |

### scope

decision が適用される範囲を記載する。application 全体なら `application`、特定 component に限定する
場合は `fullos` 等の名前を使用する。

## 4. 全 ADR 一覧

本 index は手動で管理する。ADR の追加・状態変更時に必ず更新する。

| ID | Title | Status | Area | Scope | Date | Replaces |
| --- | --- | --- | --- | --- | --- | --- |
| [ADR-0001](./0001-store-builtin-tag-state-outside-document-meta.md) | 組み込みタグの状態を document_states に分離し、削除は論理削除とする | superseded | domain, data | application | 2026-08-14 | - |
| [ADR-0002](./0002-use-just-recipes-for-tag-automation-without-owning-dag-engine.md) | タグ自動化の定義に just recipe を用い、Lineage は DAG engine を持たない | accepted | application, integration, quality | application | 2026-08-18 | - |
| [ADR-0003](./0003-share-capture-and-editing-completion-contract.md) | minos の入力と fullos の編集でメタ情報補完契約を共通化する | accepted | application, quality | application | 2026-08-18 | - |
| [ADR-0004](./0004-rust-owned-fullos-delta-mutations.md) | FullOS の書き込みを Rust 所有の差分 mutation API に集約する | accepted | application, integration, data, platform | application | 2026-08-25 | ADR-0001 |
| [ADR-0005](./0005-place-domain-features-infra-behind-composition-roots.md) | アプリごとに domain / features / infra を分け、composition root で接続する | accepted | application, platform, quality | application | 2026-09-02 | - |
| [ADR-0006](./0006-keep-a-small-shared-kernel-and-app-local-note-models.md) | lineage-core を小さな Shared Kernel とし、Note はアプリ固有モデルで表現する | accepted | domain, application, data | application | 2026-09-02 | - |
| [ADR-0007](./0007-structure-fullos-ui-with-pages-components-and-tailwind.md) | FullOS UI を pages / components / features に分け、画面 styling を Tailwind に統一する | accepted | application, platform, quality | fullos | 2026-09-02 | - |
| [ADR-0008](./0008-separate-infrastructure-by-capability-and-trust-boundary.md) | infrastructure を用途と trust boundary で分け、remote capability を build 時に除外可能にする | accepted | platform, security, application | application | 2026-09-02 | - |
| [ADR-0009](./0009-separate-structured-state-content-and-sync-persistence.md) | 永続化を structured state / content / sync の契約へ分離する | accepted | data, application, integration | application | 2026-09-02 | - |
| [ADR-0010](./0010-use-git-and-lfs-only-as-versioned-content-backends.md) | Git / Git LFS を workspace 単位の versioned content backend に限定する | accepted | data, integration, operations | application | 2026-09-02 | - |
| [ADR-0011](./0011-sync-versioned-domain-mutations-not-databases.md) | local / server 同期は DB ではなく versioned domain mutation を交換する | accepted | data, integration, domain | application | 2026-09-02 | - |
| [ADR-0012](./0012-publish-canonical-assets-with-server-side-fan-out.md) | canonical asset を複製せず publication と server-side fan-out で複数 tenant へ提供する | accepted | domain, data, integration, security | application | 2026-09-02 | - |

## 5. 現在有効な ADR（status: accepted）

- [ADR-0002](./0002-use-just-recipes-for-tag-automation-without-owning-dag-engine.md): タグ自動化の定義に just recipe を用い、Lineage は DAG engine を持たない
- [ADR-0003](./0003-share-capture-and-editing-completion-contract.md): minos の入力と fullos の編集でメタ情報補完契約を共通化する
- [ADR-0004](./0004-rust-owned-fullos-delta-mutations.md): FullOS の書き込みを Rust 所有の差分 mutation API に集約する
- [ADR-0005](./0005-place-domain-features-infra-behind-composition-roots.md): アプリごとに domain / features / infra を分け、composition root で接続する
- [ADR-0006](./0006-keep-a-small-shared-kernel-and-app-local-note-models.md): lineage-core を小さな Shared Kernel とし、Note はアプリ固有モデルで表現する
- [ADR-0007](./0007-structure-fullos-ui-with-pages-components-and-tailwind.md): FullOS UI を pages / components / features に分け、画面 styling を Tailwind に統一する
- [ADR-0008](./0008-separate-infrastructure-by-capability-and-trust-boundary.md): infrastructure を用途と trust boundary で分け、remote capability を build 時に除外可能にする
- [ADR-0009](./0009-separate-structured-state-content-and-sync-persistence.md): 永続化を structured state / content / sync の契約へ分離する
- [ADR-0010](./0010-use-git-and-lfs-only-as-versioned-content-backends.md): Git / Git LFS を workspace 単位の versioned content backend に限定する
- [ADR-0011](./0011-sync-versioned-domain-mutations-not-databases.md): local / server 同期は DB ではなく versioned domain mutation を交換する
- [ADR-0012](./0012-publish-canonical-assets-with-server-side-fan-out.md): canonical asset を複製せず publication と server-side fan-out で複数 tenant へ提供する

## 6. superseded 関係

ADR-0001 は ADR-0004 によって、`document_states` の責務ではなく FullOS WebView の書き込み境界に
ついて置き換えられた。

| 旧 ADR | 置き換え先（supersededBy） |
| --- | --- |
| [ADR-0001](./0001-store-builtin-tag-state-outside-document-meta.md) | [ADR-0004](./0004-rust-owned-fullos-delta-mutations.md) |

## 7. ADR 作成前の確認事項

最低限、次を検索する。

- [ ] 同じ `scope`
- [ ] 同じ `area`
- [ ] 類似する title
- [ ] 同じ technology、domain term、interface
- [ ] 既存 ADR の `related`
- [ ] 既存 ADR の `supersedes`
- [ ] 既存 ADR の `supersededBy`

同じ decision が既に存在する場合は新規 ADR を作らない。既存 decision を変更する場合は新しい ADR
で置き換え、旧 ADR を削除しない。
