//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType, Stmt, UnOp};
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::functions::{convert_last_return_to_tail, convert_ts_type_with_fallback};
use crate::transformer::statements::convert_stmt;
use crate::transformer::types::convert_ts_type;
use crate::transformer::TypeEnv;

/// Converts an SWC [`ast::Expr`] into an IR [`Expr`], with an optional expected type.
///
/// The `expected` type is used for:
/// - Object literals: determines the struct name from `RustType::Named`
/// - String literals: adds `.to_string()` when `RustType::String` is expected
/// - Array literals: propagates element type from `RustType::Vec`
///
/// # Errors
///
/// Returns an error for unsupported expression types.
pub fn convert_expr(
    expr: &ast::Expr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    // Option<T> expected: handle null/undefined → None, literals → Some(lit)
    if let Some(RustType::Option(inner)) = expected {
        // null / undefined → None
        if matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
            || matches!(expr, ast::Expr::Lit(ast::Lit::Null(..)))
        {
            return Ok(Expr::Ident("None".to_string()));
        }
        // Wrap non-null literals in Some() (needed for array elements like vec![Some(1.0), None])
        if matches!(expr, ast::Expr::Lit(_)) {
            let inner_result = convert_expr(expr, reg, Some(inner), type_env)?;
            return Ok(Expr::FnCall {
                name: "Some".to_string(),
                args: vec![inner_result],
            });
        }
    }

    match expr {
        ast::Expr::Ident(ident) => {
            let name = ident.sym.to_string();
            match name.as_str() {
                "undefined" => Ok(Expr::Ident("None".to_string())),
                "NaN" => Ok(Expr::Ident("f64::NAN".to_string())),
                "Infinity" => Ok(Expr::Ident("f64::INFINITY".to_string())),
                _ => Ok(Expr::Ident(name)),
            }
        }
        ast::Expr::Lit(lit) => convert_lit(lit, expected, reg),
        ast::Expr::Bin(bin) => convert_bin_expr(bin, reg, expected, type_env),
        ast::Expr::Tpl(tpl) => convert_template_literal(tpl, reg, type_env),
        ast::Expr::Paren(paren) => convert_expr(&paren.expr, reg, expected, type_env),
        ast::Expr::Member(member) => convert_member_expr(member, reg, type_env),
        ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
        ast::Expr::Assign(assign) => convert_assign_expr(assign, reg, type_env),
        ast::Expr::Update(up) => convert_update_expr(up),
        ast::Expr::Arrow(arrow) => convert_arrow_expr(arrow, reg, false, &mut Vec::new(), type_env),
        ast::Expr::Fn(fn_expr) => convert_fn_expr(fn_expr, reg, type_env),
        ast::Expr::Call(call) => convert_call_expr(call, reg, type_env),
        ast::Expr::New(new_expr) => convert_new_expr(new_expr, reg, type_env),
        ast::Expr::Array(array_lit) => convert_array_lit(array_lit, reg, expected, type_env),
        ast::Expr::Object(obj_lit) => convert_object_lit(obj_lit, reg, expected, type_env),
        ast::Expr::Cond(cond) => convert_cond_expr(cond, reg, expected, type_env),
        ast::Expr::Unary(unary) => convert_unary_expr(unary, reg, type_env),
        ast::Expr::TsAs(ts_as) => convert_ts_as_expr(ts_as, reg, expected, type_env),
        ast::Expr::OptChain(opt_chain) => convert_opt_chain_expr(opt_chain, reg, type_env),
        ast::Expr::Await(await_expr) => {
            let inner = convert_expr(&await_expr.arg, reg, None, type_env)?;
            Ok(Expr::Await(Box::new(inner)))
        }
        // Non-null assertion (expr!) — TS type-level only, no runtime effect. Strip assertion.
        ast::Expr::TsNonNull(ts_non_null) => {
            convert_expr(&ts_non_null.expr, reg, expected, type_env)
        }
        _ => Err(anyhow!("unsupported expression: {:?}", expr)),
    }
}

/// Converts an SWC literal to an IR expression.
///
/// When `expected` is `RustType::String`, string literals are wrapped with `.to_string()`
/// to produce an owned `String` instead of `&str`.
fn convert_lit(lit: &ast::Lit, expected: Option<&RustType>, reg: &TypeRegistry) -> Result<Expr> {
    match lit {
        ast::Lit::Num(n) => Ok(Expr::NumberLit(n.value)),
        ast::Lit::Str(s) => {
            let value = s.value.to_string_lossy().into_owned();
            // Check if the expected type is a string literal union enum
            if let Some(RustType::Named { name, .. }) = expected {
                if let Some(variant) = lookup_string_enum_variant(reg, name, &value) {
                    return Ok(Expr::Ident(format!("{name}::{variant}")));
                }
            }
            let expr = Expr::StringLit(value);
            if matches!(expected, Some(RustType::String)) {
                Ok(Expr::MethodCall {
                    object: Box::new(expr),
                    method: "to_string".to_string(),
                    args: vec![],
                })
            } else {
                Ok(expr)
            }
        }
        ast::Lit::Bool(b) => Ok(Expr::BoolLit(b.value)),
        ast::Lit::Null(_) => Ok(Expr::Ident("None".to_string())),
        ast::Lit::Regex(regex) => {
            let pattern = regex.exp.to_string();
            let flags = regex.flags.to_string();
            // Embed supported flags as inline flags in the pattern
            let mut prefix = String::new();
            if flags.contains('i') {
                prefix.push_str("(?i)");
            }
            if flags.contains('m') {
                prefix.push_str("(?m)");
            }
            if flags.contains('s') {
                prefix.push_str("(?s)");
            }
            // 'u' flag: Rust regex is Unicode-aware by default — no action needed.
            let full_pattern = format!("{prefix}{pattern}");
            Ok(Expr::Regex {
                pattern: full_pattern,
                global: flags.contains('g'),
                sticky: flags.contains('y'),
            })
        }
        _ => Err(anyhow!("unsupported literal: {:?}", lit)),
    }
}

/// 文字列リテラル値から string literal union enum のバリアント名を逆引きする。
fn lookup_string_enum_variant<'a>(
    reg: &'a TypeRegistry,
    enum_name: &str,
    string_value: &str,
) -> Option<&'a String> {
    if let Some(TypeDef::Enum { string_values, .. }) = reg.get(enum_name) {
        string_values.get(string_value)
    } else {
        None
    }
}

