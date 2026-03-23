//! Type resolution helpers for expressions.
//!
//! - `get_expr_type`: FileTypeResolution から式の型を取得する
//! - `resolve_field_type`: TypeRegistry から構造体フィールドの宣言型を取得する
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::RustType;
use crate::pipeline::type_resolution::Span;
use crate::pipeline::ResolvedType;
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::context::TransformContext;

/// FileTypeResolution から式の型を取得する。Unknown なら None。
///
/// TypeResolver が事前に解決した型のみを返す。
pub(crate) fn get_expr_type<'a>(
    tctx: &'a TransformContext<'_>,
    expr: &ast::Expr,
) -> Option<&'a RustType> {
    // Ident 式の場合、narrowed_type を優先参照（型ナローイング後の型）
    if let ast::Expr::Ident(ident) = expr {
        if let Some(narrowed) = tctx
            .type_resolution
            .narrowed_type(ident.sym.as_ref(), ident.span.lo.0)
        {
            return Some(narrowed);
        }
    }
    match tctx.type_resolution.expr_type(Span::from_swc(expr.span())) {
        ResolvedType::Known(ty) => Some(ty),
        ResolvedType::Unknown => None,
    }
}

/// Named 型のフィールド型を TypeRegistry から解決する。
///
/// ジェネリック型の場合、`type_args` を使ってインスタンス化した TypeDef からフィールド型を解決する。
pub(super) fn resolve_field_type(
    obj_type: &RustType,
    prop: &ast::MemberProp,
    reg: &TypeRegistry,
) -> Option<RustType> {
    let (type_name, type_args) = match obj_type {
        RustType::Named { name, type_args } => (name.as_str(), type_args.as_slice()),
        RustType::Option(inner) => match inner.as_ref() {
            RustType::Named { name, type_args } => (name.as_str(), type_args.as_slice()),
            _ => return None,
        },
        _ => return None,
    };
    let field_name = match prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    let type_def = if type_args.is_empty() {
        reg.get(type_name)?.clone()
    } else {
        reg.instantiate(type_name, type_args)?
    };
    match &type_def {
        TypeDef::Struct { fields, .. } => fields
            .iter()
            .find(|(name, _)| name == &field_name)
            .map(|(_, ty)| ty.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::TypeParam;
    use std::collections::HashMap;

    #[test]
    fn test_resolve_field_type_generic_instantiation() {
        // Container<T> { value: T } で Container<String>.value → String に解決される
        let mut reg = TypeRegistry::new();
        reg.register(
            "Container".to_string(),
            TypeDef::Struct {
                type_params: vec![TypeParam {
                    name: "T".to_string(),
                    constraint: None,
                }],
                fields: vec![(
                    "value".to_string(),
                    RustType::Named {
                        name: "T".to_string(),
                        type_args: vec![],
                    },
                )],
                methods: HashMap::new(),
                extends: vec![],
                is_interface: false,
            },
        );

        let obj_type = RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::String],
        };
        // "value" プロパティの型解決用 AST ノードを作成
        let prop = ast::MemberProp::Ident(ast::IdentName {
            span: swc_common::DUMMY_SP,
            sym: "value".into(),
        });
        let result = resolve_field_type(&obj_type, &prop, &reg);
        assert_eq!(result, Some(RustType::String));
    }
}
