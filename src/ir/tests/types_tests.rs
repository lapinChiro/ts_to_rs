use super::super::*;

#[test]
fn test_rust_type_primitives() {
    let _t: RustType = RustType::String;
    let _t: RustType = RustType::F64;
    let _t: RustType = RustType::Bool;
}

#[test]
fn test_rust_type_option() {
    let inner = RustType::String;
    let _t: RustType = RustType::Option(Box::new(inner));
}

#[test]
fn test_rust_type_vec() {
    let inner = RustType::F64;
    let _t: RustType = RustType::Vec(Box::new(inner));
}

#[test]
fn test_visibility() {
    let _pub = Visibility::Public;
    let _priv = Visibility::Private;
}

#[test]
fn test_rust_type_result() {
    let ty = RustType::Result {
        ok: Box::new(RustType::String),
        err: Box::new(RustType::String),
    };
    match ty {
        RustType::Result { ok, err } => {
            assert_eq!(*ok, RustType::String);
            assert_eq!(*err, RustType::String);
        }
        _ => panic!("expected Result"),
    }
}

#[test]
fn test_wrap_optional_non_option() {
    assert_eq!(
        RustType::F64.wrap_optional(),
        RustType::Option(Box::new(RustType::F64))
    );
}

#[test]
fn test_wrap_optional_already_option_no_double_wrap() {
    let ty = RustType::Option(Box::new(RustType::String));
    assert_eq!(ty.clone().wrap_optional(), ty);
}

// ---- I-387: RustType 構造的精緻化 ----

#[test]
fn test_rust_type_type_var_construction() {
    let ty = RustType::TypeVar {
        name: "T".to_string(),
    };
    match ty {
        RustType::TypeVar { name } => assert_eq!(name, "T"),
        _ => panic!("expected TypeVar"),
    }
}

#[test]
fn test_rust_type_primitive_int_variants() {
    let usize_ty = RustType::Primitive(PrimitiveIntKind::Usize);
    let i32_ty = RustType::Primitive(PrimitiveIntKind::I32);
    assert_ne!(usize_ty, i32_ty);
    match usize_ty {
        RustType::Primitive(PrimitiveIntKind::Usize) => {}
        _ => panic!("expected Primitive(Usize)"),
    }
}

#[test]
fn test_rust_type_std_collection_construction() {
    let ty = RustType::StdCollection {
        kind: StdCollectionKind::HashMap,
        args: vec![
            RustType::String,
            RustType::TypeVar {
                name: "V".to_string(),
            },
        ],
    };
    match ty {
        RustType::StdCollection { kind, args } => {
            assert_eq!(kind, StdCollectionKind::HashMap);
            assert_eq!(args.len(), 2);
            assert_eq!(args[0], RustType::String);
            assert_eq!(
                args[1],
                RustType::TypeVar {
                    name: "V".to_string()
                }
            );
        }
        _ => panic!("expected StdCollection"),
    }
}

#[test]
fn test_rust_type_type_var_uses_param() {
    let ty = RustType::TypeVar {
        name: "T".to_string(),
    };
    assert!(ty.uses_param("T"));
    assert!(!ty.uses_param("U"));
}

#[test]
fn test_rust_type_std_collection_uses_param_recurses() {
    let ty = RustType::StdCollection {
        kind: StdCollectionKind::Box,
        args: vec![RustType::TypeVar {
            name: "T".to_string(),
        }],
    };
    assert!(ty.uses_param("T"));
    assert!(!ty.uses_param("U"));
}

#[test]
fn test_rust_type_primitive_does_not_use_any_param() {
    let ty = RustType::Primitive(PrimitiveIntKind::Usize);
    assert!(!ty.uses_param("T"));
    assert!(!ty.uses_param("usize"));
}