/// discriminated union のオブジェクトリテラルを enum バリアント構築に変換する。
///
/// `{ kind: "circle", radius: 5 }` → `Shape::Circle { radius: 5.0 }`
fn convert_discriminated_union_object_lit(
    obj_lit: &ast::ObjectLit,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    enum_name: &str,
    tag_field: &str,
    string_values: &std::collections::HashMap<String, String>,
    variant_fields_map: &std::collections::HashMap<String, Vec<(String, RustType)>>,
) -> Result<Expr> {
    // Find the discriminant field value
    let mut disc_value = None;
    for prop in &obj_lit.props {
        if let ast::PropOrSpread::Prop(prop) = prop {
            if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                let key = match &kv.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                    _ => continue,
                };
                if key == tag_field {
                    if let ast::Expr::Lit(ast::Lit::Str(s)) = kv.value.as_ref() {
                        disc_value = Some(s.value.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    let disc_value = disc_value.ok_or_else(|| {
        anyhow!("discriminated union object literal missing discriminant field '{tag_field}'")
    })?;

    let variant_name = string_values.get(&disc_value).ok_or_else(|| {
        anyhow!("unknown discriminant value '{disc_value}' for enum '{enum_name}'")
    })?;

    let variant_field_types = variant_fields_map.get(variant_name);

    // Build fields (excluding the discriminant field)
    let mut fields = Vec::new();
    for prop in &obj_lit.props {
        if let ast::PropOrSpread::Prop(prop) = prop {
            match prop.as_ref() {
                ast::Prop::KeyValue(kv) => {
                    let key = match &kv.key {
                        ast::PropName::Ident(ident) => ident.sym.to_string(),
                        ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => continue,
                    };
                    if key == tag_field {
                        continue; // Skip discriminant field
                    }
                    let field_expected = variant_field_types
                        .and_then(|fs| fs.iter().find(|(n, _)| n == &key).map(|(_, ty)| ty));
                    let value = convert_expr(&kv.value, reg, field_expected, type_env)?;
                    fields.push((key, value));
                }
                ast::Prop::Shorthand(ident) => {
                    let key = ident.sym.to_string();
                    if key == tag_field {
                        continue;
                    }
                    let field_expected = variant_field_types
                        .and_then(|fs| fs.iter().find(|(n, _)| n == &key).map(|(_, ty)| ty));
                    let value = convert_expr(
                        &ast::Expr::Ident(ident.clone()),
                        reg,
                        field_expected,
                        type_env,
                    )?;
                    fields.push((key, value));
                }
                _ => {}
            }
        }
    }

    let full_name = format!("{enum_name}::{variant_name}");

    // Unit variant (no fields) → Ident
    if fields.is_empty() {
        return Ok(Expr::Ident(full_name));
    }

    Ok(Expr::StructInit {
        name: full_name,
        fields,
        base: None,
    })
}

/// Checks whether a RustType represents a string (including Option<String>).
fn is_string_type(ty: &RustType) -> bool {
    matches!(ty, RustType::String)
        || matches!(ty, RustType::Option(inner) if matches!(inner.as_ref(), RustType::String))
}

/// `println!` の引数で `{:?}` (Debug) を使うべき型かどうかを判定する。
///
/// `Vec<T>`, `Option<T>`, `Tuple`, 型不明の場合は Debug フォーマットを使う。
/// プリミティブ型と Named 型（enum/struct）は Display を使う。
fn needs_debug_format(ty: Option<&RustType>) -> bool {
    match ty {
        None => false, // 型不明の場合は Display を試みる（コンパイルエラーで発見できる）
        Some(RustType::Vec(_)) => true,
        Some(RustType::Option(_)) => true,
        Some(RustType::Tuple(_)) => true,
        _ => false,
    }
}

/// Checks whether an IR expression is known to produce a String value.
///
/// Used to detect string concatenation (`+`) and wrap the RHS in `&`.
fn is_string_like(expr: &Expr) -> bool {
    match expr {
        Expr::StringLit(_) | Expr::FormatMacro { .. } => true,
        Expr::MethodCall { method, .. }
            if method == "to_string"
                || method == "to_uppercase"
                || method == "to_lowercase"
                || method == "trim"
                || method == "replacen" =>
        {
            true
        }
        Expr::BinaryOp {
            op: BinOp::Add,
            left,
            ..
        } => is_string_like(left),
        _ => false,
    }
}

/// Converts an SWC binary expression to an IR `BinaryOp`.
fn convert_bin_expr(
    bin: &ast::BinExpr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    // typeof x === "type" / typeof x !== "type" pattern
    if let Some(result) = try_convert_typeof_comparison(bin, type_env, reg) {
        return Ok(result);
    }

    // x === undefined / x !== undefined pattern
    if let Some(result) = try_convert_undefined_comparison(bin, type_env, reg) {
        return Ok(result);
    }

    // string literal enum comparison: d == "up" → d == Direction::Up
    if let Some(result) = try_convert_enum_string_comparison(bin, type_env, reg) {
        return Ok(result);
    }

    // x instanceof ClassName pattern
    if bin.op == ast::BinaryOp::InstanceOf {
        return Ok(convert_instanceof(bin, type_env));
    }

    // "key" in obj pattern
    if bin.op == ast::BinaryOp::In {
        return Ok(convert_in_operator(bin, reg, type_env));
    }

    // `x ?? y` → `x.unwrap_or_else(|| y)` (Option) or `x` (non-Option)
    if bin.op == ast::BinaryOp::NullishCoalescing {
        let left_type = resolve_expr_type(&bin.left, type_env, reg);
        let is_option = left_type
            .as_ref()
            .is_some_and(|ty| matches!(ty, RustType::Option(_)));

        let left = convert_expr(&bin.left, reg, None, type_env)?;
        if !is_option && left_type.is_some() {
            // Non-Option type: nullish coalescing is a no-op, return left as-is
            return Ok(left);
        }
        let inner_expected = match &left_type {
            Some(RustType::Option(inner)) => Some(inner.as_ref().clone()),
            _ => None,
        };
        let right = convert_expr(&bin.right, reg, inner_expected.as_ref(), type_env)?;
        return Ok(Expr::MethodCall {
            object: Box::new(left),
            method: "unwrap_or_else".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: ClosureBody::Expr(Box::new(right)),
            }],
        });
    }

    let left = convert_expr(&bin.left, reg, None, type_env)?;
    let right = convert_expr(&bin.right, reg, None, type_env)?;
    let op = convert_binary_op(bin.op)?;

    // String concatenation: wrap RHS in Ref(&) when LHS is string-like.
    // Priority: type inference → expected type → IR heuristic (is_string_like fallback).
    let is_string_context = if op == BinOp::Add {
        let left_type = resolve_expr_type(&bin.left, type_env, reg);
        let type_inferred = left_type.is_some_and(|ty| is_string_type(&ty));
        type_inferred || matches!(expected, Some(RustType::String)) || is_string_like(&left)
    } else {
        false
    };

    // Mixed-type concatenation: one side is string, other is known non-string → format!
    // Handles: `42 + " px"` (f64 + &str) and `"val: " + x` (String + f64)
    if op == BinOp::Add && is_string_context {
        let left_type = resolve_expr_type(&bin.left, type_env, reg);
        let right_type = resolve_expr_type(&bin.right, type_env, reg);
        let left_is_string =
            left_type.as_ref().is_some_and(is_string_type) || is_string_like(&left);
        let left_known_non_string = (left_type.is_some()
            && !left_type.as_ref().is_some_and(is_string_type))
            && !is_string_like(&left);
        let right_known_non_string = (right_type.is_some()
            && !right_type.as_ref().is_some_and(is_string_type))
            && !is_string_like(&right);

        if (left_known_non_string && !left_is_string) || (right_known_non_string && left_is_string)
        {
            return Ok(Expr::FormatMacro {
                template: "{}{}".to_string(),
                args: vec![left, right],
            });
        }
    }

    // In string concat context:
    // - LHS StringLit needs .to_string() (Rust: &str can't use + operator directly)
    // - LHS self.field needs .clone() (Rust: can't move out of &self)
    // - RHS non-literal needs & (Rust: String + &str)
    let left = if is_string_context && matches!(left, Expr::StringLit(_)) {
        Expr::MethodCall {
            object: Box::new(left),
            method: "to_string".to_string(),
            args: vec![],
        }
    } else if is_string_context
        && matches!(
            &left,
            Expr::FieldAccess { object, .. } if matches!(object.as_ref(), Expr::Ident(name) if name == "self")
        )
    {
        Expr::MethodCall {
            object: Box::new(left),
            method: "clone".to_string(),
            args: vec![],
        }
    } else {
        left
    };

    let right = if is_string_context && !matches!(right, Expr::StringLit(_)) {
        Expr::Ref(Box::new(right))
    } else {
        right
    };

    Ok(Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
    })
}

/// Converts an SWC binary operator to an IR [`BinOp`].
pub(crate) fn convert_binary_op(op: ast::BinaryOp) -> Result<BinOp> {
    match op {
        ast::BinaryOp::Add => Ok(BinOp::Add),
        ast::BinaryOp::Sub => Ok(BinOp::Sub),
        ast::BinaryOp::Mul => Ok(BinOp::Mul),
        ast::BinaryOp::Div => Ok(BinOp::Div),
        ast::BinaryOp::Mod => Ok(BinOp::Mod),
        ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq => Ok(BinOp::Eq),
        ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq => Ok(BinOp::NotEq),
        ast::BinaryOp::Lt => Ok(BinOp::Lt),
        ast::BinaryOp::LtEq => Ok(BinOp::LtEq),
        ast::BinaryOp::Gt => Ok(BinOp::Gt),
        ast::BinaryOp::GtEq => Ok(BinOp::GtEq),
        ast::BinaryOp::LogicalAnd => Ok(BinOp::LogicalAnd),
        ast::BinaryOp::LogicalOr => Ok(BinOp::LogicalOr),
        ast::BinaryOp::BitAnd => Ok(BinOp::BitAnd),
        ast::BinaryOp::BitOr => Ok(BinOp::BitOr),
        ast::BinaryOp::BitXor => Ok(BinOp::BitXor),
        ast::BinaryOp::LShift => Ok(BinOp::Shl),
        ast::BinaryOp::RShift => Ok(BinOp::Shr),
        _ => Err(anyhow!("unsupported binary operator: {:?}", op)),
    }
}

/// Resolves a member access expression, applying special conversions for known fields.
///
/// - `.length` → `.len() as f64`
/// - enum member access → `EnumName::Variant`
/// - otherwise → `object.field`
fn resolve_member_access(
    object: &Expr,
    field: &str,
    ts_obj: &ast::Expr,
    reg: &TypeRegistry,
) -> Result<Expr> {
    // Check if the TS object is an identifier referring to an enum
    if let ast::Expr::Ident(ident) = ts_obj {
        let name = ident.sym.as_ref();
        if let Some(TypeDef::Enum { .. }) = reg.get(name) {
            return Ok(Expr::Ident(format!("{name}::{field}")));
        }
    }

    // Math.PI, Math.E etc. → std::f64::consts::PI, std::f64::consts::E
    if let ast::Expr::Ident(ident) = ts_obj {
        if ident.sym.as_ref() == "Math" {
            let const_name = match field {
                "PI" => Some("PI"),
                "E" => Some("E"),
                "LN2" => Some("LN_2"),
                "LN10" => Some("LN_10"),
                "LOG2E" => Some("LOG2_E"),
                "LOG10E" => Some("LOG10_E"),
                "SQRT2" => Some("SQRT_2"),
                _ => None,
            };
            if let Some(name) = const_name {
                return Ok(Expr::Ident(format!("std::f64::consts::{name}")));
            }
        }
    }

    // .length → .len() as f64
    if field == "length" {
        let len_call = Expr::MethodCall {
            object: Box::new(object.clone()),
            method: "len".to_string(),
            args: vec![],
        };
        return Ok(Expr::Cast {
            expr: Box::new(len_call),
            target: RustType::F64,
        });
    }

    Ok(Expr::FieldAccess {
        object: Box::new(object.clone()),
        field: field.to_string(),
    })
}

/// Converts a TypeScript type assertion (`x as T`).
///
/// - Primitive types (f64, i64, bool): generates `x as T` cast
/// - Other types: passes the assertion type as `expected` to the inner expression
fn convert_ts_as_expr(
    ts_as: &ast::TsAsExpr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    use crate::transformer::types::convert_ts_type;
    match convert_ts_type(&ts_as.type_ann, &mut Vec::new(), reg) {
        Ok(target_ty) => {
            let is_primitive_cast = matches!(target_ty, RustType::F64 | RustType::Bool);
            if is_primitive_cast {
                let inner = convert_expr(&ts_as.expr, reg, Some(&target_ty), type_env)?;
                Ok(Expr::Cast {
                    expr: Box::new(inner),
                    target: target_ty,
                })
            } else {
                // Pass the assertion type as expected to help type inference
                convert_expr(&ts_as.expr, reg, expected.or(Some(&target_ty)), type_env)
            }
        }
        Err(_) => {
            // If we can't convert the type, just ignore the assertion
            convert_expr(&ts_as.expr, reg, expected, type_env)
        }
    }
}

/// Converts an optional chaining expression (`x?.y`) to `x.as_ref().map(|_v| _v.y)`.
///
/// Supports property access, method calls, and computed access.
/// Chained optional chaining (`x?.y?.z`) is handled recursively.
fn convert_opt_chain_expr(
    opt_chain: &ast::OptChainExpr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    match opt_chain.base.as_ref() {
        ast::OptChainBase::Member(member) => {
            let obj_type = resolve_expr_type(&member.obj, type_env, reg);
            let is_option = obj_type
                .as_ref()
                .is_some_and(|ty| matches!(ty, RustType::Option(_)));

            // Non-Option type with known type: plain member access
            if !is_option && obj_type.is_some() {
                return convert_member_expr(member, reg, type_env);
            }

            let object = convert_expr(&member.obj, reg, None, type_env)?;
            let body_expr = match &member.prop {
                ast::MemberProp::Ident(ident) => {
                    let field = ident.sym.to_string();
                    resolve_member_access(&Expr::Ident("_v".to_string()), &field, &member.obj, reg)?
                }
                ast::MemberProp::Computed(computed) => {
                    let index = convert_expr(&computed.expr, reg, None, type_env)?;
                    Expr::Index {
                        object: Box::new(Expr::Ident("_v".to_string())),
                        index: Box::new(index),
                    }
                }
                _ => return Err(anyhow!("unsupported optional chaining property")),
            };

            // If the field type is Option, use and_then to avoid Option<Option<T>>
            let field_type = resolve_field_type(
                obj_type.as_ref().unwrap_or(&RustType::Any),
                &member.prop,
                reg,
            );
            let method_name = if field_type.is_some_and(|ty| matches!(ty, RustType::Option(_))) {
                "and_then"
            } else {
                "map"
            };

            Ok(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(object),
                    method: "as_ref".to_string(),
                    args: vec![],
                }),
                method: method_name.to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "_v".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(body_expr)),
                }],
            })
        }
        ast::OptChainBase::Call(opt_call) => {
            // Check if the callee object is a non-Option type
            let callee_obj_type = match opt_call.callee.as_ref() {
                ast::Expr::Member(m) => resolve_expr_type(&m.obj, type_env, reg),
                ast::Expr::OptChain(oc) => match oc.base.as_ref() {
                    ast::OptChainBase::Member(m) => resolve_expr_type(&m.obj, type_env, reg),
                    _ => None,
                },
                _ => None,
            };
            let is_option = callee_obj_type
                .as_ref()
                .is_some_and(|ty| matches!(ty, RustType::Option(_)));

            let args: Vec<Expr> = opt_call
                .args
                .iter()
                .map(|arg| convert_expr(&arg.expr, reg, None, type_env))
                .collect::<Result<_>>()?;
            let (object, method) = extract_method_from_callee(&opt_call.callee, reg, type_env)?;

            // Non-Option type: plain method call
            if !is_option && callee_obj_type.is_some() {
                return Ok(Expr::MethodCall {
                    object: Box::new(object),
                    method,
                    args,
                });
            }

            let body_expr = map_method_call(Expr::Ident("_v".to_string()), &method, args);
            Ok(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(object),
                    method: "as_ref".to_string(),
                    args: vec![],
                }),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "_v".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(body_expr)),
                }],
            })
        }
    }
}

/// Extracts the object and method name from an optional call's callee.
///
/// Handles both `x.method` (`Member`) and `x?.method` (`OptChain(Member)`) patterns.
fn extract_method_from_callee(
    callee: &ast::Expr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<(Expr, String)> {
    let member = match callee {
        ast::Expr::Member(member) => member,
        ast::Expr::OptChain(opt) => match opt.base.as_ref() {
            ast::OptChainBase::Member(member) => member,
            _ => return Err(anyhow!("unsupported optional call callee")),
        },
        _ => return Err(anyhow!("unsupported optional call callee: {:?}", callee)),
    };
    let object = convert_expr(&member.obj, reg, None, type_env)?;
    let method = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported optional call property")),
    };
    Ok((object, method))
}

