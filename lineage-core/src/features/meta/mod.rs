//! メタ情報（`#タグ`）の補完。学習済みタグの並べ替え規則そのものは domain 側にある。

pub mod complete_meta_tag;

pub use complete_meta_tag::CompleteMetaTag;
