//! Pass 1 of the 2-pass registry collection: register placeholder
//! `TypeDef` entries for every declaration so that Pass 2 (in
//! [`super::decl`]) can resolve forward references.
//!
//! Holds [`collect_type_name`] (public to the parent) and the
//! `is_registrable_const_decl` predicate used by both passes to decide
//! whether a `const` declaration is a registry candidate (`as const`
//! or type-annotated).

use std::collections::HashMap;

use swc_ecma_ast as ast;

use crate::registry::{TypeDef, TypeRegistry};

/// Pass 1: 宣言から型名だけをプレースホルダーとして登録する。
///
/// フィールド型の解決は行わず、型名の存在だけを記録する。
/// これにより Pass 2 で前方参照を解決できる。
pub(in crate::registry) fn collect_type_name(reg: &mut TypeRegistry, decl: &ast::Decl) {
    match decl {
        ast::Decl::TsInterface(iface) => {
            reg.register(
                iface.id.sym.to_string(),
                TypeDef::new_interface(vec![], vec![], HashMap::new(), vec![]),
            );
        }
        ast::Decl::TsTypeAlias(alias) => {
            reg.register(
                alias.id.sym.to_string(),
                TypeDef::new_struct(vec![], HashMap::new(), vec![]),
            );
        }
        ast::Decl::TsEnum(ts_enum) => {
            reg.register(
                ts_enum.id.sym.to_string(),
                TypeDef::Enum {
                    type_params: vec![],
                    variants: vec![],
                    string_values: HashMap::new(),
                    tag_field: None,
                    variant_fields: HashMap::new(),
                },
            );
        }
        ast::Decl::Fn(fn_decl) => {
            reg.register(
                fn_decl.ident.sym.to_string(),
                TypeDef::Function {
                    type_params: vec![],
                    params: vec![],
                    return_type: None,
                    has_rest: false,
                },
            );
        }
        ast::Decl::Var(var_decl) => {
            for d in &var_decl.decls {
                let name = match &d.name {
                    ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                    _ => continue,
                };
                if let Some(init) = &d.init {
                    if let ast::Expr::Arrow(_) = init.as_ref() {
                        reg.register(
                            name,
                            TypeDef::Function {
                                type_params: vec![],
                                params: vec![],
                                return_type: None,
                                has_rest: false,
                            },
                        );
                        continue;
                    }
                }
                // `as const` or type-annotated const: register placeholder
                if is_registrable_const_decl(d) {
                    reg.register(
                        name,
                        TypeDef::ConstValue {
                            fields: vec![],
                            elements: vec![],
                            type_ref_name: None,
                        },
                    );
                }
            }
        }
        ast::Decl::Class(class) => {
            reg.register(
                class.ident.sym.to_string(),
                TypeDef::new_struct(vec![], HashMap::new(), vec![]),
            );
        }
        _ => {}
    }
}

/// `const` 宣言が TypeRegistry に登録すべきかどうか判定する。
///
/// 以下のいずれかに該当する場合に true:
/// - `as const` アサーション付き（`const X = [...] as const`）
/// - 明示的な型注釈付き（`const X: Type = ...`）
pub(super) fn is_registrable_const_decl(d: &ast::VarDeclarator) -> bool {
    // Check for type annotation
    if let ast::Pat::Ident(ident) = &d.name {
        if ident.type_ann.is_some() {
            return true;
        }
    }
    // Check for `as const` assertion
    if let Some(init) = &d.init {
        if matches!(init.as_ref(), ast::Expr::TsConstAssertion(_)) {
            return true;
        }
    }
    false
}
