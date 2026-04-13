use crate::ir::RustType;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{
    ConstElement, ConstField, FieldDef, MethodSignature, ParamDef, TypeDef, TypeRegistry,
};
use crate::ts_type_info::resolve::typedef::resolve_method_sig;
use crate::ts_type_info::resolve::{resolve_ts_type, resolve_type_params, resolve_typedef};
use crate::ts_type_info::TsTypeInfo;

// ── resolve_typedef エラー伝播 ──

/// resolve_ts_type が必ず Err を返す TsTypeInfo を生成するヘルパー。
/// `keyof typeof NonExistent` は registry に存在しないため Err になる。
fn unresolvable_type() -> TsTypeInfo {
    TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeQuery("NonExistent".to_string())))
}

#[test]
fn resolve_typedef_struct_field_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::Struct {
        type_params: vec![],
        fields: vec![
            FieldDef {
                name: "ok".to_string(),
                ty: TsTypeInfo::String,
                optional: false,
            },
            FieldDef {
                name: "bad".to_string(),
                ty: unresolvable_type(),
                optional: false,
            },
        ],
        methods: std::collections::HashMap::new(),
        constructor: None,
        call_signatures: vec![],
        extends: vec![],
        is_interface: false,
    };
    assert!(resolve_typedef(def, &reg, &mut syn).is_err());
}

#[test]
fn resolve_typedef_struct_method_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::Struct {
        type_params: vec![],
        fields: vec![],
        methods: [(
            "bad_method".to_string(),
            vec![MethodSignature {
                params: vec![ParamDef {
                    name: "x".to_string(),
                    ty: unresolvable_type(),
                    optional: false,
                    has_default: false,
                }],
                return_type: None,
                has_rest: false,
                type_params: vec![],
            }],
        )]
        .into_iter()
        .collect(),
        constructor: None,
        call_signatures: vec![],
        extends: vec![],
        is_interface: false,
    };
    assert!(resolve_typedef(def, &reg, &mut syn).is_err());
}

#[test]
fn resolve_typedef_struct_constructor_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::Struct {
        type_params: vec![],
        fields: vec![],
        methods: std::collections::HashMap::new(),
        constructor: Some(vec![MethodSignature {
            params: vec![ParamDef {
                name: "x".to_string(),
                ty: unresolvable_type(),
                optional: false,
                has_default: false,
            }],
            return_type: None,
            has_rest: false,
            type_params: vec![],
        }]),
        call_signatures: vec![],
        extends: vec![],
        is_interface: false,
    };
    assert!(resolve_typedef(def, &reg, &mut syn).is_err());
}

#[test]
fn resolve_typedef_struct_call_signature_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::Struct {
        type_params: vec![],
        fields: vec![],
        methods: std::collections::HashMap::new(),
        constructor: None,
        call_signatures: vec![MethodSignature {
            params: vec![],
            return_type: Some(unresolvable_type()),
            has_rest: false,
            type_params: vec![],
        }],
        extends: vec![],
        is_interface: false,
    };
    assert!(resolve_typedef(def, &reg, &mut syn).is_err());
}

#[test]
fn resolve_typedef_function_param_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::Function {
        type_params: vec![],
        params: vec![ParamDef {
            name: "x".to_string(),
            ty: unresolvable_type(),
            optional: false,
            has_default: false,
        }],
        return_type: None,
        has_rest: false,
    };
    assert!(resolve_typedef(def, &reg, &mut syn).is_err());
}

#[test]
fn resolve_typedef_enum_variant_field_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::Enum {
        type_params: vec![],
        variants: vec!["A".to_string()],
        string_values: std::collections::HashMap::new(),
        tag_field: None,
        variant_fields: [(
            "A".to_string(),
            vec![FieldDef {
                name: "bad".to_string(),
                ty: unresolvable_type(),
                optional: false,
            }],
        )]
        .into_iter()
        .collect(),
    };
    assert!(resolve_typedef(def, &reg, &mut syn).is_err());
}

#[test]
fn resolve_typedef_const_value_field_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::ConstValue {
        fields: vec![ConstField {
            name: "bad".to_string(),
            ty: unresolvable_type(),
            string_literal_value: None,
        }],
        elements: vec![],
        type_ref_name: None,
    };
    assert!(resolve_typedef(def, &reg, &mut syn).is_err());
}

#[test]
fn resolve_typedef_const_value_element_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::ConstValue {
        fields: vec![],
        elements: vec![ConstElement {
            ty: unresolvable_type(),
            string_literal_value: None,
        }],
        type_ref_name: None,
    };
    assert!(resolve_typedef(def, &reg, &mut syn).is_err());
}

#[test]
fn resolve_method_sig_param_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let sig = MethodSignature {
        params: vec![ParamDef {
            name: "x".to_string(),
            ty: unresolvable_type(),
            optional: false,
            has_default: false,
        }],
        return_type: Some(TsTypeInfo::String),
        has_rest: false,
        type_params: vec![],
    };
    assert!(resolve_method_sig(sig, &reg, &mut syn).is_err());
}

