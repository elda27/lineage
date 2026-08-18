# Tag Explorer / Tag Automation Design

Status: Draft
Decision date: 2026-08-18

> この文書は 2026-08-18 時点の設計判断を記録する Design Doc である。将来の実装や追加判断によって更新されうる。長期的なアーキテクチャ判断は ADR を正本とする。

## 1. Goal

タグ機能を再編し、将来の自動化機能とアプリケーション内部の特殊実装を、同じ「タグに紐づく振る舞い」のモデルへ統合する。

従来の組み込みタグは、タグに該当する記録を特定のビューとして表示し、必要に応じてアプリケーション機能を有効化するためのものだった。たとえば `#task` はタスクリストとして表示し、完了操作を提供する。

一方、組み込みタグ以外のタグでも同じビューや変換を使いたい場合、組み込みタグの特殊処理とは別に機能を実装する必要があった。また既存の自動化機能は `automation_rules` を中心に、タグの特殊挙動とは別系統で存在していた。

本設計では、この二系統を「タグに紐づく処理」として再定義する。

目標は以下。

- 組み込みタグとユーザタグで、表示・変換・自動化の処理モデルを可能な範囲で共有する。
- 自動化を「生成AIへプロンプトを渡す機能」から、「record を入力として recipe を実行し artifact を生成する汎用処理」へ再定義する。
- noisy なコンテキスト情報を通常のタグ空間から metadata として分離する。
- タグごとに view と automation recipe を紐付けられる Tag Explorer を提供する。
- 任意の複雑な処理は `just` recipe としてユーザまたは AI が記述できるようにする。
- Lineage 自身は DAG engine を持たず、処理実装負荷を抑える。
- 冪等性と incremental rebuild の基盤を持ち、将来 DAG engine を導入しても再利用できるようにする。
- Lineage の append-only / provenance の考え方を維持する。

## 2. Current state

現行では主に以下が存在する。

1. `document_meta` / `meta_tags`
   - ユーザが入力したメタ情報を保存する。
   - `meta_tags` は入力補完の学習対象でもある。
2. `BuiltinTag.ts`
   - `task` / `memo` などの組み込みタグを定義する。
   - 組み込みタグから `complete` / `archive` / `trash` の capability を直接決定する。
3. `document_states`
   - 完了・アーカイブ・削除など、ユーザのタグ入力ではない UI 操作結果を保持する。
4. `automation_rules` / `automation_runs`
   - `manual` / `meta_match` / `schedule` などをトリガに生成AI処理を実行する。

今回の変更では、2 と 4 を「タグに紐づく振る舞い」という観点で再整理する。

## 3. Tag types

### 3.1 Built-in tag

アプリケーション側で pre-defined なタグ。

- アプリケーション機能を実現するために minos 上で特殊な挙動を持つ場合がある。
- fullos 上では可能な限り user tag と共通の処理経路を通す。
- OS 連携や入力時の特殊処理など、完全共通化が不自然なものは built-in implementation を許容する。
- stable id を持ち、表示名や alias とは分離する。

初期の組み込みタグ:

- `#task`
- `#app`

`#memo` は archive / trash が record lifecycle の一般機能ではないかという論点があるため、本再設計で存続要否を見直す。

### 3.2 User tag

ユーザが追加するタグ。

- Tag Explorer から表示方法や自動化 recipe を設定できる。
- built-in tag と同じ view binding / automation binding を利用できることを目標とする。

### 3.3 Assignment source

タグの種類と、record にどう付与されたかは分離する。

最低限以下を扱う。

- `user`: ユーザが明示的に入力
- `derived`: metadata や自動化から生成
- `system`: built-in behavior により付与

## 4. Metadata

foreground application、window title、URL など、ユーザ操作のコンテキストから自動収集される情報は通常タグから分離して metadata とする。

目的は、普段の検索・入力補完・タグ一覧に noisy な情報を混ぜないことである。

### 4.1 Default behavior

- foreground application は metadata として保存する。
- window title 等も設定が有効なら metadata として保存する。
- metadata は通常のタグ検索・タグ補完には含めない。
- advanced search / metadata filter で明示的に指定した場合のみ検索対象とする。
- 取得する metadata の種類は設定で制御可能とする。

### 4.2 `#app`

`#app` は組み込みタグとして定義する。

`#app` が付いた record では、記録時点の foreground application metadata をユーザ可視タグへ昇格する。

例:

```text
metadata.application = "Visual Studio Code"
input tag = #app
```

