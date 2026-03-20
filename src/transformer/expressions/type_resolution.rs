//! Type resolution and type assertion conversion for expressions.
//!
//! Resolves expression types from TypeEnv and TypeRegistry, and converts
//! TypeScript type assertions (`x as T`) to IR.

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType};
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::types::convert_ts_type;
use crate::transformer::TypeEnv;

use super::{convert_expr, ExprContext};

/// 式の型を解決する。解決できない場合は None を返す。
///
/// TypeEnv からローカル変数の型を、TypeRegistry からフィールドの型を解決する。
/// メソッド呼び出しの戻り値型や型パラメータの具体化は対象外。
pub fn resolve_expr_type(
    expr: &ast::Expr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<RustType> {
    match expr {
        ast::Expr::Ident(ident) => type_env.get(ident.sym.as_ref()).cloned(),
        ast::Expr::Lit(ast::Lit::Str(_)) => Some(RustType::String),
        ast::Expr::Lit(ast::Lit::Num(_)) => Some(RustType::F64),
        ast::Expr::Lit(ast::Lit::Bool(_)) => Some(RustType::Bool),
        ast::Expr::Tpl(_) => Some(RustType::String),
        ast::Expr::Bin(bin) => resolve_bin_expr_type(bin, type_env, reg),
        ast::Expr::Member(member) => {
            let obj_type = resolve_expr_type(&member.obj, type_env, reg)?;
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
            resolve_field_type(&obj_type, &member.prop, reg)
        }
        ast::Expr::Paren(paren) => resolve_expr_type(&paren.expr, type_env, reg),
        ast::Expr::TsAs(ts_as) => convert_ts_type(&ts_as.type_ann, &mut Vec::new(), reg).ok(),
        ast::Expr::Call(call) => resolve_call_return_type(call, type_env, reg),
        ast::Expr::New(new_expr) => resolve_new_expr_type(new_expr, reg),
        _ => None,
    }
}

/// 二項演算の結果型を解決する。
fn resolve_bin_expr_type(
    bin: &ast::BinExpr,
    type_env: &TypeEnv,
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
            let left_ty = resolve_expr_type(&bin.left, type_env, reg);
            if left_ty
                .as_ref()
                .is_some_and(|t| matches!(t, RustType::String))
            {
                return Some(RustType::String);
            }
            let right_ty = resolve_expr_type(&bin.right, type_env, reg);
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
        LogicalAnd | LogicalOr | NullishCoalescing => resolve_expr_type(&bin.right, type_env, reg)
            .or_else(|| resolve_expr_type(&bin.left, type_env, reg)),
    }
}

/// 関数呼び出しの戻り値型を解決する。
fn resolve_call_return_type(
    call: &ast::CallExpr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<RustType> {
    // 関数名を取得
    let callee = call.callee.as_expr()?;
    let fn_name = match callee.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };

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

/// new 式の結果型を解決する。
fn resolve_new_expr_type(new_expr: &ast::NewExpr, reg: &TypeRegistry) -> Option<RustType> {
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
pub(super) fn resolve_field_type(
    obj_type: &RustType,
    prop: &ast::MemberProp,
    reg: &TypeRegistry,
) -> Option<RustType> {
    let type_name = match obj_type {
        RustType::Named { name, .. } => name,
        RustType::Option(inner) => match inner.as_ref() {
            RustType::Named { name, .. } => name,
            _ => return None,
        },
        _ => return None,
    };
    let field_name = match prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    let type_def = reg.get(type_name)?;
    match type_def {
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
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    match convert_ts_type(&ts_as.type_ann, &mut Vec::new(), reg) {
        Ok(target_ty) => {
            let is_primitive_cast = matches!(target_ty, RustType::F64 | RustType::Bool);
            if is_primitive_cast {
                let inner = convert_expr(
                    &ts_as.expr,
                    reg,
                    &ExprContext::with_expected(&target_ty),
                    type_env,
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
                convert_expr(&ts_as.expr, reg, &ctx, type_env)
            }
        }
        Err(_) => {
            // If we can't convert the type, just ignore the assertion
            let ctx = match expected {
                Some(ty) => ExprContext::with_expected(ty),
                // Cat C: type propagated when available
                None => ExprContext::none(),
            };
            convert_expr(&ts_as.expr, reg, &ctx, type_env)
        }
    }
}
