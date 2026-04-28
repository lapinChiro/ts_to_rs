//! `MethodKind` — TS class member の method 種別 (Method / Getter / Setter)。
//!
//! I-205: TS class member の `get` / `set` / 通常 method を区別するため導入。
//! 概念的に **3 layer 横断 primitive** (registry / ts_type_info / transformer) のため、
//! foundational module (`src/ir/method_kind.rs`) に配置する。`registry::MethodSignature.kind`
//! と `ts_type_info::TsMethodInfo.kind` の両方が共通 enum を参照することで、レジストリと
//! 型情報のレイヤー間で循環依存を発生させない (= I-205 T1-T3 `/check_job` 4-layer review で
//! 発見された原因 1 = "MethodKind の foundational placement 不在" の構造的解消)。
//!
//! call site (`resolve_member_access`、`dispatch_member_write`) で getter/setter dispatch の
//! 判別に利用される。
//!
//! ## SWC との boundary conversion
//!
//! SWC `swc_ecma_ast::MethodKind` ↔ IR `MethodKind` の `From` 変換 impl は `src/ir/` の
//! SWC independence convention (per `pipeline-integrity.md`) を維持するため、本 module
//! ではなく registry layer の boundary module
//! [`crate::registry::swc_method_kind`](../../registry/swc_method_kind/index.html) に配置。
//! call site では `MethodKind::from(swc_kind)` / `swc_kind.into()` で透過的に利用可能
//! (Rust orphan rule により trait impl は型を所有する crate ならどこでも定義可)。

/// メソッド種別（SWC `swc_ecma_ast::MethodKind` の 1-to-1 mirror）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MethodKind {
    /// 通常 method (`method() {}`)。Default。
    #[default]
    Method,
    /// Getter method (`get x() {}`)。
    Getter,
    /// Setter method (`set x(v) {}`)。
    Setter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_kind_default_is_method() {
        // I-205: Default が Method であることが MethodSignature の backward compat の前提
        assert_eq!(MethodKind::default(), MethodKind::Method);
    }

    #[test]
    fn test_method_kind_value_assignment_preserves_equality() {
        // Copy trait property: 値 assignment 後も original / copy の equality が preserved
        // (= shared dispatch logic で MethodKind を value 渡しできる前提)。
        let kind = MethodKind::Getter;
        let copied = kind;
        assert_eq!(kind, copied);
    }

    #[test]
    fn test_method_kind_three_variants_distinct() {
        // I-205: Method / Getter / Setter の 3 variant が distinct (call site で
        // `sigs.iter().any(|s| s.kind == MethodKind::Getter)` 等の dispatch 判別の前提)
        assert_ne!(MethodKind::Method, MethodKind::Getter);
        assert_ne!(MethodKind::Method, MethodKind::Setter);
        assert_ne!(MethodKind::Getter, MethodKind::Setter);
    }
}
