---
name: minos-dev
description: Develop the minos ultra-lightweight DB-entry app (Rust + GPUI) in this repo. Use when adding/changing anything under minos/ — features, forms, table schemas (schema_json), SQLite persistence, hotkey/foreground/screenshot integration, the hash-chain, or the MSI packaging — and when the change might require updating docs/minos/.
---

# minos-dev

minos(超軽量DB登録アプリ、Rust + GPUI)を、
入力特化の思想と Lineage 互換のデータモデルを壊さずに継続開発するための手順。

正本ドキュメント(実装前に必読):

- `docs/minos/CONCEPT.md` … 思想。何をやらないか(non-goals)もここ
- `docs/minos/USECASE.md` … シナリオ。結合テストの正
- `docs/minos/DESIGN.md` … 技術設計。スキーマ・hash-chain・OS 抽象化境界

スキーマと hash-chain の原典は `docs/concept/MINIMAL_ARCHITECTURE.md`(Lineage 構想の正本)。

## スタックの要点

- Rust edition 2024 / GPUI + gpui-component(`minos/Cargo.toml`)
- SQLite は rusqlite (bundled)。DB は複数アプリ共有(WAL、DESIGN.md 6章)
- 配布は WiX MSI(ルート `justfile` の `msi` ターゲット)
- OS 依存(ホットキー / フォアグラウンド検出 / スクショ)は
  `ForegroundContextSource` trait に閉じ込める(DESIGN.md 3.4)

## 4つの不変条件(変更前にこれを満たすか確認)

1. **入力特化**: minos に一覧・検索・編集 UI を足さない。閲覧はビュワー(別アプリ)の責務
2. **体感速度**: ホットキー→入力可能まで 100ms 目標。常駐 show/hide 方式を壊さない。
   起動時パスに重い処理(同期 I/O、ネットワーク)を足さない
3. **1トランザクション書き込み**: 1件の登録(rows + cells + documents + links)は
   同一トランザクションで確定する
4. **Lineage 真正性**: links は append-only。`content_hash`/`prev_hash` の hash-chain を切らない。
   canonicalize(キー順固定 JSON)と SHA-256 の仕様は MINIMAL_ARCHITECTURE.md 4章と互換に保つ

## ドキュメント更新ルール(ドリフト対策)

挙動・スキーマ・アーキテクチャを変える変更は、**同一コミットで docs/minos/ の該当箇所を更新する**。
対応表:

| 変更の種類 | 更新する doc |
|---|---|
| DB スキーマ・schema_json のヒント語彙 | DESIGN.md 4章 |
| 登録トランザクション・hash-chain | DESIGN.md 4.2 / 5章 |
| ホットキー・フォアグラウンド検出・スクショ等の OS 連携 | DESIGN.md 3章 |
| DB/添付のファイル配置・共有規約(ビュワーに影響) | DESIGN.md 6章 |
| 入力フロー・UX(操作手順、自動付与の挙動) | USECASE.md 2〜3章 |
| 新しいユースケース・テーブル種別 | USECASE.md 5章(+必要なら 1章) |
| やること/やらないことの方針転換 | CONCEPT.md(特に 5章 non-goals) |
| doc ファイルの追加・削除 | docs/README.md の索引 |

doc 更新が不要な変更(リファクタ、依存更新等)はその旨をコミットメッセージか PR で明示する。
Stop フック(`.claude/hooks/check-doc-drift.sh`)が minos/ と docs/minos/ の差分の非対称を検知して
リマインドを出す。

## テスト規約

挙動を変える変更は、USECASE.md のシナリオに対応する結合テストを追随させる。
規約と手順は `.claude/skills/usecase-tests/SKILL.md` に従う(シナリオ ↔ `minos/tests/` の対応)。

## 完了前チェックリスト

- [ ] CONCEPT.md の non-goals に反する機能を足していない
- [ ] 登録処理が1トランザクションに収まっている
- [ ] links を更新・削除していない(append-only / prev_hash 連結)
- [ ] OS 依存コードが trait 境界の外に漏れていない
- [ ] 上記対応表に従って docs/minos/ を更新した(不要なら明示した)
- [ ] USECASE.md のシナリオに対応する結合テストが追随している
- [ ] `cargo build`(minos/ 内)が通る
