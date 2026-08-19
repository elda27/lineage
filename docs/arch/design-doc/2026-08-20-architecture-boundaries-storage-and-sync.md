# Architecture Boundaries, Local/Remote Infra, Storage and Sync Design

Status: Draft
Decision date: 2026-08-20
Target release: v0.0.9
Prerequisite: v0.0.8
Tracking issues: #22, #23

> この文書は 2026-08-20 時点の設計議論をまとめた Design Doc である。v0.0.8 への更新後、本書の長期的なアーキテクチャ判断をテーマごとに ADR へ分割し、ADR を正本として v0.0.9 で実装する。本書自体は将来の実装詳細をすべて固定するものではない。

## 1. Goal

Lineage のコードベースを、現在の小規模なローカルアプリ構成から、将来のオンライン版・Enterprise版・複数テナント展開・ローカル限定バイナリまで無理なく拡張できる構造へ再編する。

今回の中心課題は以下である。

- `lineage-core`、`minos`、`fullos`、`agentos` の責務境界を明確化する。
- 技術レイヤーだけでなく「機能接点」を明示し、各アプリで `features` を入口として整理する。
- Shared Kernel を「共通コード置き場」から「複数アプリで共有必須な複雑な概念」へ縮小する。
- Minos / FullOS / AgentOS が必要に応じて独自 domain model を持てるようにする。
- `capture` という入力行為中心の概念を見直し、ユーザ概念として `note` を採用する。
- FullOS frontend の UI 構造とスタイル責務を整理する。
- local / remote infra の trust boundary を明示し、将来的には remote capability を binary level で除外可能にする。
- SQLite に集まりすぎている永続化責務を structured state / content / sync に分離する。
- Git / Git LFS を適切な versioning/content backend として利用しつつ、全体 sync engine にはしない。
- local / server 間の同期を DB file/schema 同期ではなく domain mutation/event を中心に設計する。
- 1つの Source of Truth から複数 tenant へ、データを複製せず publication / automation / fan-out できるモデルを可能にする。

## 2. Non-goals

v0.0.9 で以下をすべて完成させることは必須ではない。

- 完全なオンラインサービスの実装。
- Enterprise policy engine の完成。
- local/server の双方向同期エンジンの本番完成。
- Git LFS server の実装。
- multi-tenant 配信基盤の本番運用。
- 独自 distributed transaction / CRDT engine の導入。

v0.0.9 の主目的は、これらを将来安全に追加できるよう責務境界と module layout を整え、今後の実装が誤った境界へ流れ込むことを防ぐことである。

## 3. Repository / module layout principles

### 3.1 Top-level domain and infra

`domain` と `infra` は feature ごとに囲い込まず、各アプリ / kernel の top-level に置く。

理由は、domain object や infrastructure implementation が複数 feature にまたがるためである。

たとえば `Note`、`Tag`、`Workspace`、認証情報、SQLite repository などを `features/capture/domain` のように feature 配下へ所有させると、automation / search / settings など複数 feature が同じ概念を使う時に ownership が崩れる。

基本形は以下とする。

```text
<app-or-kernel>/
  domain/
  infra/
  features/
```

`features` は domain と injected dependency を束ね、利用者から見た「機能接点」を成立させる。

### 3.2 Features are interaction boundaries, not domain ownership

`features` は domain model の所有境界ではない。

```text
domain
   ↑
features
   ↑
composition root
   ↓
infra
```

feature は必要な port / dependency を injection されて機能を成立させる。

feature が concrete infra を直接生成しない。

たとえば `features/note/save.rs` が `Database::open()` を実行するのではなく、composition root が local / remote policy に応じた storage implementation を組み立てて feature に渡す。

### 3.3 Use case and service

`usecase` と `service` は概念上区別するが、ファイル数が少ない段階で不要な directory を作らない。

