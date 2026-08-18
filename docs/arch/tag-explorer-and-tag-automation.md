# Tag Explorer / Tag Automation Design

Status: Draft
Date: 2026-08-18

## 1. Goal

タグ機能を再編し、将来の自動化機能とアプリケーション内部の特殊実装を、同じ「タグに紐づく振る舞い」のモデルへ統合する。

従来の組み込みタグは、タグに該当する記録を特定のビューとして表示し、必要に応じてアプリケーション機能を有効化するためのものだった。たとえば `#task` はタスクリストとして表示し、完了操作を提供する。

しかし、組み込みタグ以外のタグでも同じビューや変換を使いたい場合、組み込みタグの特殊処理とは別に機能を実装する必要がある。結果として「タグが意味を持つ仕組み」と「自動化ルール」が別系統になっている。

本設計では次を目標とする。

- 組み込みタグとユーザタグの処理モデルを可能な範囲で共通化する。
- タグに対して表示方法、変換、自動化パイプラインを関連付けられるようにする。
- noisy なコンテキスト情報を通常のタグ空間から分離する。
- 自動化パイプラインの結果を構造化データとして保持し、タグごとに適したビューを提供できるようにする。
- minos は入力と収集に集中し、fullos はタグの意味を解釈して表示・設定する方向を維持する。
- Lineage の append-only / provenance の考え方を壊さない。

## 2. Current state

現行では主に次の仕組みが存在する。

1. `document_meta` / `meta_tags`
   - ユーザが入力したメタ情報を保存する。
   - `meta_tags` は入力補完の学習対象でもある。
2. `BuiltinTag.ts`
   - `task` / `memo` などの組み込みタグを定義する。
   - 組み込みタグから `complete` / `archive` / `trash` の capability を直接決定する。
3. `document_states`
   - 完了・アーカイブ・削除など、ユーザのタグ入力ではない UI 操作結果を保持する。
4. `automation_rules` / `automation_runs`
   - `manual` / `meta_match` / `schedule` をトリガとして生成AI処理を実行する。

このため現在は、タグから UI capability を決める経路と、タグを条件として automation を実行する経路が別モデルになっている。

## 3. Terminology

### 3.1 Tag

ユーザが記録を分類・検索・操作するために明示的に利用するラベル。

タグには2種類ある。

#### Built-in tag

アプリケーション側で pre-defined されるタグ。

- アプリケーション機能を実現するために特殊な意味を持つことがある。
- minos 上で特殊な挙動を持つ場合がある。
- fullos では可能な限りユーザタグと共通の処理経路を通す。
- ただし OS 連携や UI capability など、完全共通化が不自然な処理については built-in implementation を許容する。

#### User tag

ユーザが作成するタグ。

- タグエクスプローラーから表示方法や自動化パイプラインを設定できる。
- 組み込みタグと同じ表示・変換パイプラインを利用できることを目標とする。

### 3.2 Metadata

ユーザの操作コンテキストから自動収集される情報。

例:

- foreground application
- window title
- URL
- 将来追加される OS / application context

metadata は通常のタグとは異なり、デフォルトではタグ一覧、タグ補完、通常検索の対象にしない。

目的は、普段の利用では noisy な情報をユーザのタグ空間へ混ぜないことにある。

metadata の取得項目は設定で有効・無効を切り替えられるものとする。

### 3.3 Tag assignment source

タグの「種類」とタグが記録に「どう付いたか」は別概念として扱う。

タグ自体は built-in / user のどちらかだが、assignment には最低限以下の source を持てるようにする。

- `user`: ユーザが明示的に入力
- `derived`: metadata や自動化から生成
- `system`: built-in behavior により付与

これにより、自動生成された `#VS Code` のようなタグも通常のユーザタグとして表示しつつ、由来を追跡できる。

## 4. Metadata and `#app`

従来、foreground application や window title は自動タグとして扱われるため、通常利用時の検索やタグ候補を noisy にする問題がある。

今後はこれらを metadata として保存する。

### 4.1 Default behavior

- foreground application は metadata として保存する。
- window title も設定が有効なら metadata として保存する。
- metadata は通常のタグ検索・タグ補完には含めない。
- advanced search / metadata filter で明示的に指定した場合のみ検索対象とする。

### 4.2 `#app`

`#app` は組み込みタグとして定義する。

`#app` が付いた記録では、記録時点の foreground application metadata をユーザ可視タグへ昇格する。

例:

```text
metadata.application = "Visual Studio Code"
input tag = #app
```

から、概念上は次を生成する。

