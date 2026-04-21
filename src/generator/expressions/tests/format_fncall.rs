//! `FormatMacro` rendering and `FnCall` dispatch per `CallTarget`
//! variant:
//!
//! - `FormatMacro` — `format!(template, args...)`
//! - `FnCall`:
//!   - `Free` / `ExternalPath` — bare or `::`-joined path
//!   - `Super` — keyword-based call
//!   - `UserTupleCtor` — bare type name `Wrapper(x)`
//!   - `UserEnumVariantCtor` / `UserAssocFn` — `Type::Variant(...)` /
//!     `Type::method(...)`
//!   - `BuiltinVariant` (`Some`, `None`, `Ok`, `Err`) — call form and
//!     I-379 `BuiltinVariantValue` reference form
//!
//! Also covers the I-378 Phase 1 structured value `Expr` variants
//! (`EnumVariant`, `PrimitiveAssocConst`, `StdConst`) since they share
//! the qualified-path rendering family.

use super::*;
use crate::ir::{CallTarget, Expr};

// --- FormatMacro ---

#[test]
fn test_generate_expr_format_macro_no_args() {
    let expr = Expr::FormatMacro {
        template: "hello".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "format!(\"hello\")");
}

#[test]
fn test_generate_expr_format_macro_with_args() {
    let expr = Expr::FormatMacro {
        template: "Hello, {}!".to_string(),
        args: vec![Expr::Ident("name".to_string())],
    };
    assert_eq!(generate_expr(&expr), "format!(\"Hello, {}!\", name)");
}

// --- FnCall builtin variants (Err, Ok) ---

#[test]
fn test_generate_expr_fn_call_err() {
    let expr = Expr::FnCall {
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
        args: vec![Expr::StringLit("error".to_string())],
    };
    assert_eq!(generate_expr(&expr), "Err(\"error\")");
}

#[test]
fn test_generate_expr_fn_call_ok() {
    let expr = Expr::FnCall {
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
        args: vec![Expr::NumberLit(42.0)],
    };
    assert_eq!(generate_expr(&expr), "Ok(42.0)");
}

// ---------------------------------------------------------------------------
// I-375: `Expr::FnCall` generator tests for each `CallTarget` variant
// ---------------------------------------------------------------------------

#[test]
fn test_generate_fn_call_single_segment_path() {
    let expr = Expr::FnCall {
        target: CallTarget::Free("foo".to_string()),
        args: vec![Expr::NumberLit(1.0), Expr::NumberLit(2.0)],
    };
    assert_eq!(generate_expr(&expr), "foo(1.0, 2.0)");
}

#[test]
fn test_generate_fn_call_two_segment_assoc_path() {
    // `Color::Red(x)` — synthetic enum variant constructor output.
    // The generator joins segments with `::` and emits the args verbatim;
    // any `.to_string()` wrapping is the Transformer's responsibility.
    let expr = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: crate::ir::UserTypeRef::new("Color"),
            method: "Red".to_string(),
        },
        args: vec![Expr::StringLit("red".to_string())],
    };
    assert_eq!(generate_expr(&expr), "Color::Red(\"red\")");
}

#[test]
fn test_generate_fn_call_multi_segment_path() {
    // `std::fs::write(path, data)` — a multi-segment std call
    let expr = Expr::FnCall {
        target: CallTarget::ExternalPath(vec![
            "std".to_string(),
            "fs".to_string(),
            "write".to_string(),
        ]),
        args: vec![
            Expr::Ident("path".to_string()),
            Expr::Ident("data".to_string()),
        ],
    };
    assert_eq!(generate_expr(&expr), "std::fs::write(path, data)");
}

#[test]
fn test_generate_fn_call_super() {
    let expr = Expr::FnCall {
        target: CallTarget::Super,
        args: vec![Expr::Ident("x".to_string())],
    };
    assert_eq!(generate_expr(&expr), "super(x)");
}

#[test]
fn test_generate_fn_call_super_no_args() {
    let expr = Expr::FnCall {
        target: CallTarget::Super,
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "super()");
}