- use case: 外部から見た機能の入口。例: `CreateNote`, `RunAutomation`, `VerifyLineage`。
- service: 複数 use case から共有される feature 内部処理。
- 1 use case だけで使う小さな処理: private function のまま保持する。
- business rule: domain へ置く。
- technical implementation: infra へ置く。

初期状態はフラットでよい。

```text
features/
  note/
    mod.rs
    create.rs
    update.rs
```

規模が増えた場合のみ `usecase/`、`service/` 等へ分割する。

### 3.4 Apply the same principle to all applications

同じ原則を `lineage-core`、`minos`、`fullos`、`agentos` に適用する。

アプリ固有の概念・エラー・メッセージ・OS interaction は、それぞれのアプリ側に置く。

Shared Kernel が具体アプリ名や UX を知る構造を避ける。

## 4. FullOS frontend structure

FullOS は frontend を持つため、基本の `domain / infra / features` に加えて UI 向け top-level を持つ。

```text
fullos/src/
  domain/
  infra/
  features/
  components/
    base/
    containers/
  pages/
```

### 4.1 `components/base`

UI 全体の根幹となる再利用コンポーネントを配置する。

例:

- Button
- Input
- Toggle
- Card / Surface
- Typography primitives
- Icon
- Form row
- Dialog primitives

feature や page 固有の意味を持たせない。

### 4.2 `components/containers`

複数の base component を組み合わせるが、特定 feature の domain behavior には依存しない reusable container を置く。

### 4.3 `pages`

実際の画面を表す。

`pages` は従来の `features/*/ui` に相当する画面責務を担い、feature/usecase と components を組み合わせる。

再利用可能な UI を page 配下へ置かない。

### 4.4 Tailwind CSS policy

FullOS frontend の画面・component 固有 styling は Tailwind CSS へ統一する。

原則:

- component / page の layout・装飾: Tailwind utility class。
- 共通 UI 部品: `components` 内で Tailwind を内包する。
- 意味のある共通 design token: Tailwind v4 `@theme`。
- global reset / base style: `index.css` の `@layer base`。
- keyframes 等、CSS で表現する方が自然なもの: `index.css`。
- `.foo { ... }` のような画面固有独自 class は原則撤廃する。

Tailwind の arbitrary value は一律禁止しない。複数箇所で同じ意味を持つ値のみ token へ昇格する。

既存 `shared/ui` は `components/base` / `components/containers` へ責務に応じて移動する。

## 5. Shared Kernel responsibility

### 5.1 Kernel is not a shared utility bucket

`lineage-core` は「Minos / FullOS / AgentOS のどこか2つ以上で使うコード」を置く場所ではない。

Shared Kernel に置く基準は、複数アプリ間で意味・不変条件・identity を共通化しなければシステム全体の整合性が壊れることである。

特に kernel が持つべき候補:

- authentication / identity
- workspace / tenant identity
- authorization / permission model
- lineage / provenance / hash-chain integrity
- cross-application stable identifiers
- canonical asset / record contract
- security-sensitive shared contracts

単純な helper や一時的な共通処理を kernel に寄せない。

### 5.2 Application-local domain models

Minos / FullOS / AgentOS は、それぞれ独自 domain を持てる。

同じ `note` というユーザ概念でも用途は異なる。

#### Minos Note

- minimum input を重視する。
- 高速入力が中心。
- foreground context など入力時観測情報を扱う。
- title 自動導出など Minos UX 固有ルールを持ちうる。

#### FullOS Note

- automation 対象となる情報資産。
- state / lifecycle / tag / automation / editing 等と関係する。
- Minos より豊かな domain behavior を持ちうる。

この2つを同一 Rust struct / TypeScript model に無理に固定しない。

共通 kernel へ保存・連携する際に canonical representation へ map する。

```text
Minos Note ----\
                -> Canonical Asset / Record
FullOS Note ---/
```

### 5.3 `capture` to `note`

`capture` は「取得する行為」であり、domain entity 名として Minos の操作へ寄りすぎている。

