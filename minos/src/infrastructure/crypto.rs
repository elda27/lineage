//! `Hasher` port の実装。

use sha2::{Digest, Sha256};

use crate::domain::shared::Hasher;

/// SHA-256（16進小文字）。
///
/// クラウド側(Workers)は WebCrypto の SHA-256 を使うが、
/// 入力（正規化済み JSON 文字列）が同じであれば出力は完全に一致する。
pub struct Sha256Hasher;

impl Hasher for Sha256Hasher {
    fn sha256_hex(&self, input: &str) -> String {
        format!("{:x}", Sha256::digest(input.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_known_sha256_of_an_empty_string() {
        assert_eq!(
            Sha256Hasher.sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