/// Converts a member expression (`obj.field`) to `Expr::FieldAccess`.
///
/// `this.x` becomes `self.x`.
fn convert_member_expr(
    member: &ast::MemberExpr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    // Computed property: arr[0], arr[i] → Expr::Index
    if let ast::MemberProp::Computed(computed) = &member.prop {
        let object = convert_expr(&member.obj, reg, None, type_env)?;
        let index = convert_expr(&computed.expr, reg, None, type_env)?;
        return Ok(Expr::Index {
            object: Box::new(object),
            index: Box::new(index),
        });
    }

    let field = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        ast::MemberProp::PrivateName(private) => format!("_{}", private.name),
        _ => return Err(anyhow!("unsupported member property (only identifiers)")),
    };

    // Check if accessing a field of a discriminated union enum
    if let Some(RustType::Named { name, .. }) =
        resolve_expr_type(&member.obj, type_env, reg).as_ref()
    {
        if let Some(TypeDef::Enum {
            tag_field: Some(tag),
            variant_fields,
            ..
        }) = reg.get(name)
        {
            if field == *tag {
                // Tag field → method call (e.g., s.kind() )
                let object = convert_expr(&member.obj, reg, None, type_env)?;
                return Ok(Expr::MethodCall {
                    object: Box::new(object),
                    method: tag.clone(),
                    args: vec![],
                });
            }
            // Non-tag field: if bound in TypeEnv (match arm destructuring),
            // clone the reference (match on &obj binds fields by reference)
            if type_env.get(&field).is_some() {
                return Ok(Expr::MethodCall {
                    object: Box::new(Expr::Ident(field)),
                    method: "clone".to_string(),
                    args: vec![],
                });
            }
            // Standalone field access → inline match expression
            return convert_du_standalone_field_access(
                &member.obj,
                name,
                &field,
                variant_fields,
                reg,
                type_env,
            );
        }
    }

    let object = convert_expr(&member.obj, reg, None, type_env)?;
    resolve_member_access(&object, &field, &member.obj, reg)
}

/// Discriminated union の standalone フィールドアクセスを inline match 式に変換する。
///
/// `s.radius` → `match &s { Shape::Circle { radius, .. } => radius.clone(), _ => panic!("...") }`
fn convert_du_standalone_field_access(
    obj_expr: &ast::Expr,
    enum_name: &str,
    field: &str,
    variant_fields: &std::collections::HashMap<String, Vec<(String, RustType)>>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    use crate::ir::{MatchArm, MatchPattern};

    let object = convert_expr(obj_expr, reg, None, type_env)?;
    let match_expr = Expr::Ref(Box::new(object));

    let mut arms: Vec<MatchArm> = Vec::new();

    // Create arms for variants that have this field
    for (variant_name, fields) in variant_fields {
        if fields.iter().any(|(n, _)| n == field) {
            arms.push(MatchArm {
                patterns: vec![MatchPattern::EnumVariant {
                    path: format!("{enum_name}::{variant_name}"),
                    bindings: vec![field.to_string()],
                }],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::MethodCall {
                    object: Box::new(Expr::Ident(field.to_string())),
                    method: "clone".to_string(),
                    args: vec![],
                })],
            });
        }
    }

    // Add wildcard arm with panic
    arms.push(MatchArm {
        patterns: vec![MatchPattern::Wildcard],
        guard: None,
        body: vec![Stmt::TailExpr(Expr::MacroCall {
            name: "panic".to_string(),
            args: vec![Expr::StringLit(format!(
                "variant does not have field '{field}'"
            ))],
            use_debug: vec![false],
        })],
    });

    Ok(Expr::Match {
        expr: Box::new(match_expr),
        arms,
    })
}

/// Converts an update expression (`i++`, `i--`, `++i`, `--i`) to `Expr::Assign`.
///
/// Both prefix and postfix forms are converted to the same assignment:
/// - `i++` / `++i` → `i = i + 1.0`
/// - `i--` / `--i` → `i = i - 1.0`
///
/// Note: In statement context, prefix/postfix distinction is irrelevant.
/// In expression context where the return value matters (e.g., `while (i--)`),
/// the prefix/postfix semantics differ, but this is not yet handled.
fn convert_update_expr(up: &ast::UpdateExpr) -> Result<Expr> {
    let name = match up.arg.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported update expression target")),
    };
    let op = match up.op {
        ast::UpdateOp::PlusPlus => BinOp::Add,
        ast::UpdateOp::MinusMinus => BinOp::Sub,
    };
    let assign = Stmt::Expr(Expr::Assign {
        target: Box::new(Expr::Ident(name.clone())),
        value: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident(name.clone())),
            op,
            right: Box::new(Expr::NumberLit(1.0)),
        }),
    });

    if up.prefix {
        // Prefix: ++i → { i = i + 1.0; i }
        Ok(Expr::Block(vec![assign, Stmt::TailExpr(Expr::Ident(name))]))
    } else {
        // Postfix: i++ → { let _old = i; i = i + 1.0; _old }
        Ok(Expr::Block(vec![
            Stmt::Let {
                mutable: false,
                name: "_old".to_string(),
                ty: None,
                init: Some(Expr::Ident(name)),
            },
            assign,
            Stmt::TailExpr(Expr::Ident("_old".to_string())),
        ]))
    }
}

/// Converts a function expression to `Expr::Closure`.
///
/// Function expressions (`function(x) { ... }` or `function name(x) { ... }`)
/// are treated identically to arrow functions — the optional name is ignored.
fn convert_fn_expr(fn_expr: &ast::FnExpr, reg: &TypeRegistry, type_env: &TypeEnv) -> Result<Expr> {
    let func = &fn_expr.function;

    // Convert parameters — reuse the same logic as arrow functions
    let mut params = Vec::new();
    let mut expansion_stmts = Vec::new();
    for param in &func.params {
        match &param.pat {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                let rust_type = ident
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
                    .transpose()?;
                params.push(Param {
                    name,
                    ty: rust_type,
                });
            }
            ast::Pat::Object(obj_pat) => {
                let (param, stmts) =
                    crate::transformer::functions::convert_object_destructuring_param(
                        obj_pat, reg,
                    )?;
                params.push(param);
                expansion_stmts.extend(stmts);
            }
            ast::Pat::Assign(assign) => match assign.left.as_ref() {
                ast::Pat::Ident(ident) => {
                    let name = ident.id.sym.to_string();
                    let inner_type = ident
                        .type_ann
                        .as_ref()
                        .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
                        .transpose()?
                        .ok_or_else(|| anyhow!("default parameter requires a type annotation"))?;
                    let option_type = RustType::Option(Box::new(inner_type));
                    let (default_expr, use_unwrap_or_default) =
                        crate::transformer::functions::convert_default_value(&assign.right)?;
                    let unwrap_call = if use_unwrap_or_default {
                        Expr::MethodCall {
                            object: Box::new(Expr::Ident(name.clone())),
                            method: "unwrap_or_default".to_string(),
                            args: vec![],
                        }
                    } else {
                        Expr::MethodCall {
                            object: Box::new(Expr::Ident(name.clone())),
                            method: "unwrap_or".to_string(),
                            args: vec![default_expr.unwrap()],
                        }
                    };
                    expansion_stmts.push(Stmt::Let {
                        mutable: false,
                        name: name.clone(),
                        ty: None,
                        init: Some(unwrap_call),
                    });
                    params.push(Param {
                        name,
                        ty: Some(option_type),
                    });
                }
                _ => return Err(anyhow!("unsupported function expression default parameter")),
            },
            ast::Pat::Rest(rest) => {
                if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                    let name = ident.id.sym.to_string();
                    let type_ann = rest.type_ann.as_ref().or(ident.type_ann.as_ref());
                    let rust_type = type_ann
                        .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
                        .transpose()?;
                    params.push(Param {
                        name,
                        ty: rust_type,
                    });
                } else {
                    return Err(anyhow!("unsupported function expression rest parameter"));
                }
            }
            _ => return Err(anyhow!("unsupported function expression parameter pattern")),
        }
    }

    let return_type = func
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
        .transpose()?;

    // void → None
    let return_type = return_type.and_then(|ty| {
        if matches!(ty, RustType::Unit) {
            None
        } else {
            Some(ty)
        }
    });

    let body = match &func.body {
        Some(block) => {
            let mut inner_env = type_env.clone();
            let mut stmts = expansion_stmts;
            for stmt in &block.stmts {
                stmts.extend(convert_stmt(
                    stmt,
                    reg,
                    return_type.as_ref(),
                    &mut inner_env,
                )?);
            }
            convert_last_return_to_tail(&mut stmts);
            ClosureBody::Block(stmts)
        }
        None => ClosureBody::Block(expansion_stmts),
    };

    Ok(Expr::Closure {
        params,
        return_type,
        body,
    })
}

/// Converts an assignment expression (`target = value`) to `Expr::Assign`.
fn convert_assign_expr(
    assign: &ast::AssignExpr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let target = match &assign.left {
        ast::AssignTarget::Simple(simple) => match simple {
            ast::SimpleAssignTarget::Member(member) => convert_member_expr(member, reg, type_env)?,
            ast::SimpleAssignTarget::Ident(ident) => Expr::Ident(ident.id.sym.to_string()),
            _ => return Err(anyhow!("unsupported assignment target")),
        },
        _ => return Err(anyhow!("unsupported assignment target pattern")),
    };
    let right = convert_expr(&assign.right, reg, None, type_env)?;

    // ??= (nullish coalescing assignment): x ??= y → x.get_or_insert_with(|| y)
    if assign.op == ast::AssignOp::NullishAssign {
        return Ok(Expr::MethodCall {
            object: Box::new(target),
            method: "get_or_insert_with".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: crate::ir::ClosureBody::Expr(Box::new(right)),
            }],
        });
    }

    // For compound assignment (+=, -=, *=, /=), desugar to target = target op value
    let value = match assign.op {
        ast::AssignOp::Assign => right,
        ast::AssignOp::AddAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Add,
            right: Box::new(right),
        },
        ast::AssignOp::SubAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Sub,
            right: Box::new(right),
        },
        ast::AssignOp::MulAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Mul,
            right: Box::new(right),
        },
        ast::AssignOp::DivAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Div,
            right: Box::new(right),
        },
        ast::AssignOp::ModAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Mod,
            right: Box::new(right),
        },
        ast::AssignOp::BitAndAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::BitAnd,
            right: Box::new(right),
        },
        ast::AssignOp::BitOrAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::BitOr,
            right: Box::new(right),
        },
        ast::AssignOp::BitXorAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::BitXor,
            right: Box::new(right),
        },
        ast::AssignOp::LShiftAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Shl,
            right: Box::new(right),
        },
        ast::AssignOp::RShiftAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::Shr,
            right: Box::new(right),
        },
        ast::AssignOp::AndAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::LogicalAnd,
            right: Box::new(right),
        },
        ast::AssignOp::OrAssign => Expr::BinaryOp {
            left: Box::new(target.clone()),
            op: BinOp::LogicalOr,
            right: Box::new(right),
        },
        _ => return Err(anyhow!("unsupported compound assignment operator")),
    };
    Ok(Expr::Assign {
        target: Box::new(target),
        value: Box::new(value),
    })
}

/// Converts an arrow expression into an IR [`Expr::Closure`].
///
/// - Expression body: `(x: number) => x + 1` → `|x: f64| x + 1`
/// - Block body: `(x: number) => { return x + 1; }` → `|x: f64| { x + 1 }`
///
/// When `resilient` is true, unsupported types fall back to [`RustType::Any`] and
/// the error message is appended to `fallback_warnings`.
/// `override_return_type` allows callers to inject a return type from an external source
/// (e.g., variable type annotation `const f: FnType = () => ...`). When provided and the
/// arrow has no explicit return type annotation, this type is used for the body conversion.
pub fn convert_arrow_expr(
    arrow: &ast::ArrowExpr,
    reg: &TypeRegistry,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    convert_arrow_expr_with_return_type(
        arrow,
        reg,
        resilient,
        fallback_warnings,
        type_env,
        None,
        None,
    )
}

