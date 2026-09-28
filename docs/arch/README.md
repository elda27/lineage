# Lineage Architecture

この文書は採用したarchitectureの設計方針と実装状況を区別して示すliving documentである。判断理由と代替案は
[`adr/README.md`](./adr/README.md) と各 ADR を参照する。2026-08-20 の Design Doc は検討時点の
snapshot であり、長期的な規範は accepted ADR を正本とする。

## 実装状況（2026-09-28、Issue #52）

#50のアプリ固有処理分離に続き、共有ドメインと保存処理をcrate境界で分離した。

- `lineage-core/src/domain`: 共有記録・タグ・mutationのモデルと検証、補完順位、hash-chainの純粋な規則。
- `lineage-store/src/features`: SaveDocument、CommitAutomationResult、ApplyMutation、VerifyLineageのI/O調整。
- `lineage-store/src/ports`: repository、transaction、clock、ID取得の契約。
- `lineage-store/src/infra`: SQLite、時計、UUID、SHA-256の実装。保存処理は各アプリへ複製しない。
- Minos: 入力・設定・foreground context、補完候補の取得、カーソル・タグ入力確定、添付ファイル管理。
- Runner: 自動化の条件評価・prompt・cron・推論・資格情報・実行制御。

依存はアプリ → lineage-store → lineage-core。アプリはドメイン型をcoreから直接利用できる。
coreにはfeatures/infraもrepository portも置かない。純粋なhash計算の契約Hasherはcoreに残す。
coreの通常依存はanyhow/serde/serde_jsonのみ。DB・GUI・HTTP・OS SDKなしでドメインを検証できる。

記録・タグ・添付・lineage、また結果・lineage・runの一括確定と失敗時のrollbackは維持する。
DB schema、保存先、CLI/JSON契約、バイナリ名は変更しない。
FullOSは引き続きRunner経由で書き込みを行う。

FullOSのMemo→Note用語統一、theme/capability/content/sync/Git/LFS/publication、migration刷新は未完了。
親Issue #22は継続する。

### 検証

- `python .github/scripts/check-boundaries.py`
- `cargo test -p lineage-core -p lineage-store -p agentos --locked`
- `cargo test -p minos --no-default-features --lib --locked`
- Windows: `cargo check -p minos --all-targets --locked`
- `cd fullos && pnpm build`

## Components

```text
Minos        FullOS        AgentOS
  \            |               /
   \           |              /
        lineage-core
        (Shared Kernel)
```

- **Minos**: keyboard shortcut から高速に Note を入力し、入力時の local context を付与する desktop app。
- **FullOS**: Note の検索、編集、整理、設定、automation 操作を提供する Tauri/React app。
- **AgentOS**: rule-based / agentic automation を実行する headless runtime。
- **lineage-core**: canonical record、workspace identity、mutation、Lineage/provenance integrity など、
  複数 app で同じ意味と不変条件を共有する必要がある contract を持つ Shared Kernel。

## Code boundaries

各 app/kernel は必要に応じて次の top-level responsibility を持つ。

```text
domain <- features <- composition root -> infra
```

- `domain`: identity、不変条件、状態遷移、port
- `features`: user/external interaction を成立させる use case
- `infra`: SQLite、filesystem、HTTP、OS API、keyring、clock 等の implementation
- composition root: policy/build capability に基づく dependency construction

`app` という directory 名は routing、provider、startup 等の composition shell に限定する。
use case layer は `features` と呼ぶ。feature 内は原則 flat に保ち、規模が必要とするまで形式的な
`usecase/` / `service/` hierarchy を作らない。

詳細: [ADR-0005](./adr/0005-place-domain-features-infra-behind-composition-roots.md)

共通UIとfeature固有UIの配置基準は [components/README.md](../../fullos/src/components/README.md) を参照する。
タグ表示・検索バーは `components/containers`、タグ補完hookは `shared/hooks` に配置し、
共通側からfeature実装へ依存させない。

## Domain language and Shared Kernel