```text
#app
#Visual Studio Code   source=derived
```

`#Visual Studio Code` はユーザタグとして扱うため、通常検索、Tag Explorer、将来の automation trigger に利用できる。

`#app` 自体を記録に残すかは実装時に選択可能だが、初期実装では provenance と挙動の明示性を優先して残す。

window title についても同じ昇格機能を将来 built-in tag として追加可能だが、初期スコープには含めない。

## 5. Tag Definition

タグの「名前」と「そのタグに紐づく設定」を分離して扱う。

概念モデルとして TagDefinition を導入する。

```text
TagDefinition
  id
  workspace_id
  name
  kind: builtin | user
  display_name
  view_binding?
  enabled
  created_at
  updated_at
```

組み込みタグはアプリケーションが stable id を持つ。

例:

```text
builtin:task
builtin:app
```

ユーザタグは workspace 内で stable id を持つ。

表示名変更を許可しても、automation や view の binding は文字列ではなく tag id を参照する。

### 5.1 Built-in registry and persistence

built-in tag の正本はアプリケーションコード側の registry とする。

DB にはユーザが変更可能な override / binding のみ保持する。

理由:

- アプリ更新で built-in tag を追加・変更できる。
- DB migration でアプリ固有の定義を配布する必要がない。
- stable id によって user tag と同じ binding model を利用できる。

したがって「組み込みタグとユーザタグを完全に同じ DB 行として保存する」のではなく、TagDefinition を resolve した後の domain model を共通化する。

## 6. Tag Explorer

Tag Explorer はタグに関する管理・探索画面である。

サイドバーから独立画面として開く。

### 6.1 Tag list

各タグについて最低限以下を表示する。

- 名前
- built-in / user
- 使用件数
- 最終使用日時
- view binding
- automation pipeline の有無
- enabled / disabled

検索、並び替え、built-in / user filter を提供する。

### 6.2 Tag detail

タグを選択すると以下を設定できる。

- display name / shorthand
- view
- automation pipeline
- pipeline preview / test run
- 対象レコードの preview
- tag rename（user tag のみ）
- tag delete（user tag のみ）

built-in tag は削除不可とし、必要であれば user-level setting として無効化を許容する。

### 6.3 Delete semantics

user tag の削除は、既存 document の provenance を壊さないよう慎重に扱う。

初期実装では以下を推奨する。

- TagDefinition を論理削除する。
- document に付いた過去の assignment は残す。
- 通常の補完候補と Tag Explorer の active list から除外する。
- historical / advanced view では辿れる。

「全記録からタグを除去」は別操作として扱う。

## 7. View binding

タグは automation と独立して view を持てる。

例:

```text
#task     -> task-list
#shopping -> checklist
#meeting  -> table
```

重要なのは、view を pipeline の副作用として決定しないことである。

理由:

- 同じ構造化データに複数 view を適用できる。
- pipeline を変更しても UI の意味が不用意に変わらない。
- built-in tag の特殊 UI と user tag の custom UI を同じ binding model に寄せられる。

初期 view kind は限定した registry とする。

例:

- `default`
- `task-list`
- `checklist`
- `table`

任意 UI plugin は初期スコープ外とする。

## 8. Automation Pipeline

### 8.1 Purpose

タグ付けされた record を source として変換を行い、構造化された成果物を得る。

例:

```text
memo + #task
  -> task parser
  -> structured task rows
  -> task-list view
```

または

```text
memo + #expense
  -> parse amount/date/category
  -> table asset rows
  -> table view
```

成果物は可能な限り Lineage 上で source record から `derived_from` として辿れるようにする。

### 8.2 Pipeline and trigger are separate

既存 `automation_rules` は trigger と処理内容を一つの rule に含めているが、本設計では分離する。

```text
Pipeline = 何を実行するか
Binding  = どのタグに、いつ実行するか
Run      = 実行履歴
```

これにより同じ pipeline を複数タグから再利用できる。

### 8.3 Execution triggers

タグ binding では最低限以下を扱う。

- manual
- on-tagged
- on-record-created-with-tag
- 将来: schedule / explicit refresh

既存の schedule automation は Tag Explorer とは独立した general automation として残すことができる。

## 9. Role of `just`

### 9.1 Initial proposal

ユーザ提供案では、tag pipeline を `just` recipe として実装し、`build-tag-<tag-name>` という recipe を実行して成果物を得る。

Linux では shell、Windows では PowerShell など、各 OS の標準 shell を利用する。