/// Inner implementation of arrow expression conversion with optional type overrides.
///
/// `override_return_type` allows callers to inject a return type from an external source
/// (e.g., variable type annotation `const f: FnType = () => ...`).
/// `override_param_types` allows callers to inject parameter types from an external source
/// (e.g., variable type annotation `const f: (x: number) => void = (x) => ...`).
pub(crate) fn convert_arrow_expr_with_return_type(
    arrow: &ast::ArrowExpr,
    reg: &TypeRegistry,
    resilient: bool,
    fallback_warnings: &mut Vec<String>,
    type_env: &TypeEnv,
    override_return_type: Option<&RustType>,
    override_param_types: Option<&[RustType]>,
) -> Result<Expr> {
    let mut params = Vec::new();
    let mut expansion_stmts = Vec::new();
    for (i, param) in arrow.params.iter().enumerate() {
        match param {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                let rust_type = ident
                    .type_ann
                    .as_ref()
                    .map(|ann| {
                        convert_ts_type_with_fallback(
                            &ann.type_ann,
                            resilient,
                            fallback_warnings,
                            &mut Vec::new(),
                            reg,
                        )
                    })
                    .transpose()?;
                // If no direct annotation, try override from variable type annotation
                let rust_type = rust_type
                    .or_else(|| override_param_types.and_then(|types| types.get(i).cloned()));
                params.push(Param {
                    name,
                    ty: rust_type,
                });
            }
            ast::Pat::Object(obj_pat) => {
                let (param, stmts) =
                    crate::transformer::functions::convert_object_destructuring_param(
                        obj_pat, reg,
                    )?;
                params.push(param);
                expansion_stmts.extend(stmts);
            }
            ast::Pat::Assign(assign) => {
                // Default parameter: (x: number = 0) => ...
                // Extract inner ident and type
                match assign.left.as_ref() {
                    ast::Pat::Ident(ident) => {
                        let name = ident.id.sym.to_string();
                        let inner_type = ident
                            .type_ann
                            .as_ref()
                            .map(|ann| {
                                convert_ts_type_with_fallback(
                                    &ann.type_ann,
                                    resilient,
                                    fallback_warnings,
                                    &mut Vec::new(),
                                    reg,
                                )
                            })
                            .transpose()?
                            .ok_or_else(|| {
                                anyhow!("default parameter requires a type annotation")
                            })?;
                        let option_type = RustType::Option(Box::new(inner_type));
                        let (default_expr, use_unwrap_or_default) =
                            crate::transformer::functions::convert_default_value(&assign.right)?;
                        let unwrap_call = if use_unwrap_or_default {
                            Expr::MethodCall {
                                object: Box::new(Expr::Ident(name.clone())),
                                method: "unwrap_or_default".to_string(),
                                args: vec![],
                            }
                        } else {
                            Expr::MethodCall {
                                object: Box::new(Expr::Ident(name.clone())),
                                method: "unwrap_or".to_string(),
                                args: vec![default_expr.unwrap()],
                            }
                        };
                        expansion_stmts.push(Stmt::Let {
                            mutable: false,
                            name: name.clone(),
                            ty: None,
                            init: Some(unwrap_call),
                        });
                        params.push(Param {
                            name,
                            ty: Some(option_type),
                        });
                    }
                    _ => return Err(anyhow!("unsupported arrow default parameter pattern")),
                }
            }
            ast::Pat::Rest(rest) => {
                // ...args: T[] → args: Vec<T>
                if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                    let name = ident.id.sym.to_string();
                    // Type annotation may be on RestPat itself or on the inner BindingIdent
                    let type_ann = rest.type_ann.as_ref().or(ident.type_ann.as_ref());
                    let rust_type = type_ann
                        .map(|ann| {
                            convert_ts_type_with_fallback(
                                &ann.type_ann,
                                resilient,
                                fallback_warnings,
                                &mut Vec::new(),
                                reg,
                            )
                        })
                        .transpose()?;
                    params.push(Param {
                        name,
                        ty: rust_type,
                    });
                } else {
                    return Err(anyhow!("unsupported arrow rest parameter pattern"));
                }
            }
            _ => return Err(anyhow!("unsupported arrow parameter pattern")),
        }
    }

    // Arrow's explicit return type annotation takes priority;
    // fall back to override_return_type from variable type annotation
    let return_type = arrow
        .return_type
        .as_ref()
        .map(|ann| {
            convert_ts_type_with_fallback(
                &ann.type_ann,
                resilient,
                fallback_warnings,
                &mut Vec::new(),
                reg,
            )
        })
        .transpose()?
        .or_else(|| override_return_type.cloned());

    let body = if expansion_stmts.is_empty() {
        match arrow.body.as_ref() {
            ast::BlockStmtOrExpr::Expr(expr) => {
                let ir_expr = convert_expr(expr, reg, return_type.as_ref(), type_env)?;
                ClosureBody::Expr(Box::new(ir_expr))
            }
            ast::BlockStmtOrExpr::BlockStmt(block) => {
                let mut inner_env = type_env.clone();
                let mut stmts = Vec::new();
                for stmt in &block.stmts {
                    stmts.extend(convert_stmt(
                        stmt,
                        reg,
                        return_type.as_ref(),
                        &mut inner_env,
                    )?);
                }
                convert_last_return_to_tail(&mut stmts);
                ClosureBody::Block(stmts)
            }
        }
    } else {
        // When we have expansion stmts, the body must be a Block
        let mut body_stmts = expansion_stmts;
        match arrow.body.as_ref() {
            ast::BlockStmtOrExpr::Expr(expr) => {
                let ir_expr = convert_expr(expr, reg, return_type.as_ref(), type_env)?;
                body_stmts.push(Stmt::Return(Some(ir_expr)));
            }
            ast::BlockStmtOrExpr::BlockStmt(block) => {
                let mut inner_env = type_env.clone();
                for stmt in &block.stmts {
                    body_stmts.extend(convert_stmt(
                        stmt,
                        reg,
                        return_type.as_ref(),
                        &mut inner_env,
                    )?);
                }
                convert_last_return_to_tail(&mut body_stmts);
            }
        }
        ClosureBody::Block(body_stmts)
    };

    Ok(Expr::Closure {
        params,
        return_type,
        body,
    })
}

/// Converts a function/method call expression.
///
/// - `foo(x, y)` → `Expr::FnCall { name: "foo", args }`
/// - `obj.method(x)` → `Expr::MethodCall { object, method, args }`
fn convert_call_expr(call: &ast::CallExpr, reg: &TypeRegistry, type_env: &TypeEnv) -> Result<Expr> {
    match call.callee {
        ast::Callee::Expr(ref callee) => match callee.as_ref() {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();

                // parseInt(s) → s.parse::<f64>().unwrap()
                // parseFloat(s) → s.parse::<f64>().unwrap()
                // isNaN(x) → x.is_nan()
                if let Some(result) = convert_global_builtin(&fn_name, &call.args, reg, type_env)? {
                    return Ok(result);
                }

                // Look up function parameter types from the registry or TypeEnv
                let typeenv_params: Vec<(String, RustType)>;
                let mut has_rest = false;
                let param_types: Option<&[(String, RustType)]> = if let Some(TypeDef::Function {
                    params,
                    has_rest: rest,
                    ..
                }) = reg.get(&fn_name)
                {
                    has_rest = *rest;
                    Some(params.as_slice())
                } else if let Some(RustType::Fn { params, .. }) = type_env.get(&fn_name) {
                    typeenv_params = params
                        .iter()
                        .enumerate()
                        .map(|(i, ty)| (format!("_p{i}"), ty.clone()))
                        .collect();
                    Some(typeenv_params.as_slice())
                } else {
                    None
                };
                let args =
                    convert_call_args_with_types(&call.args, reg, param_types, has_rest, type_env)?;
                Ok(Expr::FnCall {
                    name: fn_name,
                    args,
                })
            }
            ast::Expr::Member(member) => {
                let method = match &member.prop {
                    ast::MemberProp::Ident(ident) => ident.sym.to_string(),
                    ast::MemberProp::PrivateName(private) => format!("_{}", private.name),
                    _ => return Err(anyhow!("unsupported call target member property")),
                };

                if let ast::Expr::Ident(obj_ident) = member.obj.as_ref() {
                    // console.log/error/warn → println!/eprintln!
                    if obj_ident.sym.as_ref() == "console" {
                        let macro_name = match method.as_str() {
                            "log" => "println",
                            "error" | "warn" => "eprintln",
                            _ => return Err(anyhow!("unsupported console method: {}", method)),
                        };
                        let args = convert_call_args(&call.args, reg, type_env)?;
                        let use_debug = call
                            .args
                            .iter()
                            .map(|arg| {
                                let ty = resolve_expr_type(&arg.expr, type_env, reg);
                                needs_debug_format(ty.as_ref())
                            })
                            .collect();
                        return Ok(Expr::MacroCall {
                            name: macro_name.to_string(),
                            args,
                            use_debug,
                        });
                    }

                    // Math.method(args) → first_arg.method(rest_args)
                    if obj_ident.sym.as_ref() == "Math" {
                        return convert_math_call(&method, &call.args, reg, type_env);
                    }

                    // Number.isNaN(x) → x.is_nan(), Number.isFinite(x) → x.is_finite()
                    if obj_ident.sym.as_ref() == "Number" {
                        return convert_number_static_call(&method, &call.args, reg, type_env);
                    }
                }

                let object = convert_expr(&member.obj, reg, None, type_env)?;
                // Look up method parameter types from the object's type
                let method_params = resolve_expr_type(&member.obj, type_env, reg).and_then(|ty| {
                    if let RustType::Named { name, .. } = &ty {
                        if let Some(TypeDef::Struct { methods, .. }) = reg.get(name) {
                            return methods.get(&method).cloned();
                        }
                    }
                    None
                });
                let args = convert_call_args_with_types(
                    &call.args,
                    reg,
                    method_params.as_deref(),
                    false,
                    type_env,
                )?;
                let method_call = map_method_call(object, &method, args);
                Ok(method_call)
            }
            // Unwrap parenthesized expression and retry: (foo)(args) → foo(args)
            ast::Expr::Paren(paren) => {
                let unwrapped_call = ast::CallExpr {
                    callee: ast::Callee::Expr(paren.expr.clone()),
                    args: call.args.clone(),
                    span: call.span,
                    ctxt: call.ctxt,
                    type_args: call.type_args.clone(),
                };
                convert_call_expr(&unwrapped_call, reg, type_env)
            }
            // Chained call: f(x)(y) → { let _f = f(x); _f(y) }
            ast::Expr::Call(inner_call) => {
                let inner_result = convert_call_expr(inner_call, reg, type_env)?;
                let args = convert_call_args(&call.args, reg, type_env)?;
                Ok(Expr::Block(vec![
                    Stmt::Let {
                        name: "_f".to_string(),
                        mutable: false,
                        ty: None,
                        init: Some(inner_result),
                    },
                    Stmt::TailExpr(Expr::FnCall {
                        name: "_f".to_string(),
                        args,
                    }),
                ]))
            }
            _ => Err(anyhow!("unsupported call target expression")),
        },
        ast::Callee::Super(_) => {
            let args = convert_call_args(&call.args, reg, type_env)?;
            Ok(Expr::FnCall {
                name: "super".to_string(),
                args,
            })
        }
        _ => Err(anyhow!("unsupported callee type")),
    }
}

