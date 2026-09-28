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

## GitHub Issues

GitHub tokenを `credential set --provider github` の標準入力へ登録する。対象repositoryのIssues権限（取得はread、作成・更新はwrite）が必要。

- `github issue get --repo OWNER/REPO --number 1`
- `github issue import --repo OWNER/REPO --number 1`: Noteへ取り込み。同一Issue/同一workspaceは同じNote ID。
- `github issue import --repo OWNER/REPO --number 1 --overwrite`: 差分を確認したうえで既存Noteを置換。
- `github issue create --repo OWNER/REPO --request-file -`
- `github issue update --repo OWNER/REPO --number 1 --request-file -`

作成JSONは `{"title":"題名","body":"本文"}`。更新ではtitle/body/stateの必要な項目のみ指定。stateはopen/closed。API応答とURLをJSON出力する。
再取り込みで内容が一致すれば来歴も増やさない。異なる内容は既定で拒否し、ローカル編集を保持する。取り込み元URLとGitHub更新時刻を来歴へ記録する。
書き込みの通信失敗は結果不明の可能性があるため自動再送しない。GitHub側の状態を確認してから再実行する。PRはIssueとして取り込まない。GitHub固有実装はRunner内に限定する。

API契約: https://docs.github.com/en/rest/issues/issues

## GitHub Actions

Actions write/read権限のあるGitHub資格情報を利用する。Workflowはworkflow_dispatchに対応させる。

`github actions dispatch --request-file request.json` の入力:

```json
{"execution_id":"unique-job-001","workspace":"local","note":"SOURCE_NOTE_ID","repo":"OWNER/REPO","workflow":"work.yml","git_ref":"main","inputs":{}}
```

inputsには秘密を入れず、Workflow側のGitHub Secretsを使用する。要求はRunnerの実行記録として保存される。

- `github actions status --execution-id unique-job-001`: 正確なrun IDで状態を取得し、完了時に結果Noteと来歴を登録。
- `github actions watch --execution-id unique-job-001 --timeout-seconds 600`: 5秒間隔で確認。タイムアウト後も再開可能。
- `github actions record --execution-id unique-job-001 --result-file output.txt`: 完了済み実行に成果物のテキストを追加登録。同じ内容は同じNoteとなる。

実行記録はNote DBと同じディレクトリの `lineage.github.sqlite`（--dbの拡張子をgithub.sqliteへ変更）でRunnerだけが管理する。再起動してもrun ID、元Noteの内容fingerprint、実行条件を維持する。このファイルもバックアップ対象とする。
同じexecution_idの同じ要求は再dispatchせず既存結果を返す。異なる要求への再利用は拒否する。通信失敗やrun ID保存前の停止では結果不明として再dispatchしない。GitHubで実行状態を確認してから別execution_idで明示的に再実行する。

成功・失敗・キャンセル等のconclusionはそのまま記録し、失敗を成功に変換しない。CLI自体の成功（終了コード0）はAPI操作・状態保存が成功したことを表すため、呼び出し側はconclusionを確認する。
Workflowのバイナリartifactの自動ダウンロードは行わない。必要な出力をテキストとしてrecordへ渡す。FullOS・WebViewは不要。

API契約: https://docs.github.com/en/rest/actions/workflows?apiVersion=2022-11-28
`return_run_details: true` のworkflow_run_idを利用し、「最新のrun」から推測しない。

ヘッドレス環境では `LINEAGE_GITHUB_TOKEN` 環境変数で明示的に資格情報を設定できる。設定済みで空・不正な場合は失敗し、OS資格情報へフォールバックしない。未設定時のみOS資格情報を使用する。
