//! 実行環境ごとに使えるバックエンドの線引き。

use anyhow::{Result, bail};

use crate::domain::automation::{AutomationRule, BackendKind};

/// ブラウザ方式のルールを、ブラウザを持たない実行環境で走らせようとしたときのエラー。
///
/// agentos は WebView を持たないので実行できない。呼び出し側が「fullos で実行してください」
/// と案内できるよう、通信失敗などとは別の文言にしておく。
pub fn reject_browser_backend(rule: &AutomationRule) -> Result<()> {
    if rule.backend == BackendKind::Browser {
        bail!(
            "browser_backend_unsupported: ルール `{}` はブラウザ方式です。fullos の自動化画面から実行してください",
            rule.name
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::automation::{Trigger, TriggerKind};
    use crate::features::automation::test_support::rule;

    #[test]
    fn a_browser_rule_is_rejected_where_there_is_no_webview() {
        let mut r = rule("rule-1", TriggerKind::Manual, Trigger::default());
        r.backend = BackendKind::Browser;
        let error = reject_browser_backend(&r).unwrap_err();
        assert!(format!("{error}").contains("browser_backend_unsupported"));

        r.backend = BackendKind::ApiKey;
        assert!(reject_browser_backend(&r).is_ok());
    }
}
