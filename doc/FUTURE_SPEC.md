Lineage Projection System (LPS)

1. 背景

研究開発、設計開発、事業企画などの知的生産活動では、多数のドキュメントや分析結果が作成される。

例として研究開発では以下のような流れが存在する。

仮説立案
↓
実験計画
↓
実験実施
↓
評価レポート
↓
次アクション決定
↓
論文化判断
↓
予稿ストーリー
↓
予稿
↓
発表ストーリー
↓
発表スライド
↓
発表原稿

各成果物は前段の成果物を人間が解釈・再構成した結果である。

しかし現状の文書管理システムでは、

- 成果物間の変換履歴
- なぜその成果物が生まれたか
- 後続成果物への影響

を十分に管理できていない。

また過去の成果物を別視点から眺め直し、新たな気付きや研究テーマを発見することも困難である。

本システムは、人間による知的変換の履歴(Lineage)を管理し、多様な視点への投影(Projection)を支援することを目的とする。

---

2. 設計思想

2.1 リネージュを事実として保存する

システムは人間の意図を保存しない。

理由は以下である。

- 意図は明示されない場合が多い
- 本人でも後から説明できない場合がある
- 解釈は時代や文脈によって変化する

そのため保存対象は以下のみとする。

何から
何が
作られたか

つまり変換履歴そのものを保存する。

---

2.2 意味付けは後付けで行う

分類、タグ、テーマ、カテゴリは保存対象ではない。

代わりに利用者が必要に応じて生成する。

例：

- 研究テーマ視点
- 技術視点
- 顧客課題視点
- 製品機能視点
- 組織視点

これらはProjectionとして生成される。

---

2.3 再整理行為そのものを価値とする

利用者が

これは品質研究ではない
これは材料研究である

と再分類する行為を知識発見プロセスとみなす。

システムは正解を保持しない。

利用者による修正履歴を学習し、将来のProjection精度向上に利用する。

---

3. ドメインモデル

3.1 Core Entity

Asset

知的成果物を表す。

例：

- テーブル
- グラフ
- ドキュメント
- スライド
- 実験結果
- 論文

Asset
 ├─ id
 ├─ type
 ├─ title
 ├─ version
 ├─ created_at
 └─ metadata

---

Lineage

成果物間の変換関係を表す。

Lineage
 ├─ seq            （ワークスペース内の連番。順序を確定する）
 ├─ source_asset_id
 ├─ target_asset_id
 ├─ actor
 ├─ created_at
 ├─ content_hash   （このレコードの正規化ハッシュ）
 ├─ prev_hash      （直前レコードの content_hash）
 └─ metadata

Lineage は真正性(authenticity)を担保するため append-only とし、
content_hash / prev_hash により1本の hash-chain を構成する。

- 各レコードの content_hash = SHA-256(正規化(seq, source, target, actor, created_at, prev_hash))
- prev_hash には直前レコードの content_hash を入れる（先頭は genesis 定数）
- 途中の1件でも書き換えると以降の content_hash が破綻するため、改ざんを検知できる
- 検証は台帳を seq 昇順に再計算するだけで、ローカル・クラウドどちらでも同一ロジックで行える

これにより「何から何が作られたか」という事実そのものが、
後から無断で改変されていないことを証明できる。
実装方式の詳細は doc/MINIMAL_ARCHITECTURE.md の「Lineage の真正性担保」を参照。

例：

実験結果
↓
評価レポート

評価レポート
↓
予稿

予稿
↓
スライド

---

Projection

ある視点から見た解釈結果。

Projection
 ├─ id
 ├─ name
 ├─ description
 ├─ generated_by
 └─ created_at

例：

- 技術テーマ別
- 製品別
- 顧客別
- 組織別

---

Projection Result

投影結果。

ProjectionResult
 ├─ projection_id
 ├─ asset_id
 ├─ score
 ├─ labels
 └─ reasoning

---

Projection Feedback

利用者による修正履歴。

ProjectionFeedback
 ├─ projection_result_id
 ├─ action
 ├─ user
 └─ created_at

action:

- accept
- reject
- relabel
- merge
- split

---

4. Asset Type

Table

構造化データ。

例：

- 仮説一覧
- 実験一覧
- 実験結果一覧

---

Graph

ノード・エッジ構造。

例：

- アイデアマップ
- 因果関係図
- 技術関連図

---

Document

文章成果物。

例：

- 実験計画書
- 評価レポート
- 予稿
- 論文

---

Presentation

発表資料。

例：

- スライド
- 発表原稿

---

5. 実現したいユースケース

ケース1

研究成果の論文化

仮説一覧(Table)
↓
実験計画(Document)
↓
実験結果(Table)
↓
評価レポート(Document)
↓
予稿(Document)
↓
スライド(Presentation)

利用者は全ての成果物の変換履歴を追跡できる。

---

ケース2

過去研究の再探索

利用者が

AI研究として見たい

と指示する。

システムは過去成果物を再投影する。

利用者は

これはAIではない
これはAI関連

と修正する。

修正結果はProjection Feedbackとして保存される。

---

ケース3

新規研究テーマ探索

評価レポート群を対象に

未活用成果

というProjectionを作成する。

システムは既存テーマと異なるクラスタを生成する。

利用者は新たな研究テーマ候補を発見できる。

---

6. システムの本質

本システムは文書管理システムではない。

また知識管理システムでもない。

本システムは

Lineage
+
Projection
+
Feedback Learning

を核とする。

保存されるのは事実である。

意味は後から何度でも再生成できる。

利用者は過去成果物を異なる視点で投影し続けることで、新たな知識や研究テーマを発見できる。