/// Maps TypeScript method names to Rust equivalents.
///
/// Handles simple renames, methods that need wrapping (e.g., `trim` → `trim().to_string()`),
/// methods that need chaining (e.g., `split` → `split(s).collect::<Vec<&str>>()`),
/// and array methods (e.g., `reduce` → `iter().fold()`, `indexOf` → `iter().position()`,
/// `slice` → `[a..b].to_vec()`, `splice` → `drain().collect()`).
fn map_method_call(object: Expr, method: &str, args: Vec<Expr>) -> Expr {
    match method {
        // Simple name mappings
        "includes" => Expr::MethodCall {
            object: Box::new(object),
            method: "contains".to_string(),
            args: args.into_iter().map(|a| Expr::Ref(Box::new(a))).collect(),
        },
        "startsWith" => Expr::MethodCall {
            object: Box::new(object),
            method: "starts_with".to_string(),
            args,
        },
        "endsWith" => Expr::MethodCall {
            object: Box::new(object),
            method: "ends_with".to_string(),
            args,
        },
        "toLowerCase" => Expr::MethodCall {
            object: Box::new(object),
            method: "to_lowercase".to_string(),
            args,
        },
        "toUpperCase" => Expr::MethodCall {
            object: Box::new(object),
            method: "to_uppercase".to_string(),
            args,
        },
        // trim() returns &str, wrap with .to_string()
        "trim" => Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(object),
                method: "trim".to_string(),
                args,
            }),
            method: "to_string".to_string(),
            args: vec![],
        },
        // split() returns an iterator, chain .collect::<Vec<&str>>()
        "split" => Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(object),
                method: "split".to_string(),
                args,
            }),
            method: "collect::<Vec<&str>>".to_string(),
            args: vec![],
        },
        // Iterator methods that collect: .map(fn) / .filter(fn) → .iter().cloned().method(fn).collect()
        // .cloned() converts &T → T, giving closures value semantics matching TypeScript behavior.
        // TODO: clone 削減 — Copy 型には .copied()、不要な clone は所有権解析で除去
        "map" | "filter" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(object),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            };
            let args = args
                .into_iter()
                .map(strip_closure_type_annotations)
                .collect();
            let method_call = Expr::MethodCall {
                object: Box::new(iter_call),
                method: method.to_string(),
                args,
            };
            Expr::MethodCall {
                object: Box::new(method_call),
                method: "collect::<Vec<_>>".to_string(),
                args: vec![],
            }
        }
        // Iterator methods without collect: .find(fn), .some(fn), .every(fn)
        "find" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(object),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            };
            let args = args
                .into_iter()
                .map(strip_closure_type_annotations)
                .collect();
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "find".to_string(),
                args,
            }
        }
        "some" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(object),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            };
            let args = args
                .into_iter()
                .map(strip_closure_type_annotations)
                .collect();
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "any".to_string(),
                args,
            }
        }
        "every" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(object),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            };
            let args = args
                .into_iter()
                .map(strip_closure_type_annotations)
                .collect();
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "all".to_string(),
                args,
            }
        }
        // slice(start, end) → [start..end].to_vec()
        "slice" => {
            if args.len() != 2 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let mut iter = args.into_iter();
            let start = iter.next().unwrap();
            let end = iter.next().unwrap();
            Expr::MethodCall {
                object: Box::new(Expr::Index {
                    object: Box::new(object),
                    index: Box::new(Expr::Range {
                        start: Some(Box::new(start)),
                        end: Some(Box::new(end)),
                    }),
                }),
                method: "to_vec".to_string(),
                args: vec![],
            }
        }
        // splice(start, count) → .drain(start..start+count).collect::<Vec<_>>()
        // Pre-compute end when both are numeric literals to avoid float range issues
        "splice" => {
            if args.len() != 2 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let mut iter = args.into_iter();
            let start = iter.next().unwrap();
            let count = iter.next().unwrap();
            // Pre-compute end = start + count when both are numeric literals
            let end = match (&start, &count) {
                (Expr::NumberLit(s), Expr::NumberLit(c)) => Expr::NumberLit(s + c),
                _ => Expr::BinaryOp {
                    left: Box::new(start.clone()),
                    op: BinOp::Add,
                    right: Box::new(count),
                },
            };
            let drain_call = Expr::MethodCall {
                object: Box::new(object),
                method: "drain".to_string(),
                args: vec![Expr::Range {
                    start: Some(Box::new(start)),
                    end: Some(Box::new(end)),
                }],
            };
            Expr::MethodCall {
                object: Box::new(drain_call),
                method: "collect::<Vec<_>>".to_string(),
                args: vec![],
            }
        }
        // sort() → .sort_by(|a, b| a.partial_cmp(b).unwrap())
        // sort(fn) → .sort_by(fn) with type annotations stripped
        "sort" => {
            if args.is_empty() {
                // f64 doesn't implement Ord, so use partial_cmp
                let cmp_closure = Expr::Closure {
                    params: vec![
                        Param {
                            name: "a".to_string(),
                            ty: None,
                        },
                        Param {
                            name: "b".to_string(),
                            ty: None,
                        },
                    ],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                        object: Box::new(Expr::MethodCall {
                            object: Box::new(Expr::Ident("a".to_string())),
                            method: "partial_cmp".to_string(),
                            args: vec![Expr::Ident("b".to_string())],
                        }),
                        method: "unwrap".to_string(),
                        args: vec![],
                    })),
                };
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: "sort_by".to_string(),
                    args: vec![cmp_closure],
                };
            }
            // With comparator: strip type annotations and wrap body in partial_cmp
            // TS comparator returns number (negative/zero/positive), Rust needs Ordering
            let args = args
                .into_iter()
                .map(|arg| {
                    let stripped = strip_closure_type_annotations(arg);
                    wrap_sort_comparator_body(stripped)
                })
                .collect();
            Expr::MethodCall {
                object: Box::new(object),
                method: "sort_by".to_string(),
                args,
            }
        }
        // indexOf(x) → .iter().position(|item| *item == x).map(|i| i as f64).unwrap_or(-1.0)
        "indexOf" => {
            if args.len() != 1 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let search_value = args.into_iter().next().unwrap();
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            let position_call = Expr::MethodCall {
                object: Box::new(iter_call),
                method: "position".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "item".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Deref(Box::new(Expr::Ident("item".to_string())))),
                        op: BinOp::Eq,
                        right: Box::new(search_value),
                    })),
                }],
            };
            // .map(|i| i as f64).unwrap_or(-1.0)
            let map_call = Expr::MethodCall {
                object: Box::new(position_call),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "i".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::Cast {
                        expr: Box::new(Expr::Ident("i".to_string())),
                        target: RustType::F64,
                    })),
                }],
            };
            Expr::MethodCall {
                object: Box::new(map_call),
                method: "unwrap_or".to_string(),
                args: vec![Expr::NumberLit(-1.0)],
            }
        }
        // reduce(fn, init) → .iter().fold(init, fn)
        // Strip closure param type annotations (iter() yields &T, Rust infers correctly)
        "reduce" => {
            if args.len() != 2 {
                return Expr::MethodCall {
                    object: Box::new(object),
                    method: method.to_string(),
                    args,
                };
            }
            let mut iter = args.into_iter();
            let callback = strip_closure_type_annotations(iter.next().unwrap());
            let init = iter.next().unwrap();
            let iter_call = Expr::MethodCall {
                object: Box::new(object),
                method: "iter".to_string(),
                args: vec![],
            };
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "fold".to_string(),
                args: vec![init, callback],
            }
        }
        // forEach → .iter().cloned().for_each(fn)
        "forEach" => {
            let iter_call = Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(object),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            };
            let args = args
                .into_iter()
                .map(strip_closure_type_annotations)
                .collect();
            Expr::MethodCall {
                object: Box::new(iter_call),
                method: "for_each".to_string(),
                args,
            }
        }
        // join(sep) → join(&sep) — Rust's join takes &str, not String
        "join" => {
            let args = args
                .into_iter()
                .map(|arg| match arg {
                    // Variable: &sep
                    Expr::Ident(name) => Expr::Ref(Box::new(Expr::Ident(name))),
                    // String literal: already &str in Rust, pass through
                    lit @ Expr::StringLit(_) => lit,
                    // Other expressions: call .as_str() — but wrap in parens via method call
                    other => Expr::MethodCall {
                        object: Box::new(other),
                        method: "as_str".to_string(),
                        args: vec![],
                    },
                })
                .collect();
            Expr::MethodCall {
                object: Box::new(object),
                method: "join".to_string(),
                args,
            }
        }
        // Regex-aware replace: str.replace(/p/g, "r") → regex.replace_all(&str, "r")
        "replace" if matches!(args.first(), Some(Expr::Regex { .. })) => {
            let mut args_iter = args.into_iter();
            let regex_expr = args_iter.next().unwrap();
            let remaining_args: Vec<Expr> = args_iter.collect();
            if let Expr::Regex {
                pattern, global, ..
            } = regex_expr
            {
                let regex_obj = build_regex_new(pattern);
                let method_name = if global { "replace_all" } else { "replace" };
                let mut call_args = vec![Expr::Ref(Box::new(object))];
                call_args.extend(remaining_args);
                // Regex::replace/replace_all returns Cow<str>, convert to String
                Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(regex_obj),
                        method: method_name.to_string(),
                        args: call_args,
                    }),
                    method: "to_string".to_string(),
                    args: vec![],
                }
            } else {
                unreachable!()
            }
        }
        // str.match(regex) → regex.find(&str) or regex.find_iter(&str) depending on g flag
        "match" if matches!(args.first(), Some(Expr::Regex { .. })) => {
            let regex_expr = args.into_iter().next().unwrap();
            if let Expr::Regex {
                pattern, global, ..
            } = regex_expr
            {
                let regex_obj = build_regex_new(pattern);
                let method_name = if global { "find_iter" } else { "find" };
                Expr::MethodCall {
                    object: Box::new(regex_obj),
                    method: method_name.to_string(),
                    args: vec![Expr::Ref(Box::new(object))],
                }
            } else {
                unreachable!()
            }
        }
        // regex.test(str) → regex.is_match(&str)
        "test" if matches!(&object, Expr::Regex { .. }) => {
            if let Expr::Regex { pattern, .. } = object {
                let regex_obj = build_regex_new(pattern);
                Expr::MethodCall {
                    object: Box::new(regex_obj),
                    method: "is_match".to_string(),
                    args: args.into_iter().map(|a| Expr::Ref(Box::new(a))).collect(),
                }
            } else {
                unreachable!()
            }
        }
        // regex.exec(str) → regex.captures(&str)
        "exec" if matches!(&object, Expr::Regex { .. }) => {
            if let Expr::Regex { pattern, .. } = object {
                let regex_obj = build_regex_new(pattern);
                Expr::MethodCall {
                    object: Box::new(regex_obj),
                    method: "captures".to_string(),
                    args: args.into_iter().map(|a| Expr::Ref(Box::new(a))).collect(),
                }
            } else {
                unreachable!()
            }
        }
        // str.replaceAll("a", "b") → str.replace("a", "b") (Rust replace replaces all)
        "replaceAll" => Expr::MethodCall {
            object: Box::new(object),
            method: "replace".to_string(),
            args,
        },
        // String replace: str.replace("a", "b") → str.replacen("a", "b", 1)
        // TS replaces only the first occurrence; Rust's replace() replaces all.
        "replace" => {
            let mut new_args = args;
            new_args.push(Expr::IntLit(1));
            Expr::MethodCall {
                object: Box::new(object),
                method: "replacen".to_string(),
                args: new_args,
            }
        }
        // No mapping needed — pass through unchanged
        _ => Expr::MethodCall {
            object: Box::new(object),
            method: method.to_string(),
            args,
        },
    }
}

/// Builds a `Regex::new(r"pattern").unwrap()` expression from a pattern string.
///
/// Returns `Expr::Regex` which the generator renders as `Regex::new(r"pattern").unwrap()`.
fn build_regex_new(pattern: String) -> Expr {
    Expr::Regex {
        pattern,
        global: false,
        sticky: false,
    }
}

/// Strips type annotations from closure parameters and return type.
///
/// Used for iterator method closures (`fold`, `sort_by`, etc.) where Rust's type
/// inference handles `&T` references correctly without explicit annotations.
fn strip_closure_type_annotations(expr: Expr) -> Expr {
    match expr {
        Expr::Closure {
            params,
            return_type: _,
            body,
        } => Expr::Closure {
            params: params
                .into_iter()
                .map(|p| Param {
                    name: p.name,
                    ty: None,
                })
                .collect(),
            return_type: None,
            body,
        },
        other => other,
    }
}

