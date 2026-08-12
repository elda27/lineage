# リリースと自動更新

Windows 向け配布は GitHub Release を単一の配布元とする。
リリースを publish すると `.github/workflows/release.yml` が走り、インストーラを
そのリリースに添付する。fullos はそのリリースの `latest.json` を見て自己更新する。

| 成果物                               | 中身                  | 生成元                                           |
| ------------------------------------ | --------------------- | ------------------------------------------------ |
| `minos_<version>_x64_en-US.msi`      | minos のインストーラ  | `wix build minos/wix/minos.wxs`                  |
| `fullos_<version>_x64_en-US.msi`     | fullos のインストーラ | `tauri build --bundles msi`（Tauri も WiX 経由） |
| `fullos_<version>_x64_en-US.msi.sig` | 上記の minisign 署名  | Tauri updater                                    |
| `latest.json`                        | 更新マニフェスト      | Tauri updater                                    |

## リリース手順

1. GitHub で `vX.Y.Z` タグのリリースを作成して publish する。
2. workflow がタグからバージョンを決め（`.github/scripts/set-version.mjs` が
   `minos/Cargo.toml` / `fullos/package.json` / `fullos/src-tauri/tauri.conf.json` /
   `fullos/src-tauri/Cargo.toml` を書き換える。コミットはしない）、両方をビルドして
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

`tauri.conf.json` の `pubkey` は現在 `REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY` の
プレースホルダになっている。**ここを実際の公開鍵に置き換えるまで fullos のビルドは
失敗する**（`createUpdaterArtifacts` が有効なため署名鍵を要求する）。

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
just msi 0.1.0                        # minos の MSI
cd fullos && pnpm tauri build --bundles msi   # fullos の MSI
```

fullos をローカルで bundle する場合も `TAURI_SIGNING_PRIVATE_KEY` が必要。
署名なしで動作確認したいだけなら `tauri.conf.json` の `createUpdaterArtifacts` を
一時的に `false` にする。
