//! Type resolution helpers for expressions.
//!
//! - `get_expr_type`: FileTypeResolution から式の型を取得する
//! - `resolve_field_type`: TypeRegistry から構造体フィールドの宣言型を取得する
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::RustType;
use crate::pipeline::type_resolution::Span;
use crate::pipeline::ResolvedType;
use crate::registry::TypeDef;
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// FileTypeResolution から変数名と Span で型を取得する。
    ///
    /// `get_expr_type` の Ident 特化版。NarrowingGuard のように AST Expr への参照を
    /// 持たないが変数名と Span を持つ場合に使用する。
    pub(crate) fn get_type_for_var(
        &self,
        name: &str,
        span: swc_common::Span,
    ) -> Option<&'a RustType> {
        if let Some(narrowed) = self.tctx.type_resolution.narrowed_type(name, span.lo.0) {
            return Some(narrowed);
        }
        match self.tctx.type_resolution.expr_type(Span::from_swc(span)) {
            ResolvedType::Known(ty) => Some(ty),
            ResolvedType::Unknown => None,
        }
    }

    /// FileTypeResolution から式の型を取得する。Unknown なら None。
    ///
    /// TypeResolver が事前に解決した型のみを返す。
    /// any_enum_override は TypeResolver の declare_var 時に既に適用済みのため、
    /// ここでのフォールバックは不要。
    pub(crate) fn get_expr_type(&self, expr: &ast::Expr) -> Option<&'a RustType> {
        // Ident 式の場合、narrowed_type を優先参照（型ナローイング後の型）
        if let ast::Expr::Ident(ident) = expr {
            if let Some(narrowed) = self
                .tctx
                .type_resolution
                .narrowed_type(ident.sym.as_ref(), ident.span.lo.0)
            {
                return Some(narrowed);
            }
        }
        match self
            .tctx
            .type_resolution
            .expr_type(Span::from_swc(expr.span()))
        {
            ResolvedType::Known(ty) => Some(ty),
            ResolvedType::Unknown => None,
        }
    }

    /// Named 型のフィールド型を TypeRegistry から解決する。
    ///
    /// ジェネリック型の場合、`type_args` を使ってインスタンス化した TypeDef からフィールド型を解決する。
    pub(crate) fn resolve_field_type(
        &self,
        obj_type: &RustType,
        prop: &ast::MemberProp,
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
            self.reg().get(type_name)?.clone()
        } else {
            self.reg().instantiate(type_name, type_args)?
        };
        match &type_def {
            TypeDef::Struct { fields, .. } => fields
                .iter()
                .find(|f| f.name == field_name)
                .map(|f| f.ty.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ir::{RustType, TypeParam};
    use crate::registry::FieldDef;
    use crate::registry::{TypeDef, TypeRegistry};
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
                fields: vec![FieldDef {
                    name: "value".to_string(),
                    // I-387: 型変数は `TypeVar` で表現
                    ty: RustType::TypeVar {
                        name: "T".to_string(),
                    },
                    optional: false,
                }],
                methods: HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );

        // TypeRegistry::instantiate でジェネリック型をインスタンス化し、フィールド型を検証
        let type_def = reg
            .instantiate("Container", &[RustType::String])
            .expect("instantiation should succeed");
        let field_type = match &type_def {
            TypeDef::Struct { fields, .. } => fields
                .iter()
                .find(|f| f.name == "value")
                .map(|f| f.ty.clone()),
            _ => None,
        };
        assert_eq!(field_type, Some(RustType::String));
    }
}