`just` を知らないユーザ向けには Excel 関数に近い built-in transformation を提供し、上級ユーザは `just` を直接記述して任意の DAG を構成する。

### 9.2 Design concern

`just` を DAG の canonical representation にすることは推奨しない。

`just` は task runner として非常に相性がよいが、Lineage が将来必要とする以下の情報を表現・解析するための domain model としては弱い。

- step dependencies
- typed input / output
- retry policy
- cacheability
- deterministic / side-effecting の区別
- permission requirements
- secret requirements
- UI からの編集
- validation
- provenance
- 実行途中の可視化

さらに、recipe 名を `build-tag-<tag-name>` にすると tag rename が実行契約を壊す。

### 9.3 Recommended model

`just` は pipeline definition ではなく **executor / escape hatch** として扱う。

Pipeline 自体は Lineage 側の stable id と step model を持つ。

```text
Pipeline
  id
  name

PipelineStep
  id
  pipeline_id
  type
  config
  depends_on[]
```

初期 step type:

- `builtin-transform`
- `just-recipe`

将来:

- `python`
- `http`
- `llm`
- `agent`

`just-recipe` step は stable pipeline id / tag id に紐づけ、表示名とは切り離す。

例:

```text
lineage-pipeline-<pipeline-id>
```

または config で recipe 名を明示する。

これにより、初心者向け UI と power user 向け `just` を同じ pipeline model に載せられる。

### 9.4 Built-in transformations

「Excel 関数」の名称はコンセプトとしては分かりやすいが、実装概念としては `built-in transformations` と呼ぶ。

例:

- text split
- regex extract
- date parse
- number parse
- map
- filter
- select field
- default value
- concat
- JSON extract

UI は spreadsheet formula に近い簡易表現を提供してよい。

ただし、初期段階から Excel formula syntax そのものを互換実装する必要はない。

## 10. Why not Python as the only pipeline API

API を提供するなら Python だけで DAG を書けばよい、という選択肢は成立する。

ただし Python-only にすると以下が失われる。

- 非プログラマ向けの編集 UI
- pipeline の静的解析
- permission の事前表示
- step 単位の provenance / status
- OS を跨いだ再現性
- simple transformation の portable execution

したがって Lineage が持つべきなのは Python API ではなく **pipeline intermediate representation (IR)** であり、Python / just はその step executor の一つとする。

これが本設計の中心的な判断である。

## 11. Structured output

pipeline の出力を単なる text document に限定しない。

既存の `table_assets` / `rows` / `cells` を利用し、structured asset を成果物として生成できるようにする。

想定 output kind:

- document
- table asset
- task collection
- checklist collection

ただし task / checklist を専用 storage にするか、table asset + view で表現するかは実装前に別途決定する。

初期実装では **table asset + view** に寄せ、専用データ型の増殖を避ける案を第一候補とする。

この考え方により、Notion の database view に近く「データ構造」と「見せ方」を分離できる。

## 12. Built-in tags in initial scope

### `#task`

- kind: built-in
- default view: `task-list`
- built-in capability: complete
- optional default pipeline: text -> task structure

既存の `document_states.done` をすぐ廃止する必要はない。

第1段階では existing task behavior を adapter として新しい TagDefinition / ViewBinding から呼び出す。

構造化 task collection への完全移行は後続フェーズとする。

### `#app`

- kind: built-in
- minos behavior: captured application metadata を user-visible tag に materialize
- default view: default
- pipeline: none

### `#memo`

現行実装には存在するが、今回のタグ再設計では再検討対象とする。

archive / trash は memo だけの capability ではなく、record 全般の lifecycle operation と考える方が自然である可能性が高い。

したがって `#memo` を built-in tag として残すことを前提にせず、既存互換として維持するか削除するかを migration 時に判断する。

## 13. Proposed data model

最終的な名前は実装時に調整するが、概念上は以下を導入する。

```text
tag_settings
  tag_id
  workspace_id
  tag_kind
  tag_name
  display_name
  enabled
  view_kind
  deleted_at
  created_at
  updated_at

pipelines
  id
  workspace_id
  name
  description
  enabled
  created_at
  updated_at

pipeline_steps
  id
  pipeline_id
  step_order / dependency config
  step_type
  config_json

 tag_pipeline_bindings
  tag_id
  pipeline_id
  trigger_kind
  enabled

pipeline_runs
  id
  pipeline_id
  source_document_id
  status
  started_at
  finished_at
  error
```

注: built-in tag の正本を DB に置かない方針なので、`tag_settings` は built-in tag について override row として利用する設計でもよい。実装時には現在の `meta_tags` を拡張する案と新規テーブルを作る案を比較する。