ユーザ向け概念は `note` へ変更する。

必要であれば `capture` は feature/usecase 名として残してもよいが、Shared Kernel の domain 名としては使用しない方向とする。

### 5.4 Remove concrete app knowledge from kernel

以下のような concrete app knowledge は kernel から排除する。

- `SOURCE_KIND_MINOS`
- `DEFAULT_WORKSPACE_NAME = "minos"` 相当の共通層定義
- Minos foreground window context
- Minos 固有 title generation policy
- FullOS/Tauri固有 browser message / exception

具体アプリから canonical input を渡す。

## 6. Infrastructure trust boundaries

### 6.1 Threat model

最重要の安全要件は以下である。

> 利用者が local-only だと認識しているデータが、実装追加・設定ミス・dependency injection ミスによって意図せず端末外へ送信されないこと。

Enterprise用途では「設定上 remote を無効化した」だけでは十分ではない。

将来的には local-only binary から remote implementation 自体を除外できることを目指す。

### 6.2 Organize infra by capability and trust boundary

infra は用途を第1軸、trust boundary を第2軸として整理する。

```text
infra/
  persistence/
    local/
      sqlite.rs
    remote/
      server.rs

  inference/
    local/
      local_model.rs
    remote/
      anthropic.rs
      openai.rs

  credentials/
    local/
      keyring.rs

  auth/
  update/
  crypto/
  clock/
```

`local / remote` だけを top-level にして用途を混ぜない。

### 6.3 Definition of local

初期定義:

> local = user data を端末外へ送信しない implementation。

通信方式で分類しない。

たとえば localhost 上の local model server は HTTP を使っていても local とみなしうる。一方 remote filesystem 等は local-looking implementation でも実際の trust boundary を精査する。

### 6.4 Compile-time capabilities

remote implementation は Cargo feature / optional dependency で分離できる設計を目指す。

概念例:

```text
local-db
local-inference
remote-db
remote-inference
cloud-sync
```

`remote` という1つの feature ですべてを開放しない。

local-only build では可能な限り以下を binary から除外する。

- remote inference implementation
- remote persistence implementation
- related HTTP client dependency
- provider endpoint/config
- remote credential implementation

これにより Enterprise 顧客へ「設定ではなく binary 自体に user-data remote path が含まれない」negative guarantee を提供できる余地を持つ。

### 6.5 Runtime policy

オンライン対応 binary でも compile-time capability と runtime authorization/policy は別とする。

例:

```text
remote inference: allowed
remote storage: denied
cloud sync: denied
corporate auth: allowed
```

ただし feature/usecase が毎回 policy を判断しない。

composition root が policy に従い、許可された infra implementation だけを inject する。

### 6.6 Data plane and control plane

将来的には network access を単純な online/offline 1bit で扱わない。

- data plane: note body、artifact、LLM input、DB sync 等。
- control plane: authentication、update manifest、license、policy delivery 等。

Enterpriseでは「data planeは完全local、authだけcorporate IdP」といった構成があり得るため、infra module を目的別に分離しておく。

## 7. Persistence responsibility split

### 7.1 Current concern

現行は SQLite / Cloudflare D1 で同一schemaを利用し、structured metadata と `documents.body_text`、`blob_uri`、automation state 等が同一 schema に集約されている。

これは初期実装としては単純だが、将来以下の責務が衝突する。

- local structured state
- server canonical state
- large/non-structured content
- versioning / diff / merge
- local/server sync
- multi-tenant publication

同じDB製品・同じschemaを local/serverで使うこと自体を設計目標にしない。

守るべきものは domain contract と sync protocol である。

### 7.2 Three persistence responsibilities

永続化を最低限以下へ分離する。

```text
1. Structured State
2. Content
3. Sync
```

#### Structured State

例:

- identity / workspace
- note index / metadata
- tag definitions / assignments
- automation run state
- publication state
- sync cursor / outbox

