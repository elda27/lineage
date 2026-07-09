Record-Centric Lineage Platform

1. 概要

本システムは、表計算データと人間による記録を統合管理するためのプラットフォームである。

従来のExcelやGoogle Spreadsheetは数値管理に優れる一方で、

- なぜその数値になったのか
- どのような判断を行ったのか
- 後からどのような改善を行ったのか

といった人間の記録を継続的に管理することが難しい。

一方でNotionやObsidianは文章管理に優れるが、数値データとの関係性が弱い。

本システムは、

表計算
+
記録
+
履歴管理

を統合した仕組みを提供する。

---

2. 解決したい課題

投資管理

例：

日付| 銘柄| 損益
2026-05-21| SOXL| -8,000

利用者は取引結果だけでなく、

- なぜ損切りしたか
- どのような判断だったか
- 次回どう改善するか

を残したい。

しかし現状は、

- Excel
- メモ帳
- Notion
- SNS

などに分散しやすい。

---

研究開発

例：

実験ID| 精度
exp001| 87.5

利用者は、

- 実験条件
- 気付き
- 改善案
- 次の実験方針

を記録したい。

しかし数値と文章が分離しやすい。

---

設計開発

例：

- 設計レビュー
- 不具合解析
- 品質改善

において、

成果物と判断履歴が分離しやすい。

---

3. 設計方針

3.1 記録を最優先とする

本システムの主目的は記録である。

利用者は

表を見る
↓
気付きを残す
↓
後で見返す

だけで良い。

Lineageは内部で自動管理される。

---

3.2 スマホで入力できることを重視する

利用者はPCで分析するより、

スマホでその場の気付きを残すことが多い。

そのため、

行をタップ
↓
メモを書く
↓
保存

を最小操作とする。

---

3.3 文書ではなく記録として扱う

利用者にとっては

ドキュメント

ではなく、

メモ
コメント
振り返り
改善案

である。

内部的にはDocumentとして保存するが、UIでは意識させない。

---

4. ドメインモデル

Asset

システム内で管理される成果物。

Asset
 ├─ id
 ├─ asset_type
 ├─ title
 ├─ created_at
 └─ metadata

asset_type:

- table
- document
- attachment

---

Table Asset

表計算データ。

例：

- 取引履歴
- 実験結果
- 問い合わせ一覧

---

Document Asset

記録。

例：

- 損切り理由
- 実験メモ
- 改善案
- 振り返り

---

Attachment Asset

添付ファイル。

例：

- PDF
- Excel
- 画像
- スライド

---

Lineage

Asset間の関係。

Lineage
 ├─ seq            （ワークスペース内の連番）
 ├─ source_asset_id
 ├─ target_asset_id
 ├─ relation_type
 ├─ actor
 ├─ created_at
 ├─ content_hash   （正規化ハッシュ）
 └─ prev_hash      （直前レコードの content_hash）

relation_type:

- memo_for
- derived_from
- references
- evidence_for

Lineage は append-only で記録し、content_hash / prev_hash による hash-chain で
真正性(改ざん検知)を担保する。詳細は doc/MINIMAL_ARCHITECTURE.md を参照。

---

Table Structure

Table

Table
 ├─ id
 ├─ name
 └─ schema

---

Row

Row
 ├─ id
 ├─ table_id
 └─ created_at

---

Cell

Cell
 ├─ row_id
 ├─ column_id
 ├─ raw_value
 ├─ computed_value
 ├─ formula
 └─ updated_at

---

5. 計算機能

Spreadsheet Formula

最低限以下をサポートする。

- 四則演算
- SUM
- AVERAGE
- IF
- ROUND
- DATE

例：

=[売値]-[買値]

=[損益]/([買値]*[数量])

---

External Function

外部APIによるセル生成。

例：

=FETCH("stock.close", "SOXL", "2026-05-21")

利用用途：

- 株価
- 為替
- 指数
- ニュース
- 社内API

---

6. MVP

MVP対象

テーブル管理

- CSVアップロード
- Spreadsheet編集
- 計算式

---

行単位記録

利用者は行に対して記録を残せる。

例：

SOXL損切り

決算期待で入ったが、
期待先行の値動きだったため撤退。

---

添付

行に対して

- 画像
- PDF
- URL

を添付可能。

---

記録履歴

時系列で閲覧できる。

取引
 ↓
損切り理由
 ↓
改善案
 ↓
次回ルール

---

7. 将来拡張

将来的には

- 全文検索
- 類似記録検索
- AI要約
- AIタグ提案
- 視点別整理

を追加できる。

ただしMVPでは対象外とする。

まずは

表計算
+
記録
+
履歴

を確実に実現する。
