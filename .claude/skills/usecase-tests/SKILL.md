---
name: usecase-tests
description: Derive and maintain minos integration tests from the scenarios in docs/minos/USECASE.md. Use when invoked as /usecase-tests, when implementing a minos feature that changes user-visible behavior, or when USECASE.md scenarios are added or changed and tests must follow.
---

# usecase-tests

docs/minos/USECASE.md のシナリオを結合テストへ落とし込み、
シナリオとテストの対応を維持するための規約と手順。

シナリオが正、テストはその写像である。どちらかを変えたらもう一方を追随させる。

## マッピング規約

- USECASE.md の各シナリオ節 ↔ `minos/tests/` 配下の結合テストファイル 1 本

| USECASE.md | テストファイル |
|---|---|
| 2章 主シナリオ(業務中の todo 登録) | `minos/tests/usecase_todo_registration.rs` |
| 5.1 雑メモ | `minos/tests/usecase_memo_registration.rs` |
| 5.2 将来のテーブル例(スキーマ駆動) | `minos/tests/usecase_schema_driven_tables.rs` |

- 各テストファイルの冒頭コメントに対応シナリオ節番号を記載する
  (例: `//! docs/minos/USECASE.md §2 主シナリオ: 業務中の todo 登録`)
- テスト関数名はシナリオのステップ・観点が分かる名前にする
  (例: `registers_todo_with_auto_tag_from_foreground_app`)

この双方向の対応(doc → テスト冒頭コメント、テスト → 節番号)で追跡可能にする。

## テスト対象の切り分け

結合テストで検証するのは「フォーム値を受け取ってから DB に確定するまで」のフロー。

実物を使う:

- SQLite … テンポラリファイル(または `tempfile` クレート)に実スキーマを適用し、
  rows / cells / documents / links への書き込み結果と hash-chain の整合
  (`LineageLedger::verify`)を実 DB で検証する

モックする:

- OS 依存(ホットキー、フォアグラウンド検出、スクショ)…
  `ForegroundContextSource` trait(docs/minos/DESIGN.md 3.4)のモック実装を注入する。
  例: `process_name = "chrome"`、固定 PNG バイト列を返すスタブ
- GPUI の UI 層 … 結合テストの対象外。フォーム値は構造体で直接渡す

## 検証観点(最低限)

- 1件の登録で rows / cells が期待どおり書かれる(tags は JSON 配列、auto 列が埋まる)
- スクショありの場合: documents(attachment)+ links(attachment_for)が書かれ、
  すべて同一トランザクションで確定している(途中失敗時に部分書き込みが残らない)
- links の hash-chain が verify を通る(seq 連番、prev_hash 連結)
- required 列が未入力なら登録が拒否される

## 手順

1. `docs/minos/USECASE.md` を読み、シナリオ節を列挙する
2. `minos/tests/` の既存テストと突き合わせ、未カバーのシナリオ・観点を特定する
3. マッピング規約に従ってテストを生成・更新する(冒頭コメント必須)
4. `cd minos && cargo test` で確認する
5. シナリオ側を変更した場合は、対応テストの冒頭コメント・関数名も追随させる

## 現状の注意

minos は実装初期(Hello World)段階のため、テスト本体は機能実装
(docs/minos/DESIGN.md 9章の実装順)に合わせて段階的に作られる。
実装が存在しないシナリオのテストを先に書く場合は `#[ignore]` で明示する。
