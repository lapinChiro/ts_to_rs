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