LocalではSQLiteを引き続き有力候補とする。

ServerではPostgreSQL等のserver-grade DBを選択可能とし、SQLite/D1とのschema一致を要求しない。

#### Content

例:

- note body / large text
- documents
- images
- PDF
- generated artifacts
- binary attachments

`ContentStore` abstraction を設け、storage backendを差し替え可能にする。

候補:

- local filesystem
- Git repository
- Git LFS
- object storage

#### Sync

local DB fileやDB rowを直接replicateするのではなく、domain mutation/eventを同期する。

### 7.3 SQLite role

SQLiteは廃止しない。

将来の主な役割:

- local structured state
- local projection
- index / search support
- offline cache
- pending mutation outbox
- sync cursor

SQLite fileそのものをremote DBと同期する設計にはしない。

## 8. Git and Git LFS boundary

### 8.1 Good use cases

Gitは以下に非常に適している。

- justfile
- automation definition
- prompt / template
- managed script
- user/AI editable text asset
- revision/diff/mergeが価値を持つ text content

既存 ADR-0002 で採用している managed automation の Git revision / diff 利用方針は維持する。

### 8.2 Git LFS role

Git LFS は large/binary content を Git pointer と外部objectへ分離する backend として利用可能。

ただし Git LFS は binary content の semantic merge を解決するものではない。

### 8.3 Do not make Git the global sync engine

Git remoteをLineage全体のtenant sync / distribution engineにしない。

Gitが直接扱わない責務:

- tenant authorization
- partial delivery / retry
- source tenant ownership
- publication revocation
- target-specific automation
- domain-specific conflict resolution
- multi-remote consistency
- audit policy

Git / Git LFS は `ContentStore` / versioning backend の候補とする。

## 9. Local/server synchronization model

### 9.1 Sync domain changes, not databases

local/server間で同期する単位は DB file / table schema ではなく domain mutation/event とする。

概念例:

```text
Mutation
  id
  actor
  workspace_id
  entity_id
  operation
  payload
  base_revision
  created_at
```

ローカル更新時は概念上以下を同一 transaction にする。

```text
1. local projectionを更新
2. sync outboxへmutationを追加
```

オンライン時:

```text
outbox
  ↓
server commit
  ↓
canonical revision
  ↓
remote changes pull
  ↓
local projection update
```

### 9.2 Source of Truth is explicit

各entity / publicationについて Source of Truth を1つにする。

local-first entity が serverへ同期された後、どのauthorityが canonical revision を決めるかをADRで明確化する。

「複数remoteへ書く」ことと「複数Source of Truthを持つ」ことを混同しない。

### 9.3 Conflict policy is domain-specific

Git mergeやDB LWWにすべて委譲しない。

entityごとに必要な conflict policy を持てる設計とする。

例:

- immutable lineage record: append-only / reject conflict
- tag assignment: union可能な場合あり
- note body: manual/3-way merge候補
- task state: revision based / LWW候補
- publication: append-only state transition

v0.0.9 で完全な conflict engine は不要だが、storage layerが将来のpolicyを封じないようにする。

## 10. Multi-tenant publication and fan-out

### 10.1 Requirement

1つのSource of Truthを複数tenantへ利用可能にする。

例:

- A部門がSource Asset Xを所有する。
- XはB部門、C部門の両方で利用可能。
- B/CへXそのものをコピーしない。
- 共通automationまたはtenant固有automationで処理する。
- 処理結果をB/Cへ展開する。

### 10.2 Publication instead of copy

概念モデル:

```text
Canonical Asset X
owner = tenant A
revision = N

      ├─ Publication -> tenant B
      └─ Publication -> tenant C
```

B/Cが保持するのはXのcopyではなく、Xへの許可されたpublication / subscription / grantと、その結果artifactである。

概念例:

```text
Publication
  id
  source_asset_id
  source_revision
  source_tenant_id
  target_tenant_id
  processing_policy
  automation_binding
  state
```