から概念上、以下を生成する。

```text
#app
#Visual Studio Code   source=derived
```

昇格されたタグは通常の user tag として扱い、通常検索、Tag Explorer、automation binding に利用できる。

## 5. Tag Definition

タグ文字列と、そのタグに紐づく設定を分離する。

概念上は以下を持つ。

```text
TagDefinition
  id
  workspace_id
  name
  kind: builtin | user
  display_name
  enabled
  view_binding?
  automation_binding?
  created_at
  updated_at
```

built-in tag の正本はアプリケーションコード側の registry とし、DB には user override / binding を保持する。

binding は表示文字列ではなく stable tag id を参照する。

## 6. Tag Explorer

Tag Explorer はタグに対する探索・管理・設定画面である。

### 6.1 Tag list

各タグについて最低限以下を表示する。

- 名前
- built-in / user
- 使用件数
- 最終使用日時
- view binding
- automation recipe の有無
- enabled / disabled
- automation が managed / external のどちらか

検索、並び替え、built-in / user filter を提供する。

### 6.2 Tag detail

タグを選択すると以下を設定できる。

- display name / shorthand
- view
- automation recipe
- record preview
- test run
- rebuild
- force rebuild
- tag rename（user tag のみ）
- tag delete（user tag のみ）

外部管理処理の場合は「外部実装の変更は自動検知されない」ことを明示する。

### 6.3 Delete semantics

user tag の削除は TagDefinition の論理削除を基本とする。

- 過去の assignment と provenance は残す。
- 通常補完と active list から除外する。
- 「全 record から assignment を除去」は別操作とする。

## 7. View binding

タグは automation と独立して view を持てる。

例:

```text
#task     -> task-list
#shopping -> checklist
#meeting  -> table
```

初期 view kind は限定された registry とする。

- `default`
- `task-list`
- `checklist`
- `table`

任意 UI plugin は初期スコープ外とする。

## 8. Automation redefinition

本設計における automation は以下と定義する。

```text
automation = record を入力として recipe を実行し artifact を生成する汎用処理
```

生成AI呼び出しは automation の本体ではなく、recipe 内から利用可能な処理の一種となる。

```text
LLM / Agent / Python / CLI / HTTP / built-in transform
             ↓
          just recipe
             ↓
          artifact
```

成果物は可能な限り source record から `derived_from` として Lineage 上で辿れるようにする。

## 9. `just` as canonical automation definition

重要なアーキテクチャ判断は ADR-0002 を参照する。

Lineage は独自 DAG engine / Pipeline IR を持たない。

ユーザ定義 automation の正本は `just` recipe とする。

基本的な entry point は以下。

```text
build-tag-<tag-name>
```

ただし rename による破壊を避けるため、実装時は stable tag id と recipe の対応を binding として保持できるようにする。recipe 名自体を永続 ID とみなさない。

`just` を採用する理由:

- DAG / dependency graph を自前実装しなくてよい。
- shell / PowerShell / CLI / Python 等を組み合わせられる。
- 上級ユーザが任意の複雑な dependency を記述できる。
- AI が既存知識を利用して recipe を生成・編集しやすい。
- 独自 DSL / parser / runtime を持たずに済む。

### 9.1 Simple transform

`just` を知らないユーザ向けには、Excel 関数に近い built-in transform を GUI から提供する。

ただし execution model を二重化しない。

GUI 定義も最終的には Lineage の built-in command を呼び出す recipe へ収束させる。

例:

```just
build-tag-expense:
    lineage transform expense
```

これにより以下3経路を同一 execution model にできる。

- GUI で built-in transform を選択
- AI に recipe を生成させる
- ユーザが recipe を手書きする

## 10. Idempotency and incremental rebuild

重要なアーキテクチャ判断は ADR-0002 を参照する。

Lineage は「変更候補の検出」と「冪等性の確定」を分離する。

### 10.1 Processing side

Lineage 管理下の automation definition / script は Git 管理する。

Git diff を利用して処理定義変更を検出し、その変更を recipe 単位の影響範囲へ再解釈する。

必要に応じて just の構造化情報を利用し、recipe dependency に変更を伝播させる。

ただし冪等性の正本は raw diff そのものではなく、処理定義の revision / fingerprint とする。

概念上:

```text
processing_fingerprint
  = recipe definition revision
  + Lineage-managed dependencies revision
```

Lineage が管理する外部 script も Git revision に含める。

### 10.2 External processing

