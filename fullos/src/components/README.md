# 共通UIの配置

配置は元のフォルダや呼び出し元の数ではなく、部品が所有する責務で決める。

| 配置 | 責務 | 例 |
| --- | --- | --- |
| `components/base` | ドメインに依存しないUI primitive | Icon、Toggle、共通styles |
| `components/containers` | 機能をまたぐ共通表示・入力。特定featureの操作や状態管理を所有しない | SettingRow、MetaChips、SearchBox |
| `shared/hooks` | 共通表示・入力を支えるhook。feature実装へ依存しない | useMetaCompletion、useMetaSuggestions |
| `features/<feature>/components` | 当該featureの業務操作や表示モデルに結びついたUI | MemoCard、MemoDetail、RuleEditor、AutomationSettings |

タグ表示・補完はメモ編集や検索で共用するため、memo featureの所有物にしない。
共通部品は `core/domain` の型・表示関数と `shared/api` を利用できるが、
`features/*` のserviceやcomponentには依存しない。`base`はdomainにも依存しない。

他機能から呼ばれることだけを理由に共通部品へ移さない。たとえばActionMenuは
自動化ルールの候補取得・実行・実行結果を所有するため、automation featureに置く。
AccountButton / StorageMeterはworkspaceの情報表示、AgentSkillDialogはskill導入の操作として
各featureに置く。外見が共通化できても、これらの業務処理を共通UI層へ移さない。
