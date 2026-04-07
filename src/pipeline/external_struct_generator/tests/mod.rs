//! `external_struct_generator` のユニットテスト。カテゴリ別に分割:
//!
//! - [`refs_from_types_tests`]: `collect_type_refs` — 基本型（struct field / enum data / tuple / Result 等）
//! - [`refs_from_items_tests`]: `collect_type_refs` — Item レベル（Impl / Trait / TypeAlias / QSelf / Dyn）
//! - [`refs_from_bodies_tests`]: `collect_type_refs` — fn body / impl method body / type_params constraint
//! - [`refs_from_patterns_tests`]: `collect_type_refs` — 構造化 `Pattern`（MatchArm / IfLet / Matches）
//! - [`generate_struct_tests`]: `generate_external_struct` とモノモーフィゼーション
//! - [`undefined_refs_tests`]: `collect_all_undefined_references` / `generate_stub_structs` / `UndefinedRefScope`
//! - [`walker_tests`]: `Expr::FnCall` walker（CallTarget 経由の型参照捕捉）

use super::*;
use crate::ir::{
    AssocConst, BinOp, CallTarget, ClosureBody, EnumVariant, Expr, MatchArm, Method, Param, Stmt,
    TraitRef, TypeParam,
};
use crate::pipeline::SyntheticTypeRegistry;
use std::collections::HashMap;

mod generate_struct_tests;
mod refs_from_bodies_tests;
mod refs_from_items_tests;
mod refs_from_patterns_tests;
mod refs_from_types_tests;
mod undefined_refs_tests;
mod walker_tests;

fn named(name: &str) -> RustType {
    RustType::Named {
        name: name.to_string(),
        type_args: vec![],
    }
}

fn fn_with_body(name: &str, body: Vec<Stmt>) -> Item {
    Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: name.to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body,
    }
}

fn fn_with_body_and_type_params(name: &str, type_params: Vec<TypeParam>, body: Vec<Stmt>) -> Item {
    Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: name.to_string(),
        type_params,
        params: vec![],
        return_type: None,
        body,
    }
}

/// テスト用に TypeRegistry に外部型としてフィールド付き struct 型を登録するヘルパー。
fn register_external_struct(
    registry: &mut TypeRegistry,
    name: &str,
    fields: Vec<(&str, RustType)>,
    type_params: Vec<TypeParam>,
) {
    registry.register_external(
        name.to_string(),
        TypeDef::Struct {
            type_params,
            fields: fields
                .into_iter()
                .map(|(n, ty)| (n.to_string(), ty))
                .map(Into::into)
                .collect(),
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );
}