### 10.3 Processing and distribution are separate

2つの形を許容する。

#### Common processing then distribution

```text
Source X
   ↓
Common Automation
   ↓
Canonical Result
   ├→ Tenant B
   └→ Tenant C
```

#### Tenant-specific processing

```text
Source X
   ├→ B Automation → B Artifact
   └→ C Automation → C Artifact
```

Lineage/provenance上、source / processing / output / targetを追跡可能にする。

### 10.4 Client does not directly perform k authoritative writes

clientがB/C/Dへ直接 independent writeする方式は避ける。

```text
B success
C success
D failure
```

のようなdistributed transaction問題をclientへ持ち込むためである。

基本:

```text
Client
  ↓ one authoritative commit
Source of Truth
  ↓
Fan-out / Distribution
  ├→ B
  ├→ C
  └→ D
```

fan-outはtargetごとのdelivery stateを持ち、retry/idempotencyを個別管理できるようにする。

## 11. Migration direction

v0.0.9で一度にstorage backendを全面置換しない。

段階的に進める。

### Phase 1: Boundary refactor

- `app` を `features` へ再編。
- Minos / FullOS / AgentOS に top-level `domain / infra / features` を導入。
- FullOS `components / pages` を整理。
- concrete app dependency を Shared Kernel から排除。
- `capture` domain を `note` / app-local domainへ再整理。

### Phase 2: Infra trust boundary

- infra categoryを用途別に分ける。
- local / remote implementationを明示する。
- remote dependencyをoptional化できるmodule boundaryを作る。
- composition rootをdependency/policy injectionの唯一の入口へ寄せる。

### Phase 3: Persistence contracts

- structured state port と content store port を分離。
- SQLite repositoryの責務をstructured stateへ縮小。
- content identityとphysical locationを分離。
- sync outbox / revision contractを導入可能なinterfaceへ変更。

### Phase 4: Online / multi-tenant foundations

- mutation/event sync protocol。
- server canonical state。
- publication / fan-out model。
- target-specific automation / permissions。

## 12. ADR split after v0.0.8

v0.0.8リリース後、本書を最低限以下のADR候補へ分割する。

1. Repository/module layout and feature boundaries
2. Shared Kernel responsibility and application-local domain models
3. FullOS frontend structure and Tailwind CSS policy
4. Local/remote infrastructure trust boundary and compile-time capabilities
5. Persistence split: structured state / content / sync
6. Git / Git LFS responsibility boundary
7. Local/server synchronization protocol
8. Multi-tenant publication and fan-out model

ADRの粒度は実装調査時に統合・分割してよい。

既存ADRとの重複がある場合は新ADRを追加するのではなく、既存ADRのscopeを確認し、必要ならsupersede / amendmentを行う。

## 13. v0.0.9 issue list

v0.0.9では以下を課題として扱う。

### Architecture boundaries

- [ ] `lineage-core/src/app` を `features` へ再編する。
- [ ] `lineage-core` / `minos` / `fullos` / `agentos` に共通の top-level責務原則を適用する。
- [ ] feature内部はフラット構造を基本とし、不必要な `usecase/` / `service/` directoryを作らない。
- [ ] concrete app 固有定数・メッセージ・contextをShared Kernelから排除する。

### Shared Kernel / Note domain

- [ ] Shared Kernelに残すdomainを認証・identity・workspace・permission・lineage integrity・canonical contract中心に再評価する。
- [ ] Minos独自domainを導入する。
- [ ] FullOS独自domainを導入する。
- [ ] AgentOS独自domainの必要範囲を評価する。
- [ ] `capture` domainの廃止/renameを行い、ユーザ概念を `note` へ整理する。
- [ ] `DocumentAsset::memo()` 等に混在するMinos固有ルールをapp-local domain/serviceへ移す。

### FullOS frontend

