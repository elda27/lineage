//! 値オブジェクトと、時刻・ID・ハッシュのような「外界」を抽象化したトレイト。

use std::collections::BTreeMap;

use serde_json::Value;

/// SHA-256 を計算する。実装は infrastructure 側（WebCrypto 相当）。
pub trait Hasher {
    fn sha256_hex(&self, input: &str) -> String;
}

/// 現在時刻。テストから差し替えられるようにトレイトにする。
pub trait Clock {
    /// RFC3339 (UTC) 文字列を返す。DB にはこの形式で保存する。
    fn now_rfc3339(&self) -> String;
}

/// 一意な ID を生成する。
pub trait IdGenerator {
    fn new_id(&self) -> String;
}

/// ハッシュ対象の正規化。
///
/// キー順を固定した JSON にすることで、同じ内容なら常に同じ文字列になる。
/// `BTreeMap` はキー昇順で走査されるため、`serde_json` の出力もキー昇順で確定する。
/// ローカル・クラウドで**必ず同じ実装**を使うこと（分岐させない）。
pub fn canonicalize(fields: &BTreeMap<&'static str, Value>) -> String {
    serde_json::to_string(fields).expect("canonicalize: JSON serialization must not fail")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_is_key_order_independent() {
        let mut a = BTreeMap::new();
        a.insert("b", Value::from(2));
        a.insert("a", Value::from(1));

        let mut b = BTreeMap::new();
        b.insert("a", Value::from(1));
        b.insert("b", Value::from(2));

        assert_eq!(canonicalize(&a), canonicalize(&b));
        assert_eq!(canonicalize(&a), r#"{"a":1,"b":2}"#);
    }
}
