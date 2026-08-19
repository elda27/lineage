# Lineage

思いついたことを、その場で記録し、あとから検索・整理・自動化へつなげるローカルファーストのデスクトップアプリです。

Lineage は記録そのものだけでなく、「どの記録から何が作られたか」という由来（lineage）も追跡します。自動化で生成された結果も元の記録と結び付き、hash-chain によって履歴の改変を検知できる形で保存されます。

> [!WARNING]
> 現在は初期開発版です。Windows 向けのローカル利用を中心に実装しており、仕様やデータ構造は変更される可能性があります。

## 現在できること

- `Alt + Space` で、作業中のアプリからすぐに記録画面を開く
- 本文と `#タグ` を SQLite に保存する
- タグ候補を過去の入力頻度や短縮入力から補完する
- `#task` などの組み込みタグに応じた表示・操作を行う
- 記録の一覧表示、全文検索、詳細表示、アーカイブを行う
- タグの利用状況や表示方法を Tag Explorer で管理する
- メタ情報、手動操作、スケジュールをきっかけに自動化を実行する
- 自動化の実行結果を元の記録から辿れる形で保存する
- Lineage の hash-chain を検証する
- GitHub Releases からアプリを更新する

## アプリ構成

| コンポーネント | 役割 |
| --- | --- |
| **minos** | トレイに常駐するクイック入力アプリ。最小操作で記録を残します。 |
| **fullos** | 記録の検索・整理、Tag Explorer、自動化、設定を扱うメインアプリです。 |
| **agentos** | 自動化を実行する非常駐の CLI エンジンです。 |
| **lineage-core** | 記録、タグ、自動化、SQLite、hash-chain を共有する Rust ライブラリです。 |

## インストール

現在の配布対象は Windows です。

1. [Releases](https://github.com/elda27/lineage/releases) から最新の MSI をダウンロードします。
2. MSI を実行して Lineage をインストールします。
3. インストール後、minos がトレイで起動します。

main の最新実装を試す場合は [nightly](https://github.com/elda27/lineage/releases/tag/nightly) を利用できます。nightly は開発版であり、次の nightly へはアプリ内から自動更新されません。

MSI には minos、fullos、agentos がまとめて含まれます。正式版では fullos が更新を確認し、利用可能な更新をアプリ内に表示します。

## 基本操作

### 記録する

1. どのアプリを使用中でも `Alt + Space` を押します。
2. 内容を入力します。タグは `#タグ`、値付きタグは `#タグ=値` と書けます。
3. `Ctrl + Enter` で保存します。
4. 続けて入力する場合は `Ctrl + Shift + Enter` で保存します。
5. `Esc` で入力画面を閉じます。

例:

```text
API 設計の判断理由を ADR に残す #task #project=lineage
```

minos から fullos を開くには `Alt + F` を使います。

### 見返す・整理する

fullos では、記録の一覧、検索、タグ、組み込みタグに応じた操作、自動化ルールと実行履歴を確認できます。

自動化で API キーを使う場合、キーは Lineage の SQLite データベースには保存されません。Windows 資格情報マネージャーなど、OS の資格情報ストアに保存されます。

## データの保存

Windows では、記録と設定をローカルの SQLite データベースに保存します。

```text
%LOCALAPPDATA%\minos\lineage.db
```

記録を生成する操作は append-only の Lineage 台帳へ追加されます。各 link は直前の link のハッシュを持つため、途中の履歴が書き換えられた場合は検証時に検出できます。

クラウド同期や Cloudflare D1/R2 への保存は設計上の将来構想であり、現在の配布版の標準動作ではありません。

## 開発

### 必要なもの

- Windows
- Rust stable
- Node.js 22
- pnpm 11
- [just](https://github.com/casey/just)
- Tauri 2 の [Windows prerequisites](https://v2.tauri.app/start/prerequisites/)

### 起動

```powershell
git clone https://github.com/elda27/lineage.git
cd lineage
just dev
```

`just dev` は fullos の依存関係を導入し、Vite と Tauri を開発モードで起動します。

### よく使うコマンド

```powershell
# Rust 側のテスト
just test-rust

# minos と agentos のリリースビルド
just build-rust

# fullos のフロントエンドを型チェックしてビルド
pnpm --dir fullos build

# Windows MSI を生成
just msi

# VERSION と各マニフェストの整合性を確認
just version-check
```

MSI の出力先:

```text
fullos/src-tauri/target/release/bundle/msi/
```

署名付き updater artifact をローカルで生成する場合は、Tauri updater の署名鍵が別途必要です。リリースの詳細は [docs/release.md](docs/release.md) を参照してください。

## リポジトリ構成

```text
lineage/
├─ minos/              # GPUI 製クイック入力アプリ
├─ fullos/             # React + Tauri 製メインアプリ
├─ agentos/            # 自動化 CLI
├─ lineage-core/       # 共有ドメイン、ユースケース、永続化
├─ db/schema.sql       # SQLite スキーマ
├─ docs/concept/       # プロダクト構想
└─ docs/arch/          # ADR と Design Doc
```

Rust 側は `lineage-core` にドメイン・ユースケース・永続化を集約し、minos と agentos を薄い実行ファイルとして保ちます。fullos は SQLite の競合を避けるため、Lineage を生成する自動化処理を同梱の agentos へ委譲します。

## 設計上の原則

- **ローカルファースト** — 日常の記録はローカル SQLite へ保存する
- **記録を最優先** — 整理方法を決める前に、まず最小操作で残せるようにする
- **由来を失わない** — 生成物を元の記録と `derived_from` で結ぶ
- **改変を検知する** — Lineage を append-only の hash-chain として保持する
- **状態と記録を分ける** — 完了・アーカイブなどの UI 状態で元の記録を書き換えない
- **自動化を追跡可能にする** — 実行結果と履歴を Lineage 上に残す

詳細:

- [Minimal Architecture](docs/concept/MINIMAL_ARCHITECTURE.md)
- [UI specification](docs/ui.md)
- [Architecture Decision Records](docs/arch/adr/README.md)
- [Tag Explorer / Tag Automation Design](docs/arch/design-doc/2026-08-18-tag-explorer-and-tag-automation.md)
- [Release and updater](docs/release.md)

## 開発状況

現在は、クイック入力、記録管理、タグ、ローカル自動化の基盤を優先して開発しています。

表計算との統合、クラウド接続、複数端末同期などはコンセプト文書に含まれますが、現行版で利用できる機能としては扱っていません。実装済みの範囲と将来構想は、コードと ADR / Design Doc を基準に区別してください。
