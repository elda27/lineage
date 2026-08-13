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
| `HKLM\...\CurrentVersion\Run` の `lineage Minos` | [fullos/src-tauri/wix/minos.wxs](../fullos/src-tauri/wix/minos.wxs) |

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
`bundle.resources` でインストール先直下に入る。ショートカットと自動起動だけは Tauri が
生成するコンポーネントから参照できないので、WiX フラグメントを 1 枚足して
`bundle.windows.wix.fragmentPaths` / `componentGroupRefs` から読ませている。
Tauri v2 の MSI は WiX v3 なので、フラグメントのスキーマは `wix/2006/wi`（v4/v5 ではない）。

## minos の自動起動

minos はトレイ常駐 + Alt+Space のアプリなので、起動していないと呼び出せない。
そのためインストーラは2か所で minos を起動する（どちらも `MinosAutoStart`
コンポーネントグループ、[fullos/src-tauri/wix/minos.wxs](../fullos/src-tauri/wix/minos.wxs)）。

| いつ                 | 仕掛け                                                                                  |
| -------------------- | --------------------------------------------------------------------------------------- |
| 次回以降のログオン   | `HKLM\Software\Microsoft\Windows\CurrentVersion\Run` の `lineage Minos`（`MinosRunAtLogon`） |
| インストールした直後 | `InstallFinalize` の後に走るカスタムアクション `LaunchMinos`                             |

どちらも `"%ProgramFiles%\lineage\minos.exe" --autostart` を起動する。
アンインストールすると Run 値はコンポーネントごと消える。

`LaunchMinos` は `InstallExecuteSequence` に置いてあるので、UI 付きのインストールでも
更新時のサイレント実行でも走る（Tauri が fullos 用に持っている `LaunchApplication` は
ExitDialog のチェックボックス経由なので UI 付きのときしか走らない）。即時実行 +
`Impersonate="yes"` なので、昇格した SYSTEM ではなくインストールを始めた利用者の
セッションで起動する。条件の `NOT Installed` は新規インストールと更新（メジャー
アップグレードの新パッケージ側）で真、アンインストールと修復では偽になる。

### `--autostart`

自動起動は利用者が呼んだわけではないので、`--autostart` 付きで起動した minos は
入力画面を出さずトレイに入るだけになる（[minos/src/main.rs](../minos/src/main.rs)）。
付け忘れると、ログオンのたびに、またインストーラの最終画面の上に入力画面が現れる。
すでに常駐しているところへ `--autostart` 付きの起動が重なった場合は、既存のウィンドウを
出さずに黙って終了する（引数なしの起動、つまりショートカットからの起動では既存の
ウィンドウを前に出す）。

多重起動そのものは minos 側の名前付き Mutex で抑止されるので、ログオン起動と
ショートカットを併用してもプロセスは増えない
（[minos/src/infrastructure/system/single_instance.rs](../minos/src/infrastructure/system/single_instance.rs)）。

`--autostart` のときはウィンドウを作るだけで一度も表示しない（gpui の
`WindowOptions.show = false`）。作ってから隠す作りにすると、gpui が最初の描画を
終えるまでの数十〜数百 ms のあいだ画面の左上にウィンドウが見えてしまう
（ログオン直後ほど長い）。その代わり gpui は `WindowOptions` の大きさを適用しないので、
起動時に隠したまま `resize` で合わせている。

Run 値は**インストール時に書かれる**ので、`--autostart` が付くのは新しい MSI で
インストールまたは更新したあとから。それより前に入れた環境では引数なしの Run 値が
残っており、ログオンのたびに入力画面が出る。`reg query "HKLM\Software\Microsoft\Windows\CurrentVersion\Run" /v "lineage Minos"`
で確かめられる。

### フラグメントでは handlebars の変数が使えない

`fragmentPaths` のファイルは handlebars に **パースはされるが展開はされない**。
二重波括弧の記法を書くと構文としては検証され、壊れているとビルドが落ちる。
一方で main.wxs で使える `{{manufacturer}}` / `{{product_name}}` などの値は解決されず、
書いた文字列がそのまま MSI に入る。

実際 minos.wxs で `Key="Software\{{manufacturer}}\{{product_name}}"` と書いていたため、
インストールすると literal な `HKCU\Software\{{manufacturer}}\{{product_name}}` が
作られていた。現在は実値（`Software\lineage\lineage`）を直接書いてある。
`productName` / `identifier` を変えたらこのファイルも手で追随させること。

`manufacturer` はどこにも書いていない。tauri-utils の定義どおり `identifier`
（`com.lineage.fullos`）の 2 番目の要素から導出されるので `lineage` になる。
明示したい場合は `bundle.publisher` を設定する（レジストリキーのパスも変わる）。

Run 値を HKCU ではなく HKLM に置いているのは、この MSI が `InstallScope="perMachine"` で、
レジストリの書き込みを昇格した deferred アクション（= SYSTEM）が行うため。HKCU だと
インストールしたユーザのハイブに落ちる保証がない。代わりにこのマシンへログオンする
全ユーザで minos が起動する。perUser インストールに変えるなら HKCU へ移すこと。

なお XML のコメント内にはハイフン 2 個を書けないため、`minos.wxs` のコメントでは
引数名を裸で書いてある（属性値の側は普通に書ける）。

## 自動更新と minos

1 本になったので、fullos の自動更新は minos も一緒に置き換える。
更新時に minos が起動したままだと MSI が使用中ファイルを差し替えられず、
サイレント適用では再起動待ちになる。自動起動で常駐しているぶん残りやすいので、
更新前に minos を終了させておくのが確実。

終了した minos は更新の最後に `LaunchMinos` が入れ直す（メジャーアップグレードの
新パッケージ側では `NOT Installed` が真になる）。fullos が `relaunch()` で戻るのと同じく、
更新のたびに常駐が途切れたままにはならない。

## リリース手順

アプリのバージョンはリポジトリ直下の [`VERSION`](../VERSION) で中央管理する。
変更時は `just version-set X.Y.Z` を実行すると、`package.json`、両方の
`Cargo.toml`、`tauri.conf.json` に同じ値が反映される。整合性だけを確認する場合は
`just version-check` を使う。リリースタグ `vX.Y.Z` は `VERSION` と一致していなければならない。

1. GitHub で `vX.Y.Z` タグのリリースを作成して publish する。
2. workflow がタグ、`VERSION`、各マニフェストのバージョンが一致することを
   `.github/scripts/set-version.mjs --check` で検査し、MSI をビルドして同じリリースへ
   アップロードする。
3. 失敗したジョブを直したら、`workflow_dispatch` にタグを渡して同じリリースへ再アップロードできる。

タグは `vX.Y.Z` 形式であること。MSI のバージョンと `latest.json` の `version` は
`VERSION` から決まる。タグと一致しない場合はビルド前に workflow が失敗する。

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
