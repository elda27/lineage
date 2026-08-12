# Task runner entry points (Windows / macOS / Linux 共通)。
#
# Windows でも sh を前提にしないため、シェルは PowerShell に切り替える。
# レシピ本体はシェル組み込み（cd, &&, 変数展開など）を使わず外部コマンドの
# 呼び出しだけで書いてあるので、sh でも PowerShell でも同じ挙動になる。
# パスは常に / 区切りで書く（PowerShell も cmd も受け付ける）。
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# ホスト OS の既定のバンドル形式。just が評価するのでシェルには依存しない。
bundles := if os() == "windows" { "msi" } else if os() == "macos" { "dmg" } else { "deb,appimage" }

[private]
default:
    @just --list

# fullos の依存を入れる。
install:
    pnpm --dir ./fullos install

# fullos を開発モードで起動する（Tauri + vite）。
dev: install
    pnpm --dir ./fullos run tauri dev

# minos 単体をリリースビルドする。bundle 時は beforeBuildCommand が同じことをする。
build-minos:
    cargo build --release --manifest-path ./minos/Cargo.toml

# VERSION を唯一の入力として、各パッケージのバージョン表記を同期する。
[doc("VERSION の値を各マニフェストへ反映する")]
version-sync:
    node .github/scripts/set-version.mjs

[doc("各マニフェストのバージョンが VERSION と一致するか検査する")]
version-check:
    node .github/scripts/set-version.mjs --check

[doc("リリースバージョンを変更し、各マニフェストへ反映する")]
version-set version:
    node .github/scripts/set-version.mjs --set {{ version }}

[private]
bundle-with target: install
    pnpm --dir ./fullos run tauri build --bundles {{ target }}

# ホスト OS 向けのインストーラを作る（Windows: msi / macOS: dmg / Linux: deb,appimage）。
#
# 注意: 現状 tauri.conf.json の bundle.resources が minos.exe 固定なので、
# Windows 以外では minos のバンドルに失敗する。docs/release.md 参照。
[doc("ホスト OS 向けのインストーラを作る（win: msi / mac: dmg / linux: deb,appimage）")]
bundle: (bundle-with bundles)

# Build fullos + minos and package them into a single Windows MSI.
#
# minos.exe は fullos の tauri.conf.json の beforeBuildCommand でビルドされ、
# bundle.resources 経由で同じ MSI に入る。WiX は Tauri のバンドラが
# %LOCALAPPDATA%\tauri へ自動で用意するので手動インストールは不要。
#
# 出力: fullos/src-tauri/target/release/bundle/msi/lineage_<version>_x64_en-US.msi
# 署名鍵 TAURI_SIGNING_PRIVATE_KEY が無いと createUpdaterArtifacts の署名で失敗する。
[doc("fullos + minos を 1 本の Windows MSI にまとめる")]
[windows]
msi: (bundle-with "msi")