/// Wraps a TS sort comparator closure body with `partial_cmp(&0.0).unwrap()`.
///
/// TS comparators return a number (negative/zero/positive), but Rust's `sort_by`
/// expects `Ordering`. This wraps the body expression: `body` → `body.partial_cmp(&0.0).unwrap()`.
fn wrap_sort_comparator_body(expr: Expr) -> Expr {
    match expr {
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            let new_body = match body {
                ClosureBody::Expr(inner) => {
                    let wrapped = Expr::MethodCall {
                        object: Box::new(Expr::MethodCall {
                            object: inner,
                            method: "partial_cmp".to_string(),
                            args: vec![Expr::Ref(Box::new(Expr::NumberLit(0.0)))],
                        }),
                        method: "unwrap".to_string(),
                        args: vec![],
                    };
                    ClosureBody::Expr(Box::new(wrapped))
                }
                other => other, // Block bodies — don't attempt to wrap
            };
            Expr::Closure {
                params,
                return_type,
                body: new_body,
            }
        }
        other => other,
    }
}

/// Converts global built-in functions (`parseInt`, `parseFloat`, `isNaN`) to Rust equivalents.
///
/// Returns `Ok(Some(expr))` if the function is a known built-in, `Ok(None)` otherwise.
fn convert_global_builtin(
    fn_name: &str,
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Option<Expr>> {
    match fn_name {
        // parseInt(s) → s.parse::<f64>().unwrap_or(f64::NAN)
        "parseInt" => {
            let converted = convert_call_args(args, reg, type_env)?;
            if converted.len() != 1 {
                return Err(anyhow!("parseInt expects 1 argument"));
            }
            let arg = converted.into_iter().next().unwrap();
            Ok(Some(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(arg),
                    method: "parse::<f64>".to_string(),
                    args: vec![],
                }),
                method: "unwrap_or".to_string(),
                args: vec![Expr::Ident("f64::NAN".to_string())],
            }))
        }
        // parseFloat(s) → s.parse::<f64>().unwrap_or(f64::NAN)
        "parseFloat" => {
            let converted = convert_call_args(args, reg, type_env)?;
            if converted.len() != 1 {
                return Err(anyhow!("parseFloat expects 1 argument"));
            }
            let arg = converted.into_iter().next().unwrap();
            Ok(Some(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(arg),
                    method: "parse::<f64>".to_string(),
                    args: vec![],
                }),
                method: "unwrap_or".to_string(),
                args: vec![Expr::Ident("f64::NAN".to_string())],
            }))
        }
        // isNaN(x) → x.is_nan()
        "isNaN" => {
            let converted = convert_call_args(args, reg, type_env)?;
            if converted.len() != 1 {
                return Err(anyhow!("isNaN expects 1 argument"));
            }
            let arg = converted.into_iter().next().unwrap();
            Ok(Some(Expr::MethodCall {
                object: Box::new(arg),
                method: "is_nan".to_string(),
                args: vec![],
            }))
        }
        _ => Ok(None),
    }
}

/// Converts `Number.method(x)` static calls to Rust `f64` method calls.
///
/// - `Number.isNaN(x)` → `x.is_nan()`
/// - `Number.isFinite(x)` → `x.is_finite()`
/// - `Number.isInteger(x)` → `x.fract() == 0.0`
fn convert_number_static_call(
    method: &str,
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let converted = convert_call_args(args, reg, type_env)?;
    if converted.len() != 1 {
        return Err(anyhow!("Number.{method} expects 1 argument"));
    }
    let arg = converted.into_iter().next().unwrap();
    match method {
        "isNaN" | "isFinite" => {
            let rust_method = match method {
                "isNaN" => "is_nan",
                "isFinite" => "is_finite",
                _ => unreachable!(),
            };
            Ok(Expr::MethodCall {
                object: Box::new(arg),
                method: rust_method.to_string(),
                args: vec![],
            })
        }
        // Number.isInteger(x) → x.fract() == 0.0
        "isInteger" => Ok(Expr::BinaryOp {
            left: Box::new(Expr::MethodCall {
                object: Box::new(arg),
                method: "fract".to_string(),
                args: vec![],
            }),
            op: BinOp::Eq,
            right: Box::new(Expr::NumberLit(0.0)),
        }),
        _ => Err(anyhow!("unsupported Number method: {method}")),
    }
}

/// Converts `Math.method(args)` to Rust `f64` method calls.
///
/// - 1-arg methods (same name): `Math.floor(x)` → `x.floor()`, `Math.trunc(x)` → `x.trunc()`
/// - 1-arg methods (renamed): `Math.sign(x)` → `x.signum()`, `Math.log(x)` → `x.ln()`
/// - 2-arg methods: `Math.max(a, b)` → `a.max(b)`
/// - `Math.pow(x, y)` → `x.powf(y)`
fn convert_math_call(
    method: &str,
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let converted_args = convert_call_args(args, reg, type_env)?;
    match method {
        // 1-arg methods: first arg becomes receiver (same name)
        "floor" | "ceil" | "round" | "abs" | "sqrt" | "trunc" => {
            if converted_args.len() != 1 {
                return Err(anyhow!("Math.{method} expects 1 argument"));
            }
            let receiver = converted_args.into_iter().next().unwrap();
            Ok(Expr::MethodCall {
                object: Box::new(receiver),
                method: method.to_string(),
                args: vec![],
            })
        }
        // 1-arg methods with name mapping: first arg becomes receiver
        "sign" | "log" => {
            if converted_args.len() != 1 {
                return Err(anyhow!("Math.{method} expects 1 argument"));
            }
            let receiver = converted_args.into_iter().next().unwrap();
            let rust_method = match method {
                "sign" => "signum",
                "log" => "ln",
                _ => unreachable!(),
            };
            Ok(Expr::MethodCall {
                object: Box::new(receiver),
                method: rust_method.to_string(),
                args: vec![],
            })
        }
        // variadic methods: chain calls for 2+ args: Math.max(a,b,c) → a.max(b).max(c)
        "max" | "min" => {
            if converted_args.len() < 2 {
                return Err(anyhow!("Math.{method} expects at least 2 arguments"));
            }
            let mut iter = converted_args.into_iter();
            let mut result = iter.next().unwrap();
            for arg in iter {
                result = Expr::MethodCall {
                    object: Box::new(result),
                    method: method.to_string(),
                    args: vec![arg],
                };
            }
            Ok(result)
        }
        // pow → powf
        "pow" => {
            if converted_args.len() != 2 {
                return Err(anyhow!("Math.pow expects 2 arguments"));
            }
            let mut iter = converted_args.into_iter();
            let receiver = iter.next().unwrap();
            let arg = iter.next().unwrap();
            Ok(Expr::MethodCall {
                object: Box::new(receiver),
                method: "powf".to_string(),
                args: vec![arg],
            })
        }
        _ => Err(anyhow!("unsupported Math method: {method}")),
    }
}

/// Converts a `new` expression to a `ClassName::new(args)` call.
///
/// `new Foo(x, y)` → `Expr::FnCall { name: "Foo::new", args }`
fn convert_new_expr(
    new_expr: &ast::NewExpr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let class_name = match new_expr.callee.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported new expression target")),
    };
    // Look up constructor param types from struct fields in TypeRegistry
    let param_types = reg.get(&class_name).and_then(|def| match def {
        TypeDef::Struct { fields, .. } => Some(fields.as_slice()),
        _ => None,
    });
    let args = match &new_expr.args {
        Some(args) => convert_call_args_with_types(args, reg, param_types, false, type_env)?,
        None => vec![],
    };
    Ok(Expr::FnCall {
        name: format!("{class_name}::new"),
        args,
    })
}

/// Converts call arguments from SWC `ExprOrSpread` to IR `Expr`.
fn convert_call_args(
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Vec<Expr>> {
    convert_call_args_with_types(args, reg, None, false, type_env)
}

/// Converts call arguments with optional parameter type information from the registry.
///
/// When `param_types` is provided, each argument gets the corresponding parameter's type
/// as its expected type. This enables object literal arguments to resolve their struct name.
///
/// When `has_rest` is true, the last parameter is a rest parameter (`Vec<T>`).
/// Extra arguments beyond the regular parameters are packed into a `vec![...]`.
fn convert_call_args_with_types(
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
    param_types: Option<&[(String, RustType)]>,
    has_rest: bool,
    type_env: &TypeEnv,
) -> Result<Vec<Expr>> {
    // Determine how many regular (non-rest) parameters there are
    let regular_param_count = if has_rest {
        param_types.map(|p| p.len().saturating_sub(1)).unwrap_or(0)
    } else {
        usize::MAX // No rest param → treat all as regular
    };

    // Get the element type for the rest parameter (inner type of Vec<T>)
    let rest_element_type = if has_rest {
        param_types.and_then(|p| p.last()).and_then(|(_, ty)| {
            if let RustType::Vec(inner) = ty {
                Some(inner.as_ref())
            } else {
                None
            }
        })
    } else {
        None
    };

    // Convert regular arguments
    let regular_args_count = args.len().min(regular_param_count);
    let mut result: Vec<Expr> = args[..regular_args_count]
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            let param_ty = param_types.and_then(|params| params.get(i).map(|(_, ty)| ty));
            // For Option<T> params, pass the inner type as expected so conversions apply
            let expected = match param_ty {
                Some(RustType::Option(inner)) => Some(inner.as_ref()),
                other => other,
            };
            let mut expr = convert_expr(&arg.expr, reg, expected, type_env)?;
            // Wrap in Some(...) when the parameter type is Option<T>,
            // but skip if the value is already None (from undefined)
            if let Some(RustType::Option(_)) = param_ty {
                if !matches!(&expr, Expr::Ident(name) if name == "None") {
                    expr = Expr::FnCall {
                        name: "Some".to_string(),
                        args: vec![expr],
                    };
                }
            }
            // Wrap in Box::new(...) when the parameter type is Fn (Box<dyn Fn>)
            // and the argument is an identifier (function name), not an inline closure
            if matches!(param_ty, Some(RustType::Fn { .. })) && matches!(&expr, Expr::Ident(_)) {
                expr = Expr::FnCall {
                    name: "Box::new".to_string(),
                    args: vec![expr],
                };
            }
            Ok(expr)
        })
        .collect::<Result<Vec<_>>>()?;

    if has_rest {
        // Pack remaining arguments into a vec![]
        let rest_args = &args[regular_args_count..];
        if rest_args.len() == 1 && rest_args[0].spread.is_some() {
            // Single spread: foo(...arr) → foo(arr)
            let expr = convert_expr(&rest_args[0].expr, reg, None, type_env)?;
            result.push(expr);
        } else if rest_args.iter().any(|a| a.spread.is_some()) {
            // Mixed literals and spread: foo(1, ...arr) → foo([vec![1.0], arr].concat())
            let mut parts: Vec<Expr> = Vec::new();
            let mut literal_buf: Vec<Expr> = Vec::new();

            for arg in rest_args {
                if arg.spread.is_some() {
                    // Flush literal buffer as vec![...]
                    if !literal_buf.is_empty() {
                        parts.push(Expr::Vec {
                            elements: std::mem::take(&mut literal_buf),
                        });
                    }
                    // Add spread array directly
                    let expr = convert_expr(&arg.expr, reg, None, type_env)?;
                    parts.push(expr);
                } else {
                    let expr = convert_expr(&arg.expr, reg, rest_element_type, type_env)?;
                    literal_buf.push(expr);
                }
            }
            if !literal_buf.is_empty() {
                parts.push(Expr::Vec {
                    elements: literal_buf,
                });
            }

            // [part1, part2, ...].concat()
            let concat_receiver = Expr::Vec { elements: parts };
            result.push(Expr::MethodCall {
                object: Box::new(concat_receiver),
                method: "concat".to_string(),
                args: vec![],
            });
        } else {
            // All literal args: foo(1, 2, 3) → foo(vec![1.0, 2.0, 3.0])
            let rest_exprs: Vec<Expr> = rest_args
                .iter()
                .map(|arg| convert_expr(&arg.expr, reg, rest_element_type, type_env))
                .collect::<Result<Vec<_>>>()?;
            result.push(Expr::Vec {
                elements: rest_exprs,
            });
        }
    } else {
        // Append None for missing Option parameters (default arguments)
        if let Some(params) = param_types {
            for param in params.iter().skip(result.len()) {
                if matches!(param.1, RustType::Option(_)) {
                    result.push(Expr::Ident("None".to_string()));
                }
            }
        }
    }

    Ok(result)
}