- [ ] `components/base` にUI基盤componentを集約する。
- [ ] `components/containers` に再利用containerを集約する。
- [ ] `pages` に実画面責務を集約する。
- [ ] 既存 `shared/ui` を責務に応じてcomponentsへ移行する。
- [ ] raw CSSの画面固有classをTailwindへ移行する。
- [ ] global/base/keyframes/theme tokenのみCSSへ残す。

### Infra security boundary

- [ ] infraを `persistence / inference / credentials / auth / update ...` の用途単位へ整理する。
- [ ] user-dataを端末外へ送るimplementationを `remote` として明示する。
- [ ] remote inference / persistence等のCargo dependencyをoptional化可能にする。
- [ ] local-only binary profileの設計を行う。
- [ ] runtime policyとcompile-time capabilityの境界をADR化する。
- [ ] data plane / control plane通信の分類を導入可能なmodule構造にする。

### Persistence / sync

- [ ] SQLiteの責務をlocal structured state / projectionへ再定義する。
- [ ] `ContentStore` portを設計する。
- [ ] content identity / hash / media type と physical URIを分離する。
- [ ] filesystem / Git / Git LFS / object storeのbackend適用範囲を定義する。
- [ ] local/server同一schema前提を撤廃する。
- [ ] mutation/event + outbox型sync contractを設計する。
- [ ] revision/conflict policyをentity別に定義可能な構造にする。

### Multi-tenant

- [ ] Source of Truthを1つに保つpublication modelを設計する。
- [ ] source assetをtenantごとにコピーしないsharing modelを設計する。
- [ ] common automation / tenant-specific automationの両方を表現可能にする。
- [ ] server-side fan-out / retry / idempotency modelを設計する。
- [ ] publication / artifact / source間のprovenanceをLineageへ統合する。

## 14. Open questions

ADR化時に以下を確定する。

- Shared Kernelのcrate名をそのまま `lineage-core` とするか、kernelの意味を明示するか。
- canonical persisted representationを `Asset` / `Record` / `Document` のどれとして扱うか。
- note bodyをどのサイズ/種類からContentStoreへ外出しするか。
- small text contentをSQLiteへinline保持するoptimizationを許容するか。
- Git repositoryの単位をworkspace / user / tenant / projectのどこへ置くか。
- Git LFS serverをLineageが提供するか、external serviceとして扱うか。
- local-first entityのserver同期後authorityをどの時点で移すか。
- offline simultaneous editのnote merge policy。
- publication revocation後のderived artifact保持/削除policy。
- tenant B/Cがsource contentそのものを閲覧可能か、automation resultだけを受け取るかというpermission model。
- local-only binaryにおけるupdate/auth等control-plane通信の扱い。

## 15. Success criteria

v0.0.9完了時点で最低限以下を満たすことを目標とする。

- 新機能追加時に「どのapp/domain/feature/infraへ置くか」を説明できる。
- Shared Kernelへアプリ固有UXを追加しなくても機能実装できる。
- Minos / FullOSが異なるNote domainを持てる。
- FullOSのUI基盤がcomponents/pages/Tailwindで一貫する。
- remote data pathがsource tree上で明示される。
- 将来的にremote capabilityを含まないbinaryを作るためのCargo/module境界が存在する。
- SQLiteとserver DBが同一schemaでなくても同期可能な設計になっている。
- non-structured contentのbackendをSQLite以外へ移せる。
- Git/Git LFSの利用がsync責務と混同されていない。
- 1つのSource of Truthから複数tenantへpublication/automation結果を展開できるdomain modelへ進化可能である。

## 16. Related documents

- `docs/arch/adr/0002-use-just-recipes-for-tag-automation-without-owning-dag-engine.md`
- `docs/arch/adr/0003-share-capture-and-editing-completion-contract.md`
- `docs/arch/design-doc/2026-08-18-tag-explorer-and-tag-automation.md`
- `docs/concept/MINIMAL_ARCHITECTURE.md`
- `db/schema.sql`
