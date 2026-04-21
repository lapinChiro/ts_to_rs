//! 2-pass 型収集ロジック。
//!
//! Pass 1 で型名をプレースホルダー登録し、Pass 2 でフィールド型を完全解決する。
//!
//! Collection 関数は SWC AST → TsTypeInfo の変換を先行し、TsTypeInfo を起点として
//! resolve 関数を使用する。SWC AST を直接解析するアドホックなロジックは持たない。
//! TypeDef<TsTypeInfo> として自然に表現できる型は `resolvers::resolve_struct_for_registry`
//! に委譲し、できない型（intersection, utility type ref 等）は resolve 関数を直接
//! 使用して TypeDef<RustType> を構築する。
//!
//! ## Module layout
//!
//! - [`placeholder`] — Pass 1 (`collect_type_name` + the
//!   `is_registrable_const_decl` predicate)
//! - [`decl`] — Pass 2 main dispatcher (`collect_decl`)
//! - [`class`] — class-specific collection (`collect_class_info`)
//! - [`resolvers`] — `TypeDef<TsTypeInfo>` → `TypeDef<RustType>`
//!   resolvers (`resolve_struct/type_ref/intersection_for_registry`),
//!   kept together because they mutually recurse
//! - [`type_literals`] — `build_struct_from_type_literal` + sig
//!   converters + associated tests
//! - [`const_values`] — 5 functions that extract const value shape
//!   from `as const` / type-annotated declarations
//! - [`callable`] — [`CallableInterfaceKind`] enum and the public
//!   [`classify_callable_interface`] dispatcher
//!
//! `collect_type_params` lives here at the root since it is shared by
//! `decl`, `class`, and the external `interfaces.rs` module.

use swc_ecma_ast as ast;

use crate::ir::TypeParam;
use crate::ts_type_info::{convert_to_ts_type_info, TsTypeInfo};

mod callable;
mod class;
mod const_values;
mod decl;
mod placeholder;
mod resolvers;
mod type_literals;

pub(super) use decl::collect_decl;
pub(super) use placeholder::collect_type_name;

pub use callable::{classify_callable_interface, CallableInterfaceKind};

/// TS の型パラメータ宣言から `TypeParam<TsTypeInfo>` を収集する。
///
/// 制約は `convert_to_ts_type_info` で TsTypeInfo に変換する（TypeRegistry 不要）。
pub(crate) fn collect_type_params(
    decl: Option<&ast::TsTypeParamDecl>,
) -> Vec<TypeParam<TsTypeInfo>> {
    decl.map(|d| {
        d.params
            .iter()
            .map(|p| TypeParam {
                name: p.name.sym.to_string(),
                constraint: p
                    .constraint
                    .as_ref()
                    .and_then(|c| convert_to_ts_type_info(c).ok()),
                default: p
                    .default
                    .as_ref()
                    .and_then(|d| convert_to_ts_type_info(d).ok()),
            })
            .collect()
    })
    .unwrap_or_default()
}