/// Converts a template literal to `Expr::FormatMacro`.
///
/// `` `Hello ${name}` `` becomes `format!("Hello {}", name)`.
fn convert_template_literal(
    tpl: &ast::Tpl,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let mut template = String::new();
    let mut args = Vec::new();

    for (i, quasi) in tpl.quasis.iter().enumerate() {
        // raw text of the quasi (the string parts between expressions)
        template.push_str(&quasi.raw);
        if i < tpl.exprs.len() {
            template.push_str("{}");
            let arg = convert_expr(&tpl.exprs[i], reg, None, type_env)?;
            args.push(arg);
        }
    }

    Ok(Expr::FormatMacro { template, args })
}

/// Converts an SWC conditional (ternary) expression to `Expr::If`.
///
/// `condition ? consequent : alternate` → `if condition { consequent } else { alternate }`
fn convert_cond_expr(
    cond: &ast::CondExpr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let condition = convert_expr(&cond.test, reg, None, type_env)?;
    let then_expr = convert_expr(&cond.cons, reg, expected, type_env)?;
    let else_expr = convert_expr(&cond.alt, reg, expected, type_env)?;
    Ok(Expr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    })
}

/// Converts an SWC object literal to an IR `Expr::StructInit`.
///
/// Requires an expected type (`RustType::Named`) from the enclosing context (e.g., a variable
/// declaration's type annotation). Without a named type, returns an error because Rust requires
/// a named struct.
///
/// `{ x: 1, y: 2 }` with expected `RustType::Named { name: "Point" }` →
/// `Expr::StructInit { name: "Point", fields: [...] }`
fn convert_object_lit(
    obj_lit: &ast::ObjectLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let struct_name = match expected {
        Some(RustType::Named { name, .. }) => name.as_str(),
        _ => {
            return Err(anyhow!(
                "object literal requires a type annotation to determine struct name"
            ))
        }
    };

    // Check if this is a discriminated union enum
    if let Some(TypeDef::Enum {
        tag_field: Some(tag),
        string_values,
        variant_fields,
        ..
    }) = reg.get(struct_name)
    {
        return convert_discriminated_union_object_lit(
            obj_lit,
            reg,
            type_env,
            struct_name,
            tag,
            string_values,
            variant_fields,
        );
    }

    // Look up field types from the registry to propagate expected types to nested values
    let struct_fields = reg.get(struct_name).and_then(|def| match def {
        TypeDef::Struct { fields, .. } => Some(fields.as_slice()),
        _ => None,
    });

    let mut fields = Vec::new();
    let mut spreads: Vec<Expr> = Vec::new();

    for prop in &obj_lit.props {
        match prop {
            ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                ast::Prop::KeyValue(kv) => {
                    let key = match &kv.key {
                        ast::PropName::Ident(ident) => ident.sym.to_string(),
                        ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => return Err(anyhow!("unsupported object literal key")),
                    };
                    // Resolve the expected type for this field from the registry
                    let field_expected = struct_fields
                        .and_then(|fs| fs.iter().find(|(name, _)| name == &key).map(|(_, ty)| ty));
                    let value = convert_expr(&kv.value, reg, field_expected, type_env)?;
                    fields.push((key, value));
                }
                ast::Prop::Shorthand(ident) => {
                    let key = ident.sym.to_string();
                    let field_expected = struct_fields
                        .and_then(|fs| fs.iter().find(|(name, _)| name == &key).map(|(_, ty)| ty));
                    let value = convert_expr(
                        &ast::Expr::Ident(ident.clone()),
                        reg,
                        field_expected,
                        type_env,
                    )?;
                    fields.push((key, value));
                }
                _ => {
                    return Err(anyhow!(
                        "unsupported object literal property (only key-value pairs and shorthand)"
                    ))
                }
            },
            ast::PropOrSpread::Spread(spread_elem) => {
                let spread_expr = convert_expr(&spread_elem.expr, reg, None, type_env)?;
                spreads.push(spread_expr);
            }
        }
    }

    // Resolve spreads into field expansion + optional struct update base
    let struct_update_base = if spreads.is_empty() {
        None
    } else if spreads.len() == 1 && struct_fields.is_some() {
        // Single spread + TypeRegistry registered → expand fields (preserves type propagation)
        let base_expr = &spreads[0];
        let all_fields = struct_fields.unwrap();
        let explicit_keys: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
        for (field_name, _) in all_fields {
            if !explicit_keys.iter().any(|k| k == field_name) {
                fields.push((
                    field_name.clone(),
                    Expr::FieldAccess {
                        object: Box::new(base_expr.clone()),
                        field: field_name.clone(),
                    },
                ));
            }
        }
        None
    } else if spreads.len() == 1 {
        // Single spread + TypeRegistry unregistered → struct update syntax
        Some(Box::new(spreads.into_iter().next().unwrap()))
    } else {
        // Multiple spreads: expand all but last via TypeRegistry, last becomes base
        let (earlier, last) = spreads.split_at(spreads.len() - 1);
        if let Some(all_fields) = struct_fields {
            let explicit_keys: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
            for spread_expr in earlier {
                for (field_name, _) in all_fields {
                    if !explicit_keys.iter().any(|k| k == field_name)
                        && !fields.iter().any(|(k, _)| k == field_name)
                    {
                        fields.push((
                            field_name.clone(),
                            Expr::FieldAccess {
                                object: Box::new(spread_expr.clone()),
                                field: field_name.clone(),
                            },
                        ));
                    }
                }
            }
        } else {
            return Err(anyhow!(
                "multiple spreads with unregistered type '{}' — TypeRegistry required for field expansion",
                struct_name
            ));
        }
        Some(Box::new(last[0].clone()))
    };

    // Auto-fill omitted Option<T> fields with None (when no struct update base)
    if struct_update_base.is_none() {
        if let Some(all_fields) = struct_fields {
            let explicit_keys: std::collections::HashSet<String> =
                fields.iter().map(|(k, _)| k.clone()).collect();
            for (field_name, field_ty) in all_fields {
                if !explicit_keys.contains(field_name) && matches!(field_ty, RustType::Option(_)) {
                    fields.push((field_name.clone(), Expr::Ident("None".to_string())));
                }
            }
        }
    }

    Ok(Expr::StructInit {
        name: struct_name.to_string(),
        fields,
        base: struct_update_base,
    })
}

/// Converts an SWC array literal to an IR `Expr::Vec` or `Expr::VecSpread`.
///
/// When `expected` is `RustType::Vec(inner)`, the inner type is propagated to each element.
///
/// Spread arrays (`[...arr, 1]`) are handled at the statement level by `try_expand_spread_*`
/// in `convert_stmt`, so only non-spread arrays should reach here. If a spread array reaches
/// here (e.g., nested in a function call argument), an error is returned.
fn convert_array_lit(
    array_lit: &ast::ArrayLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let has_spread = array_lit
        .elems
        .iter()
        .filter_map(|e| e.as_ref())
        .any(|e| e.spread.is_some());

    // When expected is a Tuple type, convert to Expr::Tuple
    if let Some(RustType::Tuple(tuple_types)) = expected {
        let elements = array_lit
            .elems
            .iter()
            .filter_map(|elem| elem.as_ref())
            .enumerate()
            .map(|(i, elem)| {
                let elem_expected = tuple_types.get(i);
                convert_expr(&elem.expr, reg, elem_expected, type_env)
            })
            .collect::<Result<Vec<_>>>()?;
        return Ok(Expr::Tuple { elements });
    }

    let element_type = match expected {
        Some(RustType::Vec(inner)) => Some(inner.as_ref()),
        _ => None,
    };

    if has_spread {
        return convert_spread_array_to_block(array_lit, reg, element_type, type_env);
    }

    let elements = array_lit
        .elems
        .iter()
        .filter_map(|elem| elem.as_ref())
        .map(|elem| convert_expr(&elem.expr, reg, element_type, type_env))
        .collect::<Result<Vec<_>>>()?;
    Ok(Expr::Vec { elements })
}

/// Converts a spread array literal to an `Expr::Block` that builds the vec at runtime.
///
/// `[1, ...arr, 2]` becomes:
/// ```text
/// {
///     let mut _v = vec![1.0];
///     _v.extend(arr.iter().cloned());
///     _v.push(2.0);
///     _v
/// }
/// ```
fn convert_spread_array_to_block(
    array_lit: &ast::ArrayLit,
    reg: &TypeRegistry,
    element_type: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let mut stmts: Vec<Stmt> = Vec::new();

    // Collect initial non-spread elements for vec![...] initialization
    let mut init_elements: Vec<Expr> = Vec::new();
    let mut initialized = false;

    for elem_opt in &array_lit.elems {
        let elem = match elem_opt {
            Some(e) => e,
            None => continue,
        };

        if elem.spread.is_some() {
            // Emit initialization if not yet done
            if !initialized {
                stmts.push(Stmt::Let {
                    mutable: true,
                    name: "_v".to_string(),
                    ty: None,
                    init: Some(Expr::Vec {
                        elements: std::mem::take(&mut init_elements),
                    }),
                });
                initialized = true;
            }
            // _v.extend(arr.iter().cloned())
            let spread_expr = convert_expr(&elem.expr, reg, None, type_env)?;
            stmts.push(Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident("_v".to_string())),
                method: "extend".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(spread_expr),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "cloned".to_string(),
                    args: vec![],
                }],
            }));
        } else {
            let value = convert_expr(&elem.expr, reg, element_type, type_env)?;
            if initialized {
                // _v.push(value)
                stmts.push(Stmt::Expr(Expr::MethodCall {
                    object: Box::new(Expr::Ident("_v".to_string())),
                    method: "push".to_string(),
                    args: vec![value],
                }));
            } else {
                init_elements.push(value);
            }
        }
    }

    // If no spread was encountered (shouldn't happen), fall back
    if !initialized {
        return Ok(Expr::Vec {
            elements: init_elements,
        });
    }

    stmts.push(Stmt::TailExpr(Expr::Ident("_v".to_string())));
    Ok(Expr::Block(stmts))
}

/// Converts an SWC unary expression to an IR `UnaryOp`.
///
/// Supported operators: `!` (logical NOT), `-` (negation).
fn convert_unary_expr(
    unary: &ast::UnaryExpr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    // typeof x → resolve based on TypeEnv
    if unary.op == ast::UnaryOp::TypeOf {
        let operand_type = resolve_expr_type(&unary.arg, type_env, reg);
        return Ok(match operand_type {
            Some(RustType::Option(inner)) => {
                // Option<T>: runtime branch — is_some() → typeof inner, else "undefined"
                let operand = convert_expr(&unary.arg, reg, None, type_env)?;
                let inner_typeof = typeof_to_string(&inner);
                Expr::If {
                    condition: Box::new(Expr::MethodCall {
                        object: Box::new(operand),
                        method: "is_some".to_string(),
                        args: vec![],
                    }),
                    then_expr: Box::new(Expr::StringLit(inner_typeof.to_string())),
                    else_expr: Box::new(Expr::StringLit("undefined".to_string())),
                }
            }
            Some(ty) => Expr::StringLit(typeof_to_string(&ty).to_string()),
            None => Expr::StringLit("object".to_string()),
        });
    }

    let op = match unary.op {
        ast::UnaryOp::Bang => UnOp::Not,
        ast::UnaryOp::Minus => UnOp::Neg,
        _ => return Err(anyhow!("unsupported unary operator: {:?}", unary.op)),
    };
    let operand = convert_expr(&unary.arg, reg, None, type_env)?;
    Ok(Expr::UnaryOp {
        op,
        operand: Box::new(operand),
    })
}

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
            // 配列インデックスアクセス: Vec<T>[n] → T
            if matches!(&member.prop, ast::MemberProp::Computed(_)) {
                if let RustType::Vec(elem_ty) = &obj_type {
                    return Some(elem_ty.as_ref().clone());
                }
            }
            resolve_field_type(&obj_type, &member.prop, reg)
        }
        ast::Expr::Paren(paren) => resolve_expr_type(&paren.expr, type_env, reg),
        ast::Expr::TsAs(ts_as) => {
            use crate::transformer::types::convert_ts_type;
            convert_ts_type(&ts_as.type_ann, &mut Vec::new(), reg).ok()
        }
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
    if let Some(crate::registry::TypeDef::Function { return_type, .. }) = reg.get(&fn_name) {
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
fn resolve_field_type(
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
        crate::registry::TypeDef::Struct { fields, .. } => fields
            .iter()
            .find(|(name, _)| name == &field_name)
            .map(|(_, ty)| ty.clone()),
        _ => None,
    }
}

