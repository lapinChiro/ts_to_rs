//! Type resolution and type assertion conversion for expressions.
//!
//! まず `FileTypeResolution.expr_types` を参照し、Known ならそれを返す。
//! Unknown または未登録の場合は TypeEnv / TypeRegistry ベースのヒューリスティクスにフォールバックする。
//! TypeScript の型アサーション（`x as T`）も IR に変換する。

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType};
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::Span;
use crate::pipeline::{ResolvedType, SyntheticTypeRegistry};
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::context::TransformContext;
use crate::transformer::TypeEnv;

use super::{convert_expr, ExprContext};

/// 式の型を解決する。解決できない場合は None を返す。
///
/// まず `FileTypeResolution.expr_types` を参照し、Known ならそれを返す。
/// Unknown または未登録の場合は TypeEnv / TypeRegistry ベースのヒューリスティクスにフォールバックする。
pub fn resolve_expr_type(
    expr: &ast::Expr,
    type_env: &TypeEnv,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Option<RustType> {
    // FileTypeResolution lookup: Known ならヒューリスティクスを迂回
    let span = Span::from_swc(expr.span());
    if let ResolvedType::Known(ty) = tctx.type_resolution.expr_type(span) {
        return Some(ty.clone());
    }

    // フォールバック: 既存ヒューリスティクス
    resolve_expr_type_heuristic(expr, type_env, tctx, reg)
}

/// 既存のヒューリスティクスによる型解決。
///
/// TypeEnv からローカル変数の型を、TypeRegistry からフィールドの型を解決する。
fn resolve_expr_type_heuristic(
    expr: &ast::Expr,
    type_env: &TypeEnv,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Option<RustType> {
    match expr {
        ast::Expr::Ident(ident) => type_env.get(ident.sym.as_ref()).cloned(),
        ast::Expr::Lit(ast::Lit::Str(_)) => Some(RustType::String),
        ast::Expr::Lit(ast::Lit::Num(_)) => Some(RustType::F64),
        ast::Expr::Lit(ast::Lit::Bool(_)) => Some(RustType::Bool),
        ast::Expr::Tpl(_) => Some(RustType::String),
        ast::Expr::Bin(bin) => resolve_bin_expr_type(bin, type_env, tctx, reg),
        ast::Expr::Member(member) => {
            let obj_type = resolve_expr_type(&member.obj, type_env, tctx, reg)?;
            // インデックスアクセスの型解決
            if let ast::MemberProp::Computed(computed) = &member.prop {
                match &obj_type {
                    // 配列: Vec<T>[n] → T
                    RustType::Vec(elem_ty) => return Some(elem_ty.as_ref().clone()),
                    // タプル: (A, B)[0] → A
                    RustType::Tuple(elems) => {
                        if let ast::Expr::Lit(ast::Lit::Num(num)) = &*computed.expr {
                            let idx = num.value as usize;
                            if idx < elems.len() {
                                return Some(elems[idx].clone());
                            }
                        }
                        return None;
                    }
                    _ => {}
                }
            }
            resolve_field_type(&obj_type, &member.prop, tctx, reg)
        }
        ast::Expr::Paren(paren) => resolve_expr_type(&paren.expr, type_env, tctx, reg),
        ast::Expr::TsAs(ts_as) => {
            // resolve_expr_type is used in read-only contexts (type resolution for coercion
            // decisions). Synthetic types generated here are discarded since the actual
            // conversion happens separately via convert_ts_as_expr with the real registry.
            convert_ts_type(&ts_as.type_ann, &mut SyntheticTypeRegistry::new(), reg).ok()
        }
        ast::Expr::Call(call) => resolve_call_return_type(call, type_env, tctx, reg),
        ast::Expr::New(new_expr) => resolve_new_expr_type(new_expr, tctx, reg),
        _ => None,
    }
}

/// 二項演算の結果型を解決する。
fn resolve_bin_expr_type(
    bin: &ast::BinExpr,
    type_env: &TypeEnv,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Option<RustType> {
    use ast::BinaryOp::*;
    match bin.op {
        // 比較・等値 → Bool
        Lt | LtEq | Gt | GtEq | EqEq | NotEq | EqEqEq | NotEqEq | In | InstanceOf => {
            Some(RustType::Bool)
        }
        // 加算: 文字列 + any → String, otherwise F64
        Add => {
            let left_ty = resolve_expr_type(&bin.left, type_env, tctx, reg);
            if left_ty
                .as_ref()
                .is_some_and(|t| matches!(t, RustType::String))
            {
                return Some(RustType::String);
            }
            let right_ty = resolve_expr_type(&bin.right, type_env, tctx, reg);
            if right_ty
                .as_ref()
                .is_some_and(|t| matches!(t, RustType::String))
            {
                return Some(RustType::String);
            }
            Some(RustType::F64)
        }
        // 算術演算 → F64
        Sub | Mul | Div | Mod | Exp | BitAnd | BitOr | BitXor | LShift | RShift
        | ZeroFillRShift => Some(RustType::F64),
        // 論理演算 → operand の型（right 側で推定）
        LogicalAnd | LogicalOr | NullishCoalescing => {
            resolve_expr_type(&bin.right, type_env, tctx, reg)
                .or_else(|| resolve_expr_type(&bin.left, type_env, tctx, reg))
        }
    }
}

/// 関数呼び出しまたはメソッド呼び出しの戻り値型を解決する。
fn resolve_call_return_type(
    call: &ast::CallExpr,
    type_env: &TypeEnv,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Option<RustType> {
    let callee = call.callee.as_expr()?;
    match callee.as_ref() {
        ast::Expr::Ident(ident) => {
            let fn_name = ident.sym.to_string();
            // TypeEnv で Fn 型を探索
            if let Some(RustType::Fn { return_type, .. }) = type_env.get(&fn_name) {
                return Some(return_type.as_ref().clone());
            }
            // TypeRegistry で Function を探索
            if let Some(TypeDef::Function { return_type, .. }) = reg.get(&fn_name) {
                return Some(return_type.clone().unwrap_or(RustType::Unit));
            }
            None
        }
        ast::Expr::Member(member) => {
            // メソッド呼び出し: オブジェクト型を解決 → メソッドの戻り値型を取得
            let obj_type = resolve_expr_type(&member.obj, type_env, tctx, reg)?;
            let method_name = match &member.prop {
                ast::MemberProp::Ident(ident) => ident.sym.to_string(),
                _ => return None,
            };
            resolve_method_return_type(&obj_type, &method_name, tctx, reg)
        }
        _ => None,
    }
}

/// メソッドの戻り値型を TypeRegistry から解決する。
///
/// オブジェクト型（String, Vec, Named 等）に対応する TypeDef を探し、
/// メソッドシグネチャの戻り値型を返す。
/// ジェネリック型の場合、`type_args` を使ってインスタンス化した TypeDef から戻り値型を解決する。
pub(super) fn resolve_method_return_type(
    obj_type: &RustType,
    method_name: &str,
    _tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Option<RustType> {
    // オブジェクト型に対応する TypeRegistry のキーと型引数を決定
    let (type_name, type_args) = match obj_type {
        RustType::String => ("String", &[] as &[RustType]),
        RustType::Vec(_) => ("Vec", &[] as &[RustType]),
        RustType::Named { name, type_args } => (name.as_str(), type_args.as_slice()),
        RustType::Option(inner) => match inner.as_ref() {
            RustType::Named { name, type_args } => (name.as_str(), type_args.as_slice()),
            _ => return None,
        },
        _ => return None,
    };

    let type_def = if type_args.is_empty() {
        reg.get(type_name)?.clone()
    } else {
        reg.instantiate(type_name, type_args)?
    };
    match &type_def {
        TypeDef::Struct { methods, .. } => {
            let sig = methods.get(method_name)?;
            sig.return_type.clone()
        }
        _ => None,
    }
}

/// new 式の結果型を解決する。
fn resolve_new_expr_type(
    new_expr: &ast::NewExpr,
    _tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
) -> Option<RustType> {
    let class_name = match new_expr.callee.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };

    // TypeRegistry に登録されていれば Named 型を返す
    reg.get(&class_name)?;
    Some(RustType::Named {
        name: class_name,
        type_args: vec![],
    })
}

/// Named 型のフィールド型を TypeRegistry から解決する。
///
/// ジェネリック型の場合、`type_args` を使ってインスタンス化した TypeDef からフィールド型を解決する。
pub(super) fn resolve_field_type(
    obj_type: &RustType,
    prop: &ast::MemberProp,
    _tctx: &TransformContext<'_>,
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

/// Converts a TypeScript type assertion (`x as T`).
///
/// - Primitive types (f64, i64, bool): generates `x as T` cast
/// - Other types: passes the assertion type as `expected` to the inner expression
pub(super) fn convert_ts_as_expr(
    ts_as: &ast::TsAsExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    match convert_ts_type(&ts_as.type_ann, synthetic, reg) {
        Ok(target_ty) => {
            let is_primitive_cast = matches!(target_ty, RustType::F64 | RustType::Bool);
            if is_primitive_cast {
                let inner = convert_expr(
                    &ts_as.expr,
                    tctx,
                    reg,
                    &ExprContext::with_expected(&target_ty),
                    type_env,
                    synthetic,
                )?;
                Ok(Expr::Cast {
                    expr: Box::new(inner),
                    target: target_ty,
                })
            } else {
                // Pass the assertion type as expected to help type inference
                let merged = expected.or(Some(&target_ty));
                let ctx = match merged {
                    Some(ty) => ExprContext::with_expected(ty),
                    // Cat C: type propagated when available
                    None => ExprContext::none(),
                };
                convert_expr(&ts_as.expr, tctx, reg, &ctx, type_env, synthetic)
            }
        }
        Err(_) => {
            // If we can't convert the type, just ignore the assertion
            let ctx = match expected {
                Some(ty) => ExprContext::with_expected(ty),
                // Cat C: type propagated when available
                None => ExprContext::none(),
            };
            convert_expr(&ts_as.expr, tctx, reg, &ctx, type_env, synthetic)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::TypeParam;
    use crate::pipeline::type_resolution::FileTypeResolution;
    use crate::registry::{MethodSignature, TypeRegistry};
    use crate::transformer::test_fixtures::TctxFixture;
    use std::collections::HashMap;
    /// TypeEnv にオブジェクト型を登録し、TypeRegistry にメソッドの戻り値型を登録して
    /// resolve_expr_type が Call 式でメソッド戻り値型を返すことを検証する。
    ///
    /// テスト用の AST を手動構築するのは煩雑なため、registry のユニットテストと
    /// E2E テストでカバーし、ここでは resolve_method_return_type の単体テストのみ行う。

    #[test]
    fn test_resolve_method_return_type_from_registry() {
        // TypeRegistry に String メソッド trim() → String を登録
        let mut reg = TypeRegistry::new();
        let mut methods = HashMap::new();
        methods.insert(
            "trim".to_string(),
            MethodSignature {
                params: vec![],
                return_type: Some(RustType::String),
            },
        );
        reg.register(
            "String".to_string(),
            TypeDef::new_struct(vec![], methods, vec![]),
        );

        // String 型のオブジェクトの trim() は String を返す
        let f = TctxFixture::with_reg(reg);
        let tctx = f.tctx();
        let result = resolve_method_return_type(&RustType::String, "trim", &tctx, f.reg());
        assert_eq!(result, Some(RustType::String));
    }

    #[test]
    fn test_resolve_method_return_type_named_type() {
        // Named 型のメソッド戻り値型解決
        let mut reg = TypeRegistry::new();
        let mut methods = HashMap::new();
        methods.insert(
            "json".to_string(),
            MethodSignature {
                params: vec![],
                return_type: Some(RustType::Named {
                    name: "Value".to_string(),
                    type_args: vec![],
                }),
            },
        );
        reg.register(
            "Response".to_string(),
            TypeDef::new_struct(vec![], methods, vec![]),
        );

        let obj_type = RustType::Named {
            name: "Response".to_string(),
            type_args: vec![],
        };
        let f = TctxFixture::with_reg(reg);
        let tctx = f.tctx();
        let result = resolve_method_return_type(&obj_type, "json", &tctx, f.reg());
        assert_eq!(
            result,
            Some(RustType::Named {
                name: "Value".to_string(),
                type_args: vec![],
            })
        );
    }

    #[test]
    fn test_resolve_method_return_type_unknown_method_returns_none() {
        // 未知のメソッド → None（エラーにならない）
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let result = resolve_method_return_type(&RustType::String, "unknown", &tctx, f.reg());
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_method_return_type_vec_builtin() {
        // Vec 型のビルトインメソッドの戻り値型が解決できる
        let mut reg = TypeRegistry::new();
        let mut methods = HashMap::new();
        methods.insert(
            "len".to_string(),
            MethodSignature {
                params: vec![],
                return_type: Some(RustType::F64),
            },
        );
        reg.register(
            "Vec".to_string(),
            TypeDef::new_struct(vec![], methods, vec![]),
        );

        let obj_type = RustType::Vec(Box::new(RustType::F64));
        let f = TctxFixture::with_reg(reg);
        let tctx = f.tctx();
        let result = resolve_method_return_type(&obj_type, "len", &tctx, f.reg());
        assert_eq!(result, Some(RustType::F64));
    }

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
        let f = TctxFixture::with_reg(reg);
        let tctx = f.tctx();
        let result = resolve_field_type(&obj_type, &prop, &tctx, f.reg());
        assert_eq!(result, Some(RustType::String));
    }

    #[test]
    fn test_resolve_method_return_type_generic_instantiation() {
        // Container<T> { get(): T } で Container<String>.get() → String に解決される
        let mut reg = TypeRegistry::new();
        let mut methods = HashMap::new();
        methods.insert(
            "get".to_string(),
            MethodSignature {
                params: vec![],
                return_type: Some(RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                }),
            },
        );
        reg.register(
            "Container".to_string(),
            TypeDef::Struct {
                type_params: vec![TypeParam {
                    name: "T".to_string(),
                    constraint: None,
                }],
                fields: vec![],
                methods,
                extends: vec![],
                is_interface: false,
            },
        );

        let obj_type = RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::String],
        };
        let f = TctxFixture::with_reg(reg);
        let tctx = f.tctx();
        let result = resolve_method_return_type(&obj_type, "get", &tctx, f.reg());
        assert_eq!(result, Some(RustType::String));
    }

    // ===== FileTypeResolution lookup tests (Phase C) =====

    #[test]
    fn test_resolve_expr_type_prefers_file_resolution_over_heuristic() {
        // FileTypeResolution に Known(F64) が登録された Ident 式は、
        // TypeEnv に登録がなくても F64 を返すべき
        use crate::pipeline::type_resolution::Span;
        use crate::pipeline::ResolvedType;

        let ident_span = Span { lo: 0, hi: 1 };
        let mut res = FileTypeResolution::empty();
        res.expr_types
            .insert(ident_span, ResolvedType::Known(RustType::F64));

        let f = TctxFixture::with_resolution(res);
        let tctx = f.tctx();

        // SWC Ident AST node with matching span
        let expr = ast::Expr::Ident(ast::Ident {
            span: swc_common::Span::new(swc_common::BytePos(0), swc_common::BytePos(1)),
            ctxt: Default::default(),
            sym: "unknown_var".into(),
            optional: false,
        });

        let env = TypeEnv::new(); // empty — no type for "unknown_var"
        let result = resolve_expr_type(&expr, &env, &tctx, f.reg());

        // FileTypeResolution should take precedence over heuristic (which returns None)
        assert_eq!(
            result,
            Some(RustType::F64),
            "FileTypeResolution Known should override heuristic"
        );
    }

    #[test]
    fn test_resolve_expr_type_falls_back_when_resolution_unknown() {
        // FileTypeResolution に Unknown が登録 → 既存ヒューリスティクスにフォールバック
        use crate::pipeline::type_resolution::Span;
        use crate::pipeline::ResolvedType;

        let lit_span = Span { lo: 0, hi: 2 };
        let mut res = FileTypeResolution::empty();
        res.expr_types.insert(lit_span, ResolvedType::Unknown);

        let f = TctxFixture::with_resolution(res);
        let tctx = f.tctx();

        // 数値リテラル "42" — ヒューリスティクスは Some(F64) を返す
        let expr = ast::Expr::Lit(ast::Lit::Num(ast::Number {
            span: swc_common::Span::new(swc_common::BytePos(0), swc_common::BytePos(2)),
            value: 42.0,
            raw: None,
        }));

        let env = TypeEnv::new();
        let result = resolve_expr_type(&expr, &env, &tctx, f.reg());

        assert_eq!(
            result,
            Some(RustType::F64),
            "Unknown in FileTypeResolution should fall back to heuristic"
        );
    }

    #[test]
    fn test_resolve_expr_type_falls_back_when_span_not_in_resolution() {
        // FileTypeResolution に該当 span がない → 既存ヒューリスティクスにフォールバック
        let f = TctxFixture::new(); // empty resolution
        let tctx = f.tctx();

        let expr = ast::Expr::Lit(ast::Lit::Str(ast::Str {
            span: swc_common::DUMMY_SP,
            value: "hello".into(),
            raw: None,
        }));

        let env = TypeEnv::new();
        let result = resolve_expr_type(&expr, &env, &tctx, f.reg());

        assert_eq!(
            result,
            Some(RustType::String),
            "Missing span should fall back to heuristic"
        );
    }
}
