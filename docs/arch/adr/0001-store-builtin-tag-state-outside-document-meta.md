---
id: ADR-0001
title: 組み込みタグの状態を document_states に分離し、削除は論理削除とする
status: superseded
date: 2026-08-14
area:
  - domain
  - data
scope:
  - application
owners:
  - elda27
related: []
supersedes: []
supersededBy: ADR-0004
discussion: null
---

# ADR-0001: 組み込みタグの状態を document_states に分離し、削除は論理削除とする

## Context

minos が残す記録には利用者が自由にメタ情報（`#タグ`）を付けられる。このうちいくつかを
「組み込みタグ」として扱い、付いているとアプリ側の機能が有効になるようにしたい
（`#タスク` なら完了チェック、`#メモ` ならアーカイブとゴミ箱）。

ここで決める必要があったのは、機能が生む「状態」をどこに持つかである。

- 完了・アーカイブ・ゴミ箱は、利用者が打った文字ではなく画面の操作の結果である。
- メタ情報は minos の入力補完の学習対象であり、検索条件でもある。
- `documents` は `links` から参照される。Lineage は append-only で、link は後から
  書き換えないため、参照先が消えると辿れない link が残る。
- 記録の書き込み経路は minos と agentos（lineage-core）に集約しており、fullos の
  webview は読み出しと「lineage を生まない行」の書き込みだけを行う。

## Decision

組み込みタグの機能が生む状態は、`document_meta` とも `documents` とも別の
`document_states` テーブル（`document_id` を主キーとする1行）に持つ。行が無い記録は
「未完了・未アーカイブ・ゴミ箱でない」という既定の状態として扱う。

削除は行の物理削除ではなく `deleted_at` を立てる論理削除とする。アーカイブは
`archived_at`、完了は `done` / `done_at` で表す。

どのラベルがどの組み込みタグで、どの機能を有効にするかの定義は domain
（`fullos/core/domain/memo/BuiltinTag.ts`）に1か所だけ置き、画面・SQL には持たせない。

この状態は lineage(link) を生まないので、fullos の webview から直接書いてよい行として扱う。

## Options considered

### Option 1: `document_states` を新設する（採用）

状態を専用テーブルへ分ける。

- 利点: メタ情報の意味（利用者が付けたタグ）を汚さない。補完候補の学習や検索に
  「完了」「アーカイブ」が混ざらない。状態を持つ記録の行しかできないので、既存の
  記録に対する移行作業が要らない。
- 利点: `documents` に触らないので、minos / agentos の書き込み経路を変えずに済む。
- 欠点: 一覧の組み立てで documents と states の2つを読み、突き合わせる必要がある。

### Option 2: `document_meta` に `#完了` `#アーカイブ` を足す

状態もタグとして表現する。

- 利点: テーブルが増えない。既存の検索がそのまま使える。
- 欠点: 利用者のタグ空間に、利用者が打っていないタグが混ざる。`meta_tags` の
  学習（使用回数・補完候補）が操作の結果で歪む。`#完了` を自分で打った記録と
  チェックした記録の区別も付かない。
- 採用しなかった理由: メタ情報は「利用者の入力の学習結果」であるという前提を壊すため。

### Option 3: `documents` に列を足す

`documents` に `done` / `archived_at` / `deleted_at` を持たせる。

- 利点: 突き合わせが不要で、一覧の SQL が1本で済む。
- 欠点: `documents` は minos と agentos が書くテーブルで、fullos からの書き込み対象に
  なると「記録そのものは minos / agentos が書く」という線引きが曖昧になる。
  記録の内容と、fullos だけが持つ表示上の状態が同じ行に同居する。
- 採用しなかった理由: 書き込みの責務境界を保つほうを優先した。

### Option 4: 削除は `documents` の行を消す（物理削除）

- 利点: 消えたものが残らない。
- 欠点: `links` の `target_id` が指す先が失われる。link は append-only で書き換えられない
  ため、辿れない link が鎖に残り続ける。
- 採用しなかった理由: Lineage の真正性（何から何が作られたかを辿れること）を壊すため。

## Consequences

### Positive

- 利用者のメタ情報と、アプリの操作結果が混ざらない。
- 記録を消さないので、削除した記録から派生した結果も来歴を辿れる。
- 組み込みタグの追加は domain の定義1か所で済み、永続化の形は変えなくてよい。

### Negative

- 一覧の取得が documents と document_states の2クエリになる。
- ゴミ箱に入れた記録もディスクからは消えない。容量を戻す手段（完全削除）は別に要る。
- 状態を持つのが fullos だけなので、状態を見る必要のある処理（自動化の対象選び）は
  そのつど `document_states` を見に行く必要がある。

### Follow-up

- クラウド接続（D1）を足すときは、同じ port の実装を d1 側にも用意する。
- ゴミ箱の中身を見る画面と、完全削除の扱いは別途決める。

## References

- docs/ui.md「組み込みタグ」
- docs/concept/MINIMAL_ARCHITECTURE.md「2. 全体構成」「4. Lineage の真正性担保」