metadata は `document_meta` とは分離した専用 storage を推奨する。

```text
document_context
  document_id
  key
  value
  source
  created_at
```

これにより user-visible tags と noisy context を SQL レベルでも分離できる。

## 14. Relationship with existing automation

既存 `automation_rules` / `automation_runs` は一括削除せず段階移行する。

Migration direction:

1. 現行 automation rule を legacy adapter として維持する。
2. pipeline / binding model を新設する。
3. `meta_match` rule は tag binding へ変換可能にする。
4. LLM backend は将来 `llm` pipeline step として取り込む。
5. schedule は tag に依存しない general trigger として pipeline execution layer 側へ移す。
6. 十分に移行した後で旧 `automation_rules` schema の廃止を判断する。

## 15. Security and permissions

`just`, Python, shell, HTTP 等を許可する場合、pipeline は任意コード実行機構になり得る。

そのため初期設計から step ごとの permission declaration を考慮する。

例:

- filesystem read
- filesystem write
- network
- process execution
- secret access

built-in transformation は原則 sandboxed / no-side-effect とし、`just-recipe` は明示的に trusted / advanced と表示する。

Tag Explorer から pipeline を紐づける際、必要権限を表示できるモデルを目標とする。

## 16. Execution boundary

責務の方向性は以下とする。

### minos

- capture
- tag input / completion
- metadata collection
- `#app` など capture-time built-in behavior
- document creation

minos 自身が複雑な pipeline を実行しない。

### fullos

- Tag Explorer
- tag / view / pipeline settings
- preview
- manual execution UI
- structured result view

### agentos / execution layer

- pipeline execution
- scheduled / background invocation
- external process / API execution
- result recording
- lineage link creation

これにより fullos が lineage を直接生成しない現在の責務境界を維持する。

## 17. Notion-like behavior

本設計で Notion から参考にするのは「何でも block にする」ことではない。

採用したい考え方は以下である。

- source record と structured data を分離する。
- structured data と view を分離する。
- 同じデータに異なる view を適用できる。
- property / tag を入口として表示や workflow を切り替えられる。

Lineage の差別化は、quick capture と provenance を維持したまま、後から structure / automation を適用できる点に置く。

## 18. Phased implementation

### Phase 1: Domain unification

- TagDefinition domain model
- built-in registry の再設計
- user tag resolution
- view binding
- Tag Explorer の read-only list/detail
- metadata と user tag の概念分離
- `#app`

### Phase 2: Tag Explorer management

- user tag rename / delete
- shorthand
- built-in tag enable/disable override
- view selection

### Phase 3: Pipeline core

- pipeline / step / binding / run schema
- built-in transformations
- manual run
- on-tagged trigger
- structured output

### Phase 4: `just` executor

- `just-recipe` step
- OS-specific justfile support
- permission warning
- run logs
- failure handling

### Phase 5: Existing automation migration

- LLM execution as pipeline step
- existing meta_match migration
- schedule integration
- legacy automation cleanup

## 19. Open decisions

実装開始前に以下を確定する。

1. `#app` 自体を document に残すか、capture directive として消費するか。
2. application tag の canonical name を `#Visual Studio Code` とするか、`#app:Visual Studio Code` のような namespace を持たせるか。
3. `#memo` を built-in tag として残すか。
4. task / checklist を専用データ型にするか、table asset + view で統一するか。
5. pipeline DAG の dependency representation。
6. pipeline definition を JSON/DB のみとするか、portable manifest file も持つか。
7. user-created `justfile` の配置場所と trust model。
8. metadata の初期収集項目。

## 20. Recommended decisions for first implementation

最初の Codex 実装ではスコープを広げすぎず、以下を採用することを推奨する。

- built-in / user tag を domain layer の TagDefinition として共通化する。
- metadata を tag storage から分離する。
- `#app` を追加する。
- Tag Explorer を追加する。
- Tag Explorer では tag list / detail / rename / delete / view binding まで実装する。
- pipeline は schema/domain の骨格と built-in transform 1種類まで実装する。
- `just` は pipeline の canonical DSL にせず executor として設計する。
- `just` executor の本実装は次フェーズでもよい。
- existing automation は壊さず adapter / migration path を用意する。
- `#task` は既存挙動を adapter で新しい tag/view model に載せる。

この順序であれば、今回の目的である「組み込みタグとユーザタグの共通化」を先に達成し、任意コード実行や DAG engine まで同時に抱えて設計を不安定にすることを避けられる。