/// Detects `typeof x === "type"` / `typeof x !== "type"` patterns and resolves
/// them using TypeEnv. Returns `None` if the pattern is not recognized.
/// Detects `x === undefined` / `x !== undefined` and converts to `is_none()` / `is_some()`.
fn try_convert_undefined_comparison(
    bin: &ast::BinExpr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<Expr> {
    let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
    let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
    if !is_eq && !is_neq {
        return None;
    }

    // Extract the non-undefined side
    let other_expr = if is_undefined_ident(&bin.right) {
        Some(bin.left.as_ref())
    } else if is_undefined_ident(&bin.left) {
        Some(bin.right.as_ref())
    } else {
        None
    }?;

    let other_ir = convert_expr(other_expr, reg, None, type_env).ok()?;
    let method = if is_eq { "is_none" } else { "is_some" };
    Some(Expr::MethodCall {
        object: Box::new(other_ir),
        method: method.to_string(),
        args: vec![],
    })
}

/// 等値比較で一方が string literal union enum 型の場合、文字列リテラル側を enum バリアントに変換する。
///
/// `d == "up"` → `d == Direction::Up`、`"up" != d` → `Direction::Up != d`
fn try_convert_enum_string_comparison(
    bin: &ast::BinExpr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<Expr> {
    let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
    let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
    if !is_eq && !is_neq {
        return None;
    }

    let op = if is_eq { BinOp::Eq } else { BinOp::NotEq };

    // Try: left is enum variable, right is string literal
    if let Some(str_value) = extract_string_lit(&bin.right) {
        if let Some(enum_name) = resolve_enum_type_name(&bin.left, type_env, reg) {
            if let Some(variant) = lookup_string_enum_variant(reg, &enum_name, &str_value) {
                let left = convert_expr(&bin.left, reg, None, type_env).ok()?;
                return Some(Expr::BinaryOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(Expr::Ident(format!("{enum_name}::{variant}"))),
                });
            }
        }
    }

    // Try: left is string literal, right is enum variable
    if let Some(str_value) = extract_string_lit(&bin.left) {
        if let Some(enum_name) = resolve_enum_type_name(&bin.right, type_env, reg) {
            if let Some(variant) = lookup_string_enum_variant(reg, &enum_name, &str_value) {
                let right = convert_expr(&bin.right, reg, None, type_env).ok()?;
                return Some(Expr::BinaryOp {
                    left: Box::new(Expr::Ident(format!("{enum_name}::{variant}"))),
                    op,
                    right: Box::new(right),
                });
            }
        }
    }

    None
}

/// 式から文字列リテラル値を抽出する。
fn extract_string_lit(expr: &ast::Expr) -> Option<String> {
    if let ast::Expr::Lit(ast::Lit::Str(s)) = expr {
        Some(s.value.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// 式の型が string literal union enum の場合、その enum 名を返す。
fn resolve_enum_type_name(
    expr: &ast::Expr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<String> {
    let ty = resolve_expr_type(expr, type_env, reg)?;
    if let RustType::Named { name, .. } = &ty {
        if let Some(TypeDef::Enum { string_values, .. }) = reg.get(name) {
            if !string_values.is_empty() {
                return Some(name.clone());
            }
        }
    }
    None
}

/// Returns true if the expression is the `undefined` identifier.
fn is_undefined_ident(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
}

fn try_convert_typeof_comparison(
    bin: &ast::BinExpr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<Expr> {
    let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
    let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
    if !is_eq && !is_neq {
        return None;
    }

    // Extract (typeof operand, type string) from either order
    let (typeof_operand, type_str) = extract_typeof_and_string(bin)?;

    // Resolve the operand's type from TypeEnv
    let operand_type = resolve_expr_type(typeof_operand, type_env, reg);

    let result = match &operand_type {
        Some(ty) => resolve_typeof_match(ty, &type_str),
        None => TypeofMatch::Placeholder,
    };

    let expr = match result {
        TypeofMatch::True => Expr::BoolLit(!is_neq),
        TypeofMatch::False => Expr::BoolLit(is_neq),
        TypeofMatch::IsNone => {
            let operand_ir =
                crate::transformer::expressions::convert_expr(typeof_operand, reg, None, type_env)
                    .ok()?;
            let method = if is_neq { "is_some" } else { "is_none" };
            Expr::MethodCall {
                object: Box::new(operand_ir),
                method: method.to_string(),
                args: vec![],
            }
        }
        TypeofMatch::Placeholder => {
            // Unknown type → optimistic true (negated for !==)
            Expr::BoolLit(!is_neq)
        }
    };

    Some(expr)
}

/// Extracts the typeof operand and the comparison string from a binary expression.
/// Handles both `typeof x === "string"` and `"string" === typeof x`.
fn extract_typeof_and_string(bin: &ast::BinExpr) -> Option<(&ast::Expr, String)> {
    // Left is typeof, right is string
    if let ast::Expr::Unary(unary) = bin.left.as_ref() {
        if unary.op == ast::UnaryOp::TypeOf {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.right.as_ref() {
                return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
            }
        }
    }
    // Right is typeof, left is string
    if let ast::Expr::Unary(unary) = bin.right.as_ref() {
        if unary.op == ast::UnaryOp::TypeOf {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.left.as_ref() {
                return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
            }
        }
    }
    None
}

enum TypeofMatch {
    True,
    False,
    IsNone,
    Placeholder,
}

/// Resolves whether a RustType matches a typeof string.
fn resolve_typeof_match(ty: &RustType, typeof_str: &str) -> TypeofMatch {
    match typeof_str {
        "string" => {
            if matches!(ty, RustType::String) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        "number" => {
            if matches!(ty, RustType::F64) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        "boolean" => {
            if matches!(ty, RustType::Bool) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        "undefined" => {
            if matches!(ty, RustType::Option(_)) {
                TypeofMatch::IsNone
            } else {
                TypeofMatch::False
            }
        }
        "object" => {
            if matches!(ty, RustType::Named { .. } | RustType::Vec(_)) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        "function" => {
            if matches!(ty, RustType::Fn { .. }) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        _ => TypeofMatch::False,
    }
}

/// Converts `"key" in obj` to a Rust expression.
///
/// - struct with known fields → static `true`/`false`
/// - HashMap → `obj.contains_key("key")`
/// - unknown type → `todo!()` (no silent `true` fallback)
fn convert_in_operator(bin: &ast::BinExpr, reg: &TypeRegistry, type_env: &TypeEnv) -> Expr {
    // Extract the key string from LHS (must be a string literal)
    let key = match bin.left.as_ref() {
        ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
        _ => {
            return Expr::FnCall {
                name: "todo!".to_string(),
                args: vec![Expr::StringLit(
                    "in operator with non-string key".to_string(),
                )],
            };
        }
    };

    // Resolve the RHS object type
    let obj_type = resolve_expr_type(&bin.right, type_env, reg);

    match &obj_type {
        Some(RustType::Named { name, .. }) if name == "HashMap" || name == "BTreeMap" => {
            // HashMap/BTreeMap → obj.contains_key("key")
            let obj_ir = match bin.right.as_ref() {
                ast::Expr::Ident(ident) => Expr::Ident(ident.sym.as_ref().to_owned()),
                _ => {
                    return Expr::FnCall {
                        name: "todo!".to_string(),
                        args: vec![Expr::StringLit(
                            "in operator with complex RHS expression".to_string(),
                        )],
                    };
                }
            };
            Expr::MethodCall {
                object: Box::new(obj_ir),
                method: "contains_key".to_string(),
                args: vec![Expr::StringLit(key)],
            }
        }
        Some(RustType::Named { name, .. }) => {
            // Check TypeRegistry for field existence
            match reg.get(name) {
                Some(TypeDef::Struct { fields, .. }) => {
                    Expr::BoolLit(fields.iter().any(|(f, _)| f == &key))
                }
                Some(TypeDef::Enum {
                    tag_field,
                    variant_fields,
                    ..
                }) => {
                    // discriminated union: check if any variant has this field
                    if tag_field.as_deref() == Some(key.as_str()) {
                        Expr::BoolLit(true) // tag field always exists
                    } else {
                        Expr::BoolLit(
                            variant_fields
                                .values()
                                .any(|fields| fields.iter().any(|(f, _)| f == &key)),
                        )
                    }
                }
                _ => Expr::FnCall {
                    name: "todo!".to_string(),
                    args: vec![Expr::StringLit(format!(
                        "in operator — type '{name}' has unknown shape"
                    ))],
                },
            }
        }
        _ => Expr::FnCall {
            name: "todo!".to_string(),
            args: vec![Expr::StringLit(format!(
                "in operator — cannot resolve type of RHS for key '{key}'"
            ))],
        },
    }
}

/// Converts `x instanceof ClassName` using TypeEnv.
///
/// - Known matching type → `true`
/// - Known non-matching type → `false`
/// - `Option<T>` where T matches → `x.is_some()`
/// - Unknown type → `todo!()` placeholder (compile error, not silent `true`)
fn convert_instanceof(bin: &ast::BinExpr, type_env: &TypeEnv) -> Expr {
    // Get the class name from the RHS
    let class_name = match bin.right.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => {
            // Non-identifier RHS (e.g., `x instanceof expr`) — cannot resolve statically
            return Expr::FnCall {
                name: "todo!".to_string(),
                args: vec![Expr::StringLit(
                    "instanceof with non-identifier RHS".to_string(),
                )],
            };
        }
    };

    // Get the LHS variable name and type
    let lhs_type = match bin.left.as_ref() {
        ast::Expr::Ident(ident) => type_env.get(ident.sym.as_ref()).cloned(),
        _ => None,
    };

    match lhs_type {
        Some(RustType::Named { name, .. }) => Expr::BoolLit(name == class_name),
        Some(RustType::Option(inner)) => match inner.as_ref() {
            RustType::Named { name, .. } if name == &class_name => {
                let lhs_ir = match bin.left.as_ref() {
                    ast::Expr::Ident(ident) => Expr::Ident(ident.sym.to_string()),
                    _ => {
                        return Expr::FnCall {
                            name: "todo!".to_string(),
                            args: vec![Expr::StringLit("instanceof with complex LHS".to_string())],
                        };
                    }
                };
                Expr::MethodCall {
                    object: Box::new(lhs_ir),
                    method: "is_some".to_string(),
                    args: vec![],
                }
            }
            _ => Expr::BoolLit(false),
        },
        Some(_) => Expr::BoolLit(false),
        // Unknown type: generate todo!() instead of silent true
        None => Expr::FnCall {
            name: "todo!".to_string(),
            args: vec![Expr::StringLit(format!(
                "instanceof {class_name} — type unknown"
            ))],
        },
    }
}

/// Resolves typeof to a string literal based on TypeEnv type.
fn typeof_to_string(ty: &RustType) -> &'static str {
    match ty {
        RustType::String => "string",
        RustType::F64 => "number",
        RustType::Bool => "boolean",
        RustType::Option(_) => "undefined",
        RustType::Named { .. } | RustType::Vec(_) => "object",
        RustType::Fn { .. } => "function",
        _ => "object",
    }
}

#[cfg(test)]
mod tests;