#[test]
fn test_generate_fn_call_user_tuple_ctor_emits_bare_type_name() {
    // `Wrapper(x)` for callable interface tuple struct constructor.
    let expr = Expr::FnCall {
        target: CallTarget::UserTupleCtor(crate::ir::UserTypeRef::new("Wrapper")),
        args: vec![Expr::IntLit(42)],
    };
    assert_eq!(generate_expr(&expr), "Wrapper(42)");
}

#[test]
fn test_generate_fn_call_user_enum_variant_ctor_emits_enum_path() {
    // `Color::Red(x)` — payload enum variant constructor.
    let expr = Expr::FnCall {
        target: CallTarget::UserEnumVariantCtor {
            enum_ty: crate::ir::UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        },
        args: vec![Expr::StringLit("red".to_string())],
    };
    assert_eq!(generate_expr(&expr), "Color::Red(\"red\")");
}

#[test]
fn test_generate_fn_call_builtin_variant_some_and_none() {
    // `Some(x)` / `None` — Option constructors.
    let some_expr = Expr::FnCall {
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
        args: vec![Expr::IntLit(1)],
    };
    assert_eq!(generate_expr(&some_expr), "Some(1)");

    let none_expr = Expr::FnCall {
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::None),
        args: vec![],
    };
    assert_eq!(generate_expr(&none_expr), "None()");

    // I-379: `None` as a value reference (not a call) is structured as
    // `Expr::BuiltinVariantValue(BuiltinVariant::None)`.
    let none_value = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None);
    assert_eq!(generate_expr(&none_value), "None");
    let some_value = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::Some);
    assert_eq!(generate_expr(&some_value), "Some");
    let ok_value = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::Ok);
    assert_eq!(generate_expr(&ok_value), "Ok");
    let err_value = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::Err);
    assert_eq!(generate_expr(&err_value), "Err");
}

#[test]
fn test_generate_fn_call_user_assoc_fn_emits_qualified_path() {
    // I-378: `CallTarget::UserAssocFn` は `UserTypeRef` を保持し、generator は
    // 単純に `{ty}::{method}(args)` を emit する。I-375 の `Path { type_ref }` は
    // metadata 形式だったが、I-378 で構造的に区別されるようになった。
    let target = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: crate::ir::UserTypeRef::new("myClass"),
            method: "new".to_string(),
        },
        args: vec![],
    };
    assert_eq!(generate_expr(&target), "myClass::new()");
}

// I-378 Phase 1: rendering tests for the 3 new structured Expr variants.

#[test]
fn renders_enum_variant_as_qualified_path() {
    let expr = Expr::EnumVariant {
        enum_ty: crate::ir::UserTypeRef::new("Color"),
        variant: "Red".to_string(),
    };
    assert_eq!(generate_expr(&expr), "Color::Red");
}

#[test]
fn renders_primitive_assoc_const_as_qualified_path() {
    let nan = Expr::PrimitiveAssocConst {
        ty: crate::ir::PrimitiveType::F64,
        name: "NAN".to_string(),
    };
    assert_eq!(generate_expr(&nan), "f64::NAN");

    let i32_max = Expr::PrimitiveAssocConst {
        ty: crate::ir::PrimitiveType::I32,
        name: "MAX".to_string(),
    };
    assert_eq!(generate_expr(&i32_max), "i32::MAX");
}

#[test]
fn renders_std_const_via_rust_path() {
    assert_eq!(
        generate_expr(&Expr::StdConst(crate::ir::StdConst::F64Pi)),
        "std::f64::consts::PI"
    );
    assert_eq!(
        generate_expr(&Expr::StdConst(crate::ir::StdConst::F64Ln2)),
        "std::f64::consts::LN_2"
    );
    assert_eq!(
        generate_expr(&Expr::StdConst(crate::ir::StdConst::F64Sqrt2)),
        "std::f64::consts::SQRT_2"
    );
}
