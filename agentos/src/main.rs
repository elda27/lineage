//! agentos — 記録に基づく自動化を実行する CUI。
//!
//! docs/ui.md「agentos」に対応する。常駐せず、状態も持たない。DB とコマンド引数と
//! OS の資格情報ストアだけで動くので、タスクスケジューラからでも fullos からでも
//! シェルからでも、同じように1回ぶんの実行として呼べる。
//!
//! 画面は持たない（UI は fullos 側）。そのためブラウザ方式のルールはここでは実行できず、
//! `browser_backend_unsupported` として断る。
//!
//! 出力の約束:
//! - 機械が読む結果は stdout（`--json` で JSON、既定でも1行1件の素朴な形）
//! - 人が読む進行状況は stderr
//! - `run` / `record` は自動化が成功しなかったとき終了コード 2 を返す

use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use lineage_core::app::automation::{Automation, reject_browser_backend};
use lineage_core::domain::automation::{AutomationRule, AutomationRun, RunStatus};
use lineage_core::domain::ports::{AutomationRuleQuery, LineageQuery};
use lineage_core::infra::anthropic::AnthropicBackend;
use lineage_core::infra::clock::{SystemClock, UuidGenerator};
use lineage_core::infra::credentials::OsCredentialStore;
use lineage_core::infra::crypto::Sha256Hasher;
use lineage_core::infra::sqlite::Database;

/// minos と同じ既定 workspace。
const DEFAULT_WORKSPACE_ID: &str = "local";

/// 自動化が成功しなかったときの終了コード。実行そのものの失敗（1）と区別する。
const EXIT_NOT_SUCCEEDED: i32 = 2;

#[derive(Parser)]
#[command(name = "agentos", version, about = "記録に基づく自動化を実行する")]
struct Cli {
    #[command(flatten)]
    global: Global,

    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone)]
struct Global {
    /// 対象の DB。既定は minos と同じ `%LOCALAPPDATA%\minos\lineage.db`。
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    /// ワークスペース ID。
    #[arg(long, global = true, default_value = DEFAULT_WORKSPACE_ID)]
    workspace: String,

    /// 結果を JSON で出力する。
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// ルールを1件、指定の記録に対して実行する。
    Run {
        #[arg(long)]
        rule: String,
        #[arg(long)]
        memo: String,
    },
    /// 記録に対して実行できるルールを一覧する。
    Match {
        #[arg(long)]
        memo: String,
    },
    /// 送信するプロンプトを組み立てて出力する（ブラウザ方式の前段）。
    Render {
        #[arg(long)]
        rule: String,
        #[arg(long)]
        memo: String,
    },
    /// 外部で得た結果を確定する（ブラウザ方式の後段）。
    Record {
        #[arg(long)]
        rule: String,
        #[arg(long)]
        memo: String,
        /// 結果テキストのファイル。`-` で標準入力。
        #[arg(long, value_name = "PATH")]
        result_file: String,
    },
    /// メタ情報マッチのルールについて、未処理の記録をまとめて実行する。
    Poll {
        /// 1回で実行する上限。
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// 発火時刻が来たスケジュール実行のルールを走らせる。
    ScheduleDue,
    /// 定期起動からの1回ぶん（`poll` と `schedule-due` の両方）。
    ///
    /// タスクスケジューラに登録するのはこれ1本にする。2つ登録すると、
    /// 同じ記録を2つのプロセスが同時に拾う余地ができる。
    Tick {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// 登録されているルールを一覧する。
    Rules,
    /// hash-chain を検証する。
    Verify,
    /// API キーなどの資格情報を OS の資格情報ストアで扱う。
    #[command(subcommand)]
    Credential(CredentialCommand),
}

#[derive(Subcommand)]
enum CredentialCommand {
    /// 秘密を登録する。値は標準入力から読む。
    ///
    /// コマンドライン引数で渡さないのは、引数が他プロセスから見えるため。
    Set {
        #[arg(long)]
        provider: String,
    },
    /// 登録済みかどうかを返す。値そのものは出力しない。
    Has {
        #[arg(long)]
        provider: String,
    },
    /// 登録を削除する。
    Delete {
        #[arg(long)]
        provider: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match dispatch(&cli) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("agentos: {error:#}");
            std::process::exit(1);
        }
    }
}

fn dispatch(cli: &Cli) -> Result<i32> {
    // 資格情報は DB を触らないので、DB を開く前に処理する。
    // minos を一度も起動していない環境でも鍵の登録だけはできてほしい。
    if let Command::Credential(command) = &cli.command {
        return run_credential(command, cli.global.json);
    }

    let session = Session::open(&cli.global)?;
    match &cli.command {
        Command::Run { rule, memo } => session.run(rule, memo),
        Command::Match { memo } => session.match_rules(memo),
        Command::Render { rule, memo } => session.render(rule, memo),
        Command::Record {
            rule,
            memo,
            result_file,
        } => session.record(rule, memo, result_file),
        Command::Poll { limit } => session.poll(*limit),
        Command::ScheduleDue => session.schedule_due(),
        Command::Tick { limit } => {
            let polled = session.poll(*limit)?;
            let due = session.schedule_due()?;
            Ok(polled.max(due))
        }
        Command::Rules => session.rules(),
        Command::Verify => session.verify(),
        // 上で処理済み。
        Command::Credential(_) => unreachable!(),
    }
}

fn run_credential(command: &CredentialCommand, json: bool) -> Result<i32> {
    match command {
        CredentialCommand::Set { provider } => {
            let mut secret = String::new();
            std::io::stdin()
                .read_to_string(&mut secret)
                .context("標準入力から秘密を読めません")?;
            // 貼り付けたときの改行は取り除く。鍵の前後に空白が入る事故が多い。
            OsCredentialStore::set(provider, secret.trim())?;
            Ok(0)
        }
        CredentialCommand::Has { provider } => {
            let registered = OsCredentialStore::has(provider)?;
            if json {
                println!("{}", serde_json::json!({ "registered": registered }));
            } else {
                println!("{registered}");
            }
            Ok(0)
        }
        CredentialCommand::Delete { provider } => {
            OsCredentialStore::delete(provider)?;
            Ok(0)
        }
    }
}

/// 1回の起動ぶんの組み立て（composition root）。
struct Session {
    database: Database,
    clock: SystemClock,
    ids: UuidGenerator,
    hasher: Sha256Hasher,
    credentials: OsCredentialStore,
    workspace: String,
    json: bool,
}

impl Session {
    fn open(global: &Global) -> Result<Self> {
        let database = match &global.db {
            Some(path) => Database::open(path)
                .with_context(|| format!("DB を開けません: {}", path.display()))?,
            None => Database::open_default()?,
        };
        Ok(Self {
            database,
            clock: SystemClock,
            ids: UuidGenerator,
            hasher: Sha256Hasher,
            credentials: OsCredentialStore,
            workspace: global.workspace.clone(),
            json: global.json,
        })
    }