#[test]
fn resolve_method_sig_return_type_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let sig = MethodSignature {
        params: vec![],
        return_type: Some(unresolvable_type()),
        has_rest: false,
        type_params: vec![],
    };
    assert!(resolve_method_sig(sig, &reg, &mut syn).is_err());
}

#[test]
fn resolve_type_params_constraint_error_propagates() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let params = vec![crate::ir::TypeParam {
        name: "T".to_string(),
        constraint: Some(unresolvable_type()),
        default: None,
    }];
    assert!(resolve_type_params(params, &reg, &mut syn).is_err());
}

// --- I-387 T4: primitive_int_kind_from_name / std_collection_kind_from_name ヘルパー ---

#[test]
fn test_primitive_int_kind_from_name_covers_all_rust_int_types() {
    use crate::ir::PrimitiveIntKind::*;
    use crate::ts_type_info::resolve::primitive_int_kind_from_name;
    let cases = [
        ("usize", Usize),
        ("isize", Isize),
        ("i8", I8),
        ("i16", I16),
        ("i32", I32),
        ("i64", I64),
        ("i128", I128),
        ("u8", U8),
        ("u16", U16),
        ("u32", U32),
        ("u64", U64),
        ("u128", U128),
        ("f32", F32),
    ];
    for (name, expected) in cases {
        assert_eq!(
            primitive_int_kind_from_name(name),
            Some(expected),
            "name={name}"
        );
    }
}

#[test]
fn test_primitive_int_kind_from_name_rejects_non_int_types() {
    use crate::ts_type_info::resolve::primitive_int_kind_from_name;
    // f64 / bool / String は `RustType` 本体の専用 variant を使うため
    // `PrimitiveIntKind` には含まれない。
    assert_eq!(primitive_int_kind_from_name("f64"), None);
    assert_eq!(primitive_int_kind_from_name("bool"), None);
    assert_eq!(primitive_int_kind_from_name("String"), None);
    assert_eq!(primitive_int_kind_from_name("HTTPException"), None);
    assert_eq!(primitive_int_kind_from_name(""), None);
}

#[test]
fn test_std_collection_kind_from_name_covers_all_supported_types() {
    use crate::ir::StdCollectionKind::*;
    use crate::ts_type_info::resolve::std_collection_kind_from_name;
    let cases = [
        ("Box", Box),
        ("HashMap", HashMap),
        ("BTreeMap", BTreeMap),
        ("HashSet", HashSet),
        ("BTreeSet", BTreeSet),
        ("VecDeque", VecDeque),
        ("Rc", Rc),
        ("Arc", Arc),
        ("Mutex", Mutex),
        ("RwLock", RwLock),
        ("RefCell", RefCell),
        ("Cell", Cell),
    ];
    for (name, expected) in cases {
        assert_eq!(
            std_collection_kind_from_name(name),
            Some(expected),
            "name={name}"
        );
    }
}

#[test]
fn test_std_collection_kind_from_name_rejects_existing_variants() {
    use crate::ts_type_info::resolve::std_collection_kind_from_name;
    // `Vec` / `Option` / `Result` / `Tuple` は `RustType` 本体の専用 variant を
    // 使用するため `StdCollectionKind` には含まれない。
    assert_eq!(std_collection_kind_from_name("Vec"), None);
    assert_eq!(std_collection_kind_from_name("Option"), None);
    assert_eq!(std_collection_kind_from_name("Result"), None);
    assert_eq!(std_collection_kind_from_name("Tuple"), None);
    assert_eq!(std_collection_kind_from_name("HTTPException"), None);
    assert_eq!(std_collection_kind_from_name(""), None);
}

#[test]
fn test_resolve_type_ref_returns_type_var_when_in_scope() {
    // I-387 T4b: `is_in_type_param_scope` で見つかった名前は TypeVar に routing される。
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    syn.push_type_param_scope(vec!["T".to_string()]);

    let info = TsTypeInfo::TypeRef {
        name: "T".to_string(),
        type_args: vec![],
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::TypeVar {
            name: "T".to_string()
        }
    );
}

#[test]
fn test_resolve_type_ref_returns_named_when_not_in_scope() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    // scope に何も push せず、user 型として Named に fallback することを確認
    let info = TsTypeInfo::TypeRef {
        name: "T".to_string(),
        type_args: vec![],
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn test_primitive_and_std_collection_kind_from_name_are_disjoint() {
    // 整数名と std コレクション名は重複しない (命名の直交性検証)
    use crate::ts_type_info::resolve::{
        primitive_int_kind_from_name, std_collection_kind_from_name,
    };
    let int_names = [
        "usize", "isize", "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128",
        "f32",
    ];
    for name in int_names {
        assert!(
            std_collection_kind_from_name(name).is_none(),
            "int name {name} leaked into std collection"
        );
    }
    let coll_names = [
        "Box", "HashMap", "BTreeMap", "HashSet", "BTreeSet", "VecDeque", "Rc", "Arc", "Mutex",
        "RwLock", "RefCell", "Cell",
    ];
    for name in coll_names {
        assert!(
            primitive_int_kind_from_name(name).is_none(),
            "collection name {name} leaked into primitive int"
        );
    }
}
