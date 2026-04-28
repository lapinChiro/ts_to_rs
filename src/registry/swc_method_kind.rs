//! Boundary conversion: SWC `MethodKind` → IR `MethodKind`.
//!
//! Pipeline integrity (per [`pipeline-integrity.md`](../../.claude/rules/pipeline-integrity.md)
//! convention): `src/ir/` は abstract IR layer として SWC-independent を維持する。本 module
//! は registry layer (既に SWC を import 済) に配置され、`From<swc_ecma_ast::MethodKind>`
//! boundary impl を提供する。
//!
//! I-205 T1-T3 batch `/check_job` deep review (2026-04-28) 由来 (D1 fix): pre-fix では
//! `From` impl を `src/ir/method_kind.rs` に配置していたが、これは `src/ir/` 配下で唯一の
//! SWC 依存 file となり pipeline integrity convention 違反 (codebase 全体でも `From<swc_*>`
//! impl は他に存在せず、本 file が boundary impl pattern の origin)。本 file を新設する
//! ことで:
//!
//! - `src/ir/` の SWC independence 維持 (✓ pipeline integrity restored)
//! - SWC ↔ IR boundary conversion の discoverable な置き場確立 (= future SWC type の IR
//!   conversion も同 module / 同 directory pattern で展開可能)
//! - 単一 file scope (1 trait impl + 関連 test) で over-engineering 回避

use crate::ir::MethodKind;

impl From<swc_ecma_ast::MethodKind> for MethodKind {
    fn from(kind: swc_ecma_ast::MethodKind) -> Self {
        match kind {
            swc_ecma_ast::MethodKind::Method => Self::Method,
            swc_ecma_ast::MethodKind::Getter => Self::Getter,
            swc_ecma_ast::MethodKind::Setter => Self::Setter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_swc_method_kind_method_maps_to_method() {
        assert_eq!(
            MethodKind::from(swc_ecma_ast::MethodKind::Method),
            MethodKind::Method
        );
    }

    #[test]
    fn test_from_swc_method_kind_getter_maps_to_getter() {
        assert_eq!(
            MethodKind::from(swc_ecma_ast::MethodKind::Getter),
            MethodKind::Getter
        );
    }

    #[test]
    fn test_from_swc_method_kind_setter_maps_to_setter() {
        assert_eq!(
            MethodKind::from(swc_ecma_ast::MethodKind::Setter),
            MethodKind::Setter
        );
    }

    #[test]
    fn test_into_chain_from_swc_method_kind_to_ir() {
        // I-205 D1 fix: `From<swc_ecma_ast::MethodKind>` → `Into<MethodKind>` の chain
        // が boundary conversion で利用可能であることを verify (call site で `.into()` 形式
        // も使えるため)。
        let swc_kind: swc_ecma_ast::MethodKind = swc_ecma_ast::MethodKind::Getter;
        let ir_kind: MethodKind = swc_kind.into();
        assert_eq!(ir_kind, MethodKind::Getter);
    }
}