    fn automation(&self) -> Automation<'_> {
        Automation {
            rules: &self.database,
            runs: &self.database,
            memos: &self.database,
            store: &self.database,
            clock: &self.clock,
            ids: &self.ids,
            hasher: &self.hasher,
        }
    }

    fn run(&self, rule_id: &str, memo_id: &str) -> Result<i32> {
        let rule = self.require_rule(rule_id)?;
        reject_browser_backend(&rule)?;

        let backend = AnthropicBackend::new(&self.credentials);
        let run = self.automation().run(&self.workspace, rule_id, memo_id, &backend)?;
        self.report_run(&run)?;
        Ok(exit_code_for(&run))
    }

    fn match_rules(&self, memo_id: &str) -> Result<i32> {
        let rules = self.automation().matching_rules(&self.workspace, memo_id)?;
        if self.json {
            self.print_json(&rules)?;
        } else {
            for rule in &rules {
                println!("{}\t{}\t{}", rule.id, rule.backend.as_str(), rule.name);
            }
        }
        Ok(0)
    }

    fn render(&self, rule_id: &str, memo_id: &str) -> Result<i32> {
        let prompt = self.automation().prompt(&self.workspace, rule_id, memo_id)?;
        if self.json {
            self.print_json(&serde_json::json!({ "prompt": prompt }))?;
        } else {
            // プロンプトは複数行なので、そのまま出す（パイプで受け取れるように）。
            print!("{prompt}");
            std::io::stdout().flush()?;
        }
        Ok(0)
    }

    fn record(&self, rule_id: &str, memo_id: &str, result_file: &str) -> Result<i32> {
        let text = read_result(result_file)?;
        let run = self
            .automation()
            .record(&self.workspace, rule_id, memo_id, &text)?;
        self.report_run(&run)?;
        Ok(exit_code_for(&run))
    }

    fn poll(&self, limit: usize) -> Result<i32> {
        let automation = self.automation();
        let backend = AnthropicBackend::new(&self.credentials);
        let mut runs = Vec::new();

        for rule in automation.meta_match_rules(&self.workspace)? {
            if runs.len() >= limit {
                break;
            }
            // ブラウザ方式は WebView が要るので、ここでは黙って飛ばす。
            // 「実行できない」と毎回エラーを出すと、poll を定期実行できなくなる。
            if reject_browser_backend(&rule).is_err() {
                eprintln!(
                    "ルール `{}` はブラウザ方式のため agentos では実行しません",
                    rule.name
                );
                continue;
            }
            for memo in automation.pending(&self.workspace, &rule)? {
                if runs.len() >= limit {
                    break;
                }
                eprintln!("実行: {} ← {}", rule.name, memo.title);
                runs.push(automation.run_rule(&rule, &memo, &backend)?);
            }
        }

        self.report_runs(&runs)
    }

