# Runner / data CLI

現在の実行ファイルは `agentos`。#29で `lineage-runner` に改称予定。
FullOSの通常の読み書きはRunnerを起動せずTauri Rust内で実行する。

- `agentos --json notes --limit 50`: Note一覧（最大1000）
- `agentos --json get --note ID`: Note取得
- `agentos --json runs --limit 50`: 自動化実行履歴
- `agentos put --request-file -`: 標準入力JSONから本文を作成／更新
- `agentos apply --request-file -`: 既存typed mutation契約で状態・タグ・設定等を更新
- `agentos record --rule RULE --memo NOTE --result-file -`: 外部の自動化結果を登録

共通指定は `--db PATH --workspace WORKSPACE`。既定workspaceはlocal。
putのJSONは `{"title":"題名","bodyText":"本文"}`。更新時は `id` を追加する。
id指定時に対象workspaceでNoteが見つからなければ失敗する。本文・タグをコマンド引数へ入れない。
putは通常の編集なので呼び出すたびに来歴を追加する。通信再試行による無条件の再送は行わない。
applyはoperationIdによる既存の冪等性・revision契約に従う。

成功は終了コード0、入力・保存エラーは1。run/recordの自動化失敗は2。
取得・put・applyの出力はJSON。診断はstderr。