利用者向け概念は `Note` とする。Minos と FullOS は用途の異なる app-local Note model を持ち、
canonical `DocumentAsset` へ明示的に map する。foreground context、title derivation、画面状態など
concrete app の知識を `lineage-core` に置かない。`capture` は Minos の入力 use case 名としてのみ
使用でき、Shared Kernel の entity 名にはしない。

詳細: [ADR-0006](./adr/0006-keep-a-small-shared-kernel-and-app-local-note-models.md)

## FullOS frontend

```text
fullos/src/
  app/
  pages/
  components/
    base/
    containers/
  features/
  domain/
  infra/
```

route-level screen は `pages`、domain-neutral primitive は `components/base`、cross-page composite は
`components/containers`、feature-local behavior/UI は `features` が所有する。component/page styling
は Tailwind utility と semantic theme token に統一し、global CSS は theme/base/keyframe に限定する。

詳細: [ADR-0007](./adr/0007-structure-fullos-ui-with-pages-components-and-tailwind.md)

## Infrastructure and trust boundaries

infra は persistence、inference、credentials、auth、update 等の用途を第一軸にし、各用途内で
local/remote trust boundary を明示する。local は user data を端末外へ送信しない implementation を
意味する。remote inference、remote persistence、cloud sync、control-plane communication は独立した
compile-time capability と runtime policy を持つ。選択した provider の失敗時に別 trust boundary へ
自動 fallback しない。

詳細: [ADR-0008](./adr/0008-separate-infrastructure-by-capability-and-trust-boundary.md)

## Persistence and synchronization

永続化は次の三責務を別 contract とする。

1. **Structured State**: local SQLite を source/projection とする metadata、revision、run state 等
2. **Content**: `ContentStore` が扱う text、image、PDF、artifact、attachment
3. **Sync**: versioned domain mutation、outbox、cursor、delivery state

physical path/provider URL は domain identity にしない。local/server は同一 DB schema を要求せず、
DB file/row ではなく typed mutation を同期する。sync-enabled workspace の canonical revision は
server が採番し、local state は offline-capable projection とする。

詳細:
[ADR-0009](./adr/0009-separate-structured-state-content-and-sync-persistence.md)、
[ADR-0011](./adr/0011-sync-versioned-domain-mutations-not-databases.md)

## Git and Git LFS

Git / Git LFS は workspace 単位の versioned content backend として使用する。automation definition、
prompt、template、managed script、revision に価値がある text を対象とし、SQLite、secret、model
cache、delivery state は格納しない。Git remote は application sync、authorization、tenant fan-out の
代替にしない。

詳細: [ADR-0010](./adr/0010-use-git-and-lfs-only-as-versioned-content-backends.md)

## Multi-tenant publication

canonical source asset は一つの owner と revision を持つ。別 tenant へは copy ではなく Publication
reference と access scope を作成し、authoritative server commit 後に per-target delivery state を持つ
fan-out worker が処理する。source access と result-only access を区別し、source、automation、
artifact、target の provenance を保持する。

詳細: [ADR-0012](./adr/0012-publish-canonical-assets-with-server-side-fan-out.md)

## Active decisions

有効な decision の完全な一覧は [ADR index](./adr/README.md#5-現在有効な-adrstatus-accepted) を参照する。


## 2026-09-28: #54 責務境界の見直し

FullOSの通常の読み書きはTauriのRust内で完結する。plugin-sql/sqlxを廃止し、SQLite実装をrusqliteへ統一する。自動化実行はRunnerが所有する。外部ブラウザ応答のrender/record経路は既存の手動結果入力として残る。

lineage-storeの独立性を再評価した結果、Minosの記録保存、Runnerの結果保存、FullOS/CLIの差分更新で必要なSQLite実装・port・原子的な保存プリミティブのみを共有する。アプリへの同一SQLite/hash-chain実装の複製を避ける実際の利用があるため、この範囲でcrateを維持する。Automation結果の検証と完了処理CommitAutomationResultはRunnerへ移動した。coreにはI/Oを戻さない。

Runnerのget/notes/runsはJSONで外部から参照でき、applyは既存のtyped mutation JSONを標準入力またはファイルから受け取る。recordは外部結果登録の入口。内部から自身のCLIを再起動しない。改称は#29で別途扱う。