ユーザが Lineage 管理外で完全に管理する script / project / executable は、Lineage が変更追跡しない。

原則:

- 外部処理の変更は自動 invalidate しない。
- ユーザが変更した場合は手動 Rebuild / Force rebuild を行う。
- Force rebuild はキャッシュ削除ではなく、新しい run として履歴を残す。
- run には `forced=true` 相当の情報を保持し、なぜ同じ入力・定義で再実行されたか追跡可能にする。

### 10.3 Data side

`updated_at` / `modified_at` は高速な変更候補抽出に利用する。

ただし冪等性判定の正本にはしない。

最終判定には automation が参照する record 内容から `input_fingerprint` を生成する。

概念例:

```text
input_fingerprint = hash(
  title
  + body
  + relevant tags
  + relevant metadata
)
```

`modified_at` は以下の用途とする。

```text
modified_at
    ↓
変更候補を絞る
    ↓
input_fingerprint
    ↓
本当に入力が変わったか判定
```

### 10.4 Execution key

成功済み実行の再利用判定は概念上以下とする。

```text
execution_key = hash(
  tag_id
  + processing_fingerprint
  + input_fingerprint
)
```

同一 execution key の成功済み run がある場合は原則 skip する。

Force rebuild の場合はこの cache 判定を無視して新規 run を作る。

成果物にも `output_fingerprint` を持てる設計とし、将来は同一出力の artifact reuse に利用可能とする。

### 10.5 Future DAG compatibility

この冪等性モデルは将来自前 DAG engine を導入する場合にも再利用できる。

将来 node 単位へ細分化した場合は、同じ原則を以下へ拡張できる。

```text
node_execution_key = hash(
  node_definition_fingerprint
  + node_input_fingerprint
)
```

したがって、今回の実装は使い捨ての最適化ではなく、将来の incremental DAG execution の cache 基盤となる。

## 11. Managed boundary

責務境界を以下とする。

```text
Git
  -> Lineage-managed automation definition revision

DB
  -> record data revision / input fingerprint

Automation run store
  -> processing + input を結びつけた execution cache / history

External project
  -> Lineage は revision を保証しない
```

Lineage が保証するのは、Lineage 管理範囲内の処理定義と入力データに対する増分再計算である。

## 12. Built-in tags at this point

### `#task`

- task-list view を利用する。
- task 表示・完了操作を提供する。
- 必要な構造化変換は built-in automation recipe として扱う方向へ寄せる。

### `#app`

- foreground application metadata を user-visible tag に昇格する。
- minos 側に特殊入力処理を持つ built-in tag とする。

## 13. Storage direction

既存の以下は可能な限り活用する。

- `meta_tags`
- `document_meta`
- `document_states`
- `automation_runs`
- `table_assets`
- `rows`
- `cells`
- `links`

自動化結果の実行履歴には、少なくとも将来的に以下を保持できる必要がある。

```text
source_document_id
tag_id
recipe binding
processing_fingerprint
input_fingerprint
execution_key
output_fingerprint
status
forced
started_at
finished_at
```

実際の migration 形状は実装時に既存 schema と整合させて決める。

## 14. Implementation phases

### Phase 1: domain reorganization

- built-in / user tag の共通 TagDefinition model を導入
- metadata と user tag を分離
- `#app` の metadata promotion を実装
- 既存 `#memo` の位置付けを見直す

### Phase 2: Tag Explorer

- tag list / detail
- rename / delete / disable
- shorthand
- view binding

### Phase 3: recipe automation

- tag -> just recipe binding
- built-in transform を recipe 経由へ統合
- manual run / test run
- automation run history

### Phase 4: idempotency

- Git ベースの managed processing revision
- recipe 影響範囲判定
- data candidate detection by updated_at
- input fingerprint
- execution key cache
- rebuild / force rebuild

### Phase 5: advanced authoring

- AI による just recipe 作成・修正支援
- external recipe / project の明示
- additional built-in transformations

## 15. Non-goals at 2026-08-18

- Lineage 独自 DAG engine の実装
- 独自 pipeline DSL の実装
- 外部プロジェクトのファイル監視・依存関係自動解析
- Python runtime の内蔵
- 任意 UI plugin system
- 外部処理変更の完全自動検出

## 16. Related ADR

- ADR-0001: 組み込みタグの状態を `document_states` に分離し、削除は論理削除とする
- ADR-0002: 自動化定義に just recipe を用い、Lineage は DAG engine を持たない
