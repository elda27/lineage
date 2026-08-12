# リリースと自動更新

Windows 向け配布は GitHub Release を単一の配布元とする。
リリースを publish すると `.github/workflows/release.yml` が走り、インストーラを
そのリリースに添付する。fullos はそのリリースの `latest.json` を見て自己更新する。

インストーラは **fullos と minos をまとめた 1 本の MSI**。

| 成果物                               | 中身                          | 生成元                                           |
| ------------------------------------ | ----------------------------- | ------------------------------------------------ |
| `lineage_<version>_x64_en-US.msi`     | fullos + minos のインストーラ | `tauri build --bundles msi`（Tauri も WiX 経由） |
| `lineage_<version>_x64_en-US.msi.sig` | 上記の minisign 署名          | Tauri updater                                    |
| `latest.json`                        | 更新マニフェスト              | Tauri updater                                    |

## MSI の中身

| 配置                                             | 生成元                                                    |
| ------------------------------------------------ | --------------------------------------------------------- |
| `%ProgramFiles%\lineage\fullos.exe`              | Tauri の主バイナリ                                        |
| `%ProgramFiles%\lineage\minos.exe`               | `cargo build --release`（`bundle.resources` で同梱）      |
| スタートメニュー `lineage\fullos`                | [fullos/src-tauri/wix/main.wxs](../fullos/src-tauri/wix/main.wxs)   |
| スタートメニュー `lineage\Minos`                 | [fullos/src-tauri/wix/minos.wxs](../fullos/src-tauri/wix/minos.wxs) |

スタートメニューのショートカット名を `lineage` ではなく `fullos` にするため、Tauri の
WiX テンプレートを [fullos/src-tauri/wix/main.wxs](../fullos/src-tauri/wix/main.wxs) に
vendoring して `bundle.windows.wix.template` から読ませている。原本は
tauri-cli v2.11.4 の `crates/tauri-bundler/src/bundle/windows/msi/main.wxs` で、
変更点はショートカット名の 1 行だけ。**Tauri を上げたときはこのファイルも取り直すこと。**

インストーラ名とインストール先は `tauri.conf.json` の `productName`（= `lineage`）で決まる。
実行ファイル名だけは `mainBinaryName` で `fullos.exe` に固定してある。minos が同じ
ディレクトリの `fullos.exe` を名前決め打ちで起動するため
（[minos/src/infrastructure/system/launcher.rs](../minos/src/infrastructure/system/launcher.rs)）、
ここを変えると起動導線が壊れる。

minos.exe は
[fullos/src-tauri/tauri.conf.json](../fullos/src-tauri/tauri.conf.json) の
`build.beforeBuildCommand` でビルドされ（`cargo build --release --manifest-path ../minos/Cargo.toml`）、
`bundle.resources` でインストール先直下に入る。ショートカットだけは Tauri が
生成するコンポーネントから参照できないので、WiX フラグメントを 1 枚足して
`bundle.windows.wix.fragmentPaths` / `componentGroupRefs` から読ませている。
Tauri v2 の MSI は WiX v3 なので、フラグメントのスキーマは `wix/2006/wi`（v4/v5 ではない）。

1 本になったので、fullos の自動更新は minos も一緒に置き換える。
更新時に minos が起動したままだと MSI が使用中ファイルを差し替えられず、
サイレント適用では再起動待ちになる。更新前に minos を終了させておくのが確実。

## リリース手順

1. GitHub で `vX.Y.Z` タグのリリースを作成して publish する。
2. workflow がタグからバージョンを決め（`.github/scripts/set-version.mjs` が
   `minos/Cargo.toml` / `fullos/package.json` / `fullos/src-tauri/tauri.conf.json` /
   `fullos/src-tauri/Cargo.toml` を書き換える。コミットはしない）、MSI をビルドして
   同じリリースへアップロードする。
3. 失敗したジョブを直したら、`workflow_dispatch` にタグを渡して同じリリースへ再アップロードできる。

タグは `vX.Y.Z` 形式であること。MSI のバージョンと `latest.json` の `version` は
このタグから決まるので、タグとアプリのバージョン表記がずれることはない。

## 初回だけ必要な設定：updater の署名鍵

Tauri の updater は署名されていない更新を拒否する。鍵はリポジトリに入れず、
手元で生成して秘密鍵を GitHub Secrets に置く。

```sh
cd fullos
pnpm install
pnpm tauri signer generate -w ~/.tauri/lineage-updater.key
```

生成された内容を次の 3 か所に配る。

| 出力                        | 置き場所                                                                           |
| --------------------------- | ---------------------------------------------------------------------------------- |
| 公開鍵（`.key.pub` の中身） | `fullos/src-tauri/tauri.conf.json` の `plugins.updater.pubkey`                     |
| 秘密鍵（`.key` の中身）     | リポジトリ Secret `TAURI_SIGNING_PRIVATE_KEY`                                      |
| 生成時に入力したパスワード  | リポジトリ Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`（未設定なら空文字列で登録） |

`tauri.conf.json` の `pubkey` は設定済み。公開鍵なのでリポジトリに入れて構わない
（`.key.pub` の中身をそのまま貼る）。ここが未設定だと
`failed to decode base64 pubkey` でビルドが落ちる。

秘密鍵の方は環境変数 `TAURI_SIGNING_PRIVATE_KEY` で渡す。`tauri.conf.json` や
`.env` からは読まれないので、`tauri build` を実行するシェル自身に設定すること。

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content "$env:USERPROFILE\.tauri\lineage-updater.key" -Raw)
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""   # 鍵生成時にパスワードを付けたならその値
just msi
```

秘密鍵を失うと、既に配布済みのアプリは以後の更新を検証できなくなる。鍵は必ず保管すること。

## 自動更新の動作

- エンドポイント: `https://github.com/elda27/lineage/releases/latest/download/latest.json`
  （常に最新リリースへ解決される GitHub の固定 URL）
- 起動時に一度だけ黙ってチェックし、更新があれば画面上部にバーを出す
  （[fullos/src/updater/useUpdater.ts](../fullos/src/updater/useUpdater.ts)）。
- ユーザーが「更新する」を押すと MSI を取得・検証・適用し、`relaunch()` で再起動する。
- ブラウザでの `pnpm dev` には Tauri のランタイムが無いためチェックは失敗するが、
  起動時チェックは `silent` なので UI には出ない（コンソールにのみ記録）。

## ローカルでのパッケージング

```sh
just msi                                      # = 下の 2 行（Windows 専用レシピ）
cd fullos && pnpm install
cd fullos && pnpm tauri build --bundles msi   # minos も一緒にビルドされる
```

`justfile` は Windows / macOS / Linux で共通に動く（Windows では PowerShell を使う）。
`just bundle` はホスト OS の既定形式（Windows: `msi` / macOS: `dmg` / Linux: `deb,appimage`）で
バンドルする。`just msi` は Windows でのみ実行できる。

出力は `fullos/src-tauri/target/release/bundle/msi/lineage_<version>_x64_en-US.msi`。
バージョンは `tauri.conf.json` の `version` がそのまま入る（CI ではタグから上書きされる）。

ローカルで bundle する場合も `TAURI_SIGNING_PRIVATE_KEY` が必要。
署名なしで動作確認したいだけなら `tauri.conf.json` の `createUpdaterArtifacts` を
一時的に `false` にする。

MSI 以外のターゲット（`bundle.targets: "all"`）は Windows 以外では
`minos.exe` が無くて失敗する。Windows 以外で bundle する用事ができたら
`bundle.resources` をプラットフォームで分ける必要がある。