    fn schedule_due(&self) -> Result<i32> {
        let automation = self.automation();
        let backend = AnthropicBackend::new(&self.credentials);
        let mut runs = Vec::new();

        for rule in automation.due_schedules(&self.workspace)? {
            if reject_browser_backend(&rule).is_err() {
                eprintln!(
                    "ルール `{}` はブラウザ方式のため agentos では実行しません",
                    rule.name
                );
                continue;
            }
            // スケジュール実行も対象は「未処理の記録」。時刻が来ただけで
            // 同じ記録を何度も処理しないよう、poll と同じ絞り込みを通す。
            for memo in automation.pending(&self.workspace, &rule)? {
                eprintln!("実行: {} ← {}", rule.name, memo.title);
                runs.push(automation.run_rule(&rule, &memo, &backend)?);
            }
        }

        self.report_runs(&runs)
    }

    fn rules(&self) -> Result<i32> {
        let rules = self.database.all(&self.workspace)?;
        if self.json {
            self.print_json(&rules)?;
        } else {
            for rule in &rules {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    rule.id,
                    if rule.enabled { "有効" } else { "停止中" },
                    rule.trigger_kind.as_str(),
                    rule.backend.as_str(),
                    rule.name,
                );
            }
        }
        Ok(0)
    }

    fn verify(&self) -> Result<i32> {
        let records = self.database.list(&self.workspace)?;
        let result = lineage_core::domain::lineage::LineageLedger::new(&self.hasher).verify(&records);
        if self.json {
            self.print_json(&serde_json::json!({
                "ok": result.is_ok(),
                "checked": records.len(),
                "detail": format!("{result:?}"),
            }))?;
        } else {
            println!("{result:?}");
        }
        Ok(if result.is_ok() { 0 } else { EXIT_NOT_SUCCEEDED })
    }

    fn require_rule(&self, rule_id: &str) -> Result<AutomationRule> {
        self.database
            .get(rule_id)?
            .with_context(|| format!("自動化ルールが見つかりません: {rule_id}"))
    }

    fn report_run(&self, run: &AutomationRun) -> Result<()> {
        if self.json {
            self.print_json(run)?;
        } else {
            println!("{}\t{}", run.status.as_str(), run.id);
            if let Some(error) = &run.error {
                eprintln!("{error}");
            }
        }
        Ok(())
    }

    /// まとめ実行の結果。実行0件でも「何もすることが無かった」は失敗ではない。
    fn report_runs(&self, runs: &[AutomationRun]) -> Result<i32> {
        if self.json {
            self.print_json(runs)?;
        } else {
            for run in runs {
                println!("{}\t{}\t{}", run.status.as_str(), run.rule_id, run.id);
            }
        }
        let failed = runs
            .iter()
            .filter(|run| run.status != RunStatus::Succeeded)
            .count();
        if failed > 0 {
            eprintln!("{failed} 件が成功しませんでした（{} 件中）", runs.len());
        }
        Ok(0)
    }

    fn print_json<T: serde::Serialize + ?Sized>(&self, value: &T) -> Result<()> {
        println!("{}", serde_json::to_string_pretty(value)?);
        Ok(())
    }
}

/// `-` なら標準入力、それ以外はファイルから結果テキストを読む。
fn read_result(path: &str) -> Result<String> {
    if path == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .context("標準入力を読めません")?;
        return Ok(text);
    }
    std::fs::read_to_string(path).with_context(|| format!("結果ファイルを読めません: {path}"))
}

fn exit_code_for(run: &AutomationRun) -> i32 {
    if run.status == RunStatus::Succeeded {
        0
    } else {
        EXIT_NOT_SUCCEEDED
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_line_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_failed_run_exits_with_its_own_code() {
        let run = |status| AutomationRun {
            id: "run-1".into(),
            workspace_id: "ws".into(),
            rule_id: "rule-1".into(),
            source_document_id: "doc-1".into(),
            result_document_id: None,
            status,
            backend: lineage_core::domain::automation::BackendKind::ApiKey,
            error: None,
            started_at: "2026-08-13T09:00:00Z".into(),
            finished_at: None,
        };

        assert_eq!(exit_code_for(&run(RunStatus::Succeeded)), 0);
        assert_eq!(exit_code_for(&run(RunStatus::Failed)), EXIT_NOT_SUCCEEDED);
        // 拒否も「成功していない」ので、スクリプトからは失敗として見える。
        assert_eq!(exit_code_for(&run(RunStatus::Refused)), EXIT_NOT_SUCCEEDED);
    }

    #[test]
    fn a_missing_result_file_is_reported() {
        assert!(read_result("存在しないファイル.txt").is_err());
    }
}
