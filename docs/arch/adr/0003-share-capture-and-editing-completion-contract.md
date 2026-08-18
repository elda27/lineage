---
id: ADR-0003
title: minos の入力と fullos の編集でメタ情報補完契約を共通化する
status: accepted
date: 2026-08-18
area:
  - application
  - quality
scope:
  - application
owners:
  - lineage
related: []
supersedes: []
supersededBy: null
discussion: null
---

# ADR-0003: minos の入力と fullos の編集でメタ情報補完契約を共通化する

## Context

minos は素早く記録するためのネイティブ入力画面、fullos は保存した記録を検索・編集する画面であり、起動経路と UI framework は異なる。一方、利用者にとっては同じ記録本文を扱う入力体験である。メタ情報の候補、一致順、確定後の文字列、キーボード操作が画面ごとに異なると、入力し直すたびに操作を学び直す必要がある。

起動時間と描画部品まで同一にすることは、常駐する minos と WebView を使う fullos の責務・性能要件に反する。しかし、補完の意味と操作は framework に依存しない。個別画面への補完ロジックの複製は、候補数やキー処理が静かに分岐するドリフトを招く。

## Decision

minos の新規入力と fullos の記録本文編集は、次のメタ情報補完契約を共有する。

- カーソル直前の `#` トークンを補完対象とし、空白、改行、`#`、`,`、`、` を終端とする。
- ラベル前方一致、短縮文字列前方一致、ラベル部分一致の順に候補を優先し、同順位では利用回数、ラベルの順に並べる。
- 最大12候補を表示する。
- `ArrowUp` / `ArrowDown` で選択し、`Enter` / `Tab` で確定、`Escape` で閉じる。IME 変換中のキー確定には介入しない。
- 確定時はカーソルまでのトークンを `#<label> ` に置き換え、入力を継続できる位置へカーソルを戻す。
- 候補取得に失敗しても本文の入力と保存を妨げない。

起動方式、framework 固有の input widget、視覚表現は共通化の対象外とする。Rust と TypeScript の境界を越えて UI 実装を共有しようとせず、候補のドメイン規則を各アプリの core 境界に置き、UI adapter は契約へ従う。fullos 内では検索と本文編集が同一の補完 controller と候補リストを利用する。

ドリフト防止のため、補完契約を変更する際は minos と fullos を同一変更単位で更新し、この ADR の決定を変える場合は新しい ADR で置き換える。候補数や順位規則を画面固有に上書きしない。

## Options considered

### Option 1: framework 非依存の契約を共有し、UI adapter を分ける

採用案。常駐アプリの起動性能を保ったまま利用者の操作を揃えられ、fullos 内の重複も除去できる。言語ごとの adapter と整合性の検証は必要になる。

### Option 2: fullos の編集画面へ補完を個別実装する

局所的には実装しやすいが、検索、編集、minos の3箇所で候補数やキー操作が分岐するため採用しない。

### Option 3: minos と fullos で同じ UI runtime と widget を使用する

コード共有は最大化できるが、minos の常駐・即時入力という性能要件を fullos の WebView 構成へ従属させる。起動方式まで揃える必要はないため採用しない。

### Option 4: 補完を minos に限定する

実装量は少ないが、編集時に新規入力と異なる操作を要求し、利用者の期待と再入力効率を損なうため採用しない。

## Consequences

### Positive

- 新規入力、検索、編集で同じ候補とキーボード操作を利用できる。
- fullos の検索と編集の補完処理を一箇所で修正できる。
- UI framework の選択や minos の起動性能を維持できる。
- 将来の補完変更について、同時に確認すべき範囲が明示される。

### Negative

- Rust と TypeScript の adapter 間の契約整合性を継続して確認する必要がある。
- framework 固有の制約により、候補 popup の外観や細かな描画タイミングは完全には一致しない。
- 補完候補取得のため、fullos の編集画面にも application port への依存が生じる。

### Follow-up

- 補完契約の例を両言語の自動テストで固定し、順位、終端文字、置換結果、候補上限を検証する。
- 新しい記録編集 surface を追加するときは、独自の補完処理ではなく各アプリの共通 adapter を利用する。
- UX の変更レビューでは minos の入力、fullos の検索、fullos の本文編集を対象にする。

## References

- [UI仕様](../../ui.md)
- [ADR管理方針](./README.md)
