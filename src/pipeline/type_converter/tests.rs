use super::*;
use crate::parser::parse_typescript;
use crate::registry::{build_registry, TypeDef};

/// Helper: parse a type annotation from a variable declaration.
fn parse_type_annotation(source: &str) -> swc_ecma_ast::Module {
    parse_typescript(source).unwrap()
}

#[test]
fn test_convert_string_type() {
    let module = parse_type_annotation("const x: string = '';");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::Var(var_decl),
    ))) = module.body.first()
    {
        if let Some(ann) = &var_decl.decls[0].name.as_ident().unwrap().type_ann {
            let result = convert_ts_type(&ann.type_ann, &mut synthetic, &reg);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), RustType::String);
            assert!(
                synthetic.all_items().is_empty(),
                "string type should not create synthetic types"
            );
        }
    }
}

#[test]
fn test_convert_number_type() {
    let module = parse_type_annotation("const x: number = 0;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::Var(var_decl),
    ))) = module.body.first()
    {
        if let Some(ann) = &var_decl.decls[0].name.as_ident().unwrap().type_ann {
            let result = convert_ts_type(&ann.type_ann, &mut synthetic, &reg);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), RustType::F64);
        }
    }
}

#[test]
fn test_convert_union_type_registers_synthetic() {
    let module = parse_type_annotation("const x: string | number = '';");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::Var(var_decl),
    ))) = module.body.first()
    {
        if let Some(ann) = &var_decl.decls[0].name.as_ident().unwrap().type_ann {
            let result = convert_ts_type(&ann.type_ann, &mut synthetic, &reg);
            assert!(result.is_ok());
            // The union should have created a synthetic enum
            assert!(
                !synthetic.all_items().is_empty(),
                "union type should create a synthetic enum"
            );
        }
    }
}

#[test]
fn test_convert_nullable_type() {
    let module = parse_type_annotation("const x: string | null = null;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::Var(var_decl),
    ))) = module.body.first()
    {
        if let Some(ann) = &var_decl.decls[0].name.as_ident().unwrap().type_ann {
            let result = convert_ts_type(&ann.type_ann, &mut synthetic, &reg);
            assert!(result.is_ok());
            assert_eq!(
                result.unwrap(),
                RustType::Option(Box::new(RustType::String))
            );
            assert!(
                synthetic.all_items().is_empty(),
                "nullable union should not create synthetic enum (just Option)"
            );
        }
    }
}

// --- typeof const value (T3) ---

#[test]
fn test_convert_typeof_const_array_returns_named() {
    // typeof on a registered ConstValue should return RustType::Named
    let mut reg = crate::registry::TypeRegistry::new();
    reg.register(
        "TYPES".to_string(),
        TypeDef::ConstValue {
            fields: vec![],
            elements: vec![
                crate::registry::ConstElement {
                    ty: RustType::String,
                    string_literal_value: Some("a".to_string()),
                },
                crate::registry::ConstElement {
                    ty: RustType::String,
                    string_literal_value: Some("b".to_string()),
                },
            ],
            type_ref_name: None,
        },
    );
    let mut synthetic = SyntheticTypeRegistry::new();

    // Build a typeof expression AST node
    let module = parse_typescript("type T = typeof TYPES;").unwrap();
    // The type alias body should be TsTypeQuery
    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.first()
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(result.is_ok(), "typeof on ConstValue should succeed");
        assert_eq!(
            result.unwrap(),
            RustType::Named {
                name: "TYPES".to_string(),
                type_args: vec![],
            }
        );
    } else {
        panic!("expected type alias declaration");
    }
}

// --- indexed access [number] key (T4) ---

#[test]
fn test_indexed_access_number_key_const_string_array() {
    // (typeof X)[number] on a string array ConstValue → synthetic string enum
    let module = parse_typescript(
        "const TYPES = ['gzip', 'deflate'] as const;\ntype T = (typeof TYPES)[number];",
    )
    .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    // Get the second declaration (the type alias)
    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "indexed access [number] on const array should succeed: {:?}",
            result.err()
        );
        // Should be a Named type pointing to a synthetic string enum
        let ty = result.unwrap();
        assert!(
            matches!(&ty, RustType::Named { .. }),
            "expected Named type for synthetic enum, got {ty:?}"
        );
        // Synthetic registry should have the enum
        assert!(
            !synthetic.all_items().is_empty(),
            "should generate a synthetic string enum"
        );
    } else {
        panic!("expected type alias as second declaration");
    }
}

// --- indexed access [keyof typeof X] key (T5) ---

#[test]
fn test_indexed_access_keyof_typeof_string_values() {
    // (typeof X)[keyof typeof X] on an object with string values → string enum
    let module = parse_typescript(
        "const MIMES = { aac: 'audio/aac', avi: 'video/avi' } as const;\ntype T = (typeof MIMES)[keyof typeof MIMES];",
    )
    .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "indexed access [keyof typeof X] should succeed: {:?}",
            result.err()
        );
        let ty = result.unwrap();
        assert!(
            matches!(&ty, RustType::Named { .. }),
            "expected Named type for synthetic string enum, got {ty:?}"
        );
    } else {
        panic!("expected type alias as second declaration");
    }
}

#[test]
fn test_indexed_access_keyof_typeof_number_values() {
    // (typeof X)[keyof typeof X] on an object with number values → f64
    let module = parse_typescript(
        "const PHASE = { A: 1, B: 2 } as const;\ntype T = (typeof PHASE)[keyof typeof PHASE];",
    )
    .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "indexed access [keyof typeof X] with number values should succeed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), RustType::F64);
    } else {
        panic!("expected type alias as second declaration");
    }
}

// --- keyof typeof X type operator (T6) ---

#[test]
fn test_keyof_typeof_const_object() {
    // keyof typeof X → string enum of field names
    let module =
        parse_typescript("const OBJ = { a: 1, b: 2, c: 3 } as const;\ntype K = keyof typeof OBJ;")
            .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "keyof typeof should succeed: {:?}",
            result.err()
        );
        let ty = result.unwrap();
        assert!(
            matches!(&ty, RustType::Named { .. }),
            "expected Named type for key string enum, got {ty:?}"
        );
        assert!(
            !synthetic.all_items().is_empty(),
            "should generate a synthetic string enum for keys"
        );
    } else {
        panic!("expected type alias as second declaration");
    }
}

// --- lookup_field_type for ConstValue (T7) ---

#[test]
fn test_indexed_access_string_key_const_object() {
    // (typeof X)['fieldName'] → field type from ConstValue
    let module = parse_typescript(
        "const OBJ = { x: 'hello', y: 42 } as const;\ntype T = (typeof OBJ)['x'];",
    )
    .unwrap();
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    if let Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
        swc_ecma_ast::Decl::TsTypeAlias(alias),
    ))) = module.body.get(1)
    {
        let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
        assert!(
            result.is_ok(),
            "string key indexed access on ConstValue should succeed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), RustType::String);
    } else {
        panic!("expected type alias as second declaration");
    }
}

// ---------------------------------------------------------------------------
// Helper: extract a TsTypeAliasDecl from parsed TS source at the given index
// ---------------------------------------------------------------------------

/// Extracts a `TsTypeAliasDecl` from the module body at `index`.
///
/// Panics if the item at `index` is not a type alias declaration.
fn extract_type_alias(
    module: &swc_ecma_ast::Module,
    index: usize,
) -> &swc_ecma_ast::TsTypeAliasDecl {
    match module.body.get(index) {
        Some(swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
            swc_ecma_ast::Decl::TsTypeAlias(alias),
        ))) => alias,
        _ => panic!("expected type alias declaration at index {index}"),
    }
}

// ===========================================================================
// convert_type_alias_items — conditional type (tests 1–3)
// ===========================================================================

#[test]
fn test_convert_type_alias_conditional_type_infer_pattern_generates_associated_type() {
    // type X<T> = T extends Promise<infer U> ? U : never → associated type
    let module = parse_type_annotation("type X<T> = T extends Promise<infer U> ? U : never;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let items = super::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .expect("should succeed");

    // Should produce exactly one TypeAlias item
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "X");
            // The type should reference an associated type like <T as Promise>::Output
            match ty {
                RustType::Named { name, .. } => {
                    assert!(
                        name.contains("Promise") && name.contains("Output"),
                        "expected associated type path, got: {name}"
                    );
                }
                other => panic!("expected Named type, got: {other:?}"),
            }
        }
        other => panic!("expected TypeAlias item, got: {other:?}"),
    }

    // Synthetic registry should contain a stub trait for Promise
    assert!(
        !synthetic.all_items().is_empty(),
        "should register a synthetic stub trait for Promise"
    );
}

#[test]
fn test_convert_type_alias_conditional_type_true_false_literal_generates_bool() {
    // type X<T> = T extends Y ? true : false → bool
    let module = parse_type_annotation("type X<T> = T extends Y ? true : false;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let items = super::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .expect("should succeed");

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "X");
            assert_eq!(*ty, RustType::Bool);
        }
        other => panic!("expected TypeAlias item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_conditional_type_fallback_uses_true_branch() {
    // A conditional type where the true branch uses an unsupported type construct,
    // causing convert_conditional_type to fail → fallback: Comment + TypeAlias.
    // `T extends string ? (typeof unknownVar) : boolean` — typeof on unregistered
    // identifier fails in convert_ts_type, triggering the Err fallback path.
    let module =
        parse_type_annotation("type X<T> = T extends string ? typeof unknownVar : boolean;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let items = super::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .expect("should succeed");

    // Should produce Comment + TypeAlias (2 items)
    assert_eq!(
        items.len(),
        2,
        "expected Comment + TypeAlias, got: {items:?}"
    );
    assert!(
        matches!(&items[0], Item::Comment(c) if c.contains("Conditional type")),
        "first item should be a TODO comment, got: {:?}",
        items[0]
    );
    match &items[1] {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "X");
            // true branch conversion also fails → fallback to RustType::Any
            assert_eq!(*ty, RustType::Any);
        }
        other => panic!("expected TypeAlias item, got: {other:?}"),
    }
}

// ===========================================================================
// try_convert_keyof_typeof_alias (tests 4–6)
// ===========================================================================

#[test]
fn test_convert_type_alias_keyof_typeof_struct_generates_string_enum() {
    // type K = keyof typeof myStruct → enum with struct field names
    let mut reg = TypeRegistry::new();
    reg.register(
        "myStruct".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![
                ("alpha".to_string(), RustType::String),
                ("beta".to_string(), RustType::F64),
            ],
            methods: std::collections::HashMap::new(),
            constructor: None,
            extends: vec![],
            is_interface: false,
        },
    );

    let module = parse_type_annotation("type K = keyof typeof myStruct;");
    let alias = extract_type_alias(&module, 0);
    let mut synthetic = SyntheticTypeRegistry::new();

    let items = super::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .expect("should succeed");

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "K");
            let variant_names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
            assert!(variant_names.contains(&"alpha"), "should contain 'alpha'");
            assert!(variant_names.contains(&"beta"), "should contain 'beta'");
        }
        other => panic!("expected Enum item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_keyof_typeof_enum_generates_string_enum() {
    // type K = keyof typeof myEnum → enum with string_values
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("A".to_string(), "ValueA".to_string());
    string_values.insert("B".to_string(), "ValueB".to_string());
    reg.register(
        "myEnum".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["A".to_string(), "B".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let module = parse_type_annotation("type K = keyof typeof myEnum;");
    let alias = extract_type_alias(&module, 0);
    let mut synthetic = SyntheticTypeRegistry::new();

    let items = super::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .expect("should succeed");

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "K");
            let variant_names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
            assert!(
                variant_names.contains(&"ValueA") && variant_names.contains(&"ValueB"),
                "variants should come from string_values, got: {variant_names:?}"
            );
        }
        other => panic!("expected Enum item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_keyof_typeof_unknown_falls_through_to_error() {
    // type K = keyof typeof unknown_name → type not in registry
    // try_convert_keyof_typeof_alias returns None, falling through to normal
    // convert_type_alias which fails because `keyof typeof X` is unsupported
    let reg = TypeRegistry::new();
    let module = parse_type_annotation("type K = keyof typeof unknownName;");
    let alias = extract_type_alias(&module, 0);
    let mut synthetic = SyntheticTypeRegistry::new();

    let result = super::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    );
    assert!(
        result.is_err(),
        "should fail for unknown type in keyof typeof"
    );
}

// ===========================================================================
// convert_type_alias — TsTypeLit 3-way classification (tests 7–11)
// ===========================================================================

#[test]
fn test_convert_type_alias_call_signature_only_generates_fn_type() {
    // type F = { (x: string): number } → TypeAlias(Fn)
    let module = parse_type_annotation("type F = { (x: string): number };");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let item =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg)
            .expect("should succeed");

    match &item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "F");
            match ty {
                RustType::Fn {
                    params,
                    return_type,
                } => {
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0], RustType::String);
                    assert_eq!(**return_type, RustType::F64);
                }
                other => panic!("expected Fn type, got: {other:?}"),
            }
        }
        other => panic!("expected TypeAlias item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_methods_only_generates_trait() {
    // type T = { foo(): void; bar(): string } → Trait
    let module = parse_type_annotation("type T = { foo(): void; bar(): string };");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let item =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg)
            .expect("should succeed");

    match &item {
        Item::Trait { name, methods, .. } => {
            assert_eq!(name, "T");
            assert_eq!(methods.len(), 2);
            let method_names: Vec<&str> = methods.iter().map(|m| m.name.as_str()).collect();
            assert!(method_names.contains(&"foo"));
            assert!(method_names.contains(&"bar"));
        }
        other => panic!("expected Trait item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_properties_generates_struct() {
    // type T = { x: number; y: string } → Struct
    let module = parse_type_annotation("type T = { x: number; y: string };");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let item =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg)
            .expect("should succeed");

    match &item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "T");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, RustType::F64);
            assert_eq!(fields[1].name, "y");
            assert_eq!(fields[1].ty, RustType::String);
        }
        other => panic!("expected Struct item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_index_signature_generates_hashmap() {
    // type T = { [key: string]: number } → TypeAlias(HashMap)
    let module = parse_type_annotation("type T = { [key: string]: number };");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let item =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg)
            .expect("should succeed");

    match &item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "T");
            match ty {
                RustType::Named { name, type_args } => {
                    assert_eq!(name, "HashMap");
                    assert_eq!(type_args.len(), 2);
                    assert_eq!(type_args[0], RustType::String);
                    assert_eq!(type_args[1], RustType::F64);
                }
                other => panic!("expected Named(HashMap) type, got: {other:?}"),
            }
        }
        other => panic!("expected TypeAlias item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_index_signature_no_type_returns_error() {
    // Index signature without type annotation → Error
    // `type T = { [key: string] }` — SWC parses this but the index sig has no type_ann
    let module = parse_type_annotation("type T = { [key: string] };");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let result =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg);

    assert!(
        result.is_err(),
        "index signature without type annotation should fail"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("index signature") || err_msg.contains("unsupported"),
        "error should mention index signature, got: {err_msg}"
    );
}

// ===========================================================================
// Other type forms (tests 12–15)
// ===========================================================================

#[test]
fn test_convert_type_alias_function_type_generates_fn_alias() {
    // type F = (x: string) => number → TypeAlias(Fn)
    let module = parse_type_annotation("type F = (x: string) => number;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let item =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg)
            .expect("should succeed");

    match &item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "F");
            match ty {
                RustType::Fn {
                    params,
                    return_type,
                } => {
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0], RustType::String);
                    assert_eq!(**return_type, RustType::F64);
                }
                other => panic!("expected Fn type, got: {other:?}"),
            }
        }
        other => panic!("expected TypeAlias item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_tuple_type_generates_tuple() {
    // type T = [string, number] → TypeAlias(Tuple)
    let module = parse_type_annotation("type T = [string, number];");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let item =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg)
            .expect("should succeed");

    match &item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "T");
            match ty {
                RustType::Tuple(elems) => {
                    assert_eq!(elems.len(), 2);
                    assert_eq!(elems[0], RustType::String);
                    assert_eq!(elems[1], RustType::F64);
                }
                other => panic!("expected Tuple type, got: {other:?}"),
            }
        }
        other => panic!("expected TypeAlias item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_single_string_literal_generates_enum() {
    // type X = "only" → Enum with 1 variant
    let module = parse_type_annotation("type X = \"only\";");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let item =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg)
            .expect("should succeed");

    match &item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "X");
            assert_eq!(variants.len(), 1, "should have exactly 1 variant");
            assert_eq!(variants[0].name, "Only");
        }
        other => panic!("expected Enum item, got: {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_unsupported_fn_param_pattern_returns_error() {
    // fn type param with destructuring pattern (not Ident) → Error
    // `type F = ({x}: {x: string}) => number` — param is ObjectPat, not Ident
    let module = parse_type_annotation("type F = ({x}: {x: string}) => number;");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let result =
        super::type_aliases::convert_type_alias(alias, Visibility::Public, &mut synthetic, &reg);

    assert!(
        result.is_err(),
        "destructured fn param pattern should return error"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("unsupported function type parameter pattern"),
        "error should mention unsupported param pattern, got: {err_msg}"
    );
}

// ─── Intersection preprocessing and distribution tests ───

/// Helper: extract the type alias Item from a type alias declaration.
fn convert_type_alias_from_source(source: &str) -> crate::ir::Item {
    let module = parse_type_annotation(source);
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();

    for item in &module.body {
        if let swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
            swc_ecma_ast::Decl::TsTypeAlias(decl),
        )) = item
        {
            return crate::pipeline::type_converter::convert_type_alias(
                decl,
                crate::ir::Visibility::Private,
                &mut synthetic,
                &reg,
            )
            .unwrap();
        }
    }
    panic!("no type alias found in source");
}

#[test]
fn test_intersection_identity_mapped_type_simplification() {
    let item = convert_type_alias_from_source("type Simplify<T> = { [K in keyof T]: T[K] } & {};");
    match item {
        crate::ir::Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "Simplify");
            assert_eq!(
                ty,
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![]
                }
            );
        }
        other => panic!("expected TypeAlias, got {other:?}"),
    }
}

#[test]
fn test_intersection_identity_mapped_type_with_modifier_not_simplified() {
    // readonly modifier prevents identity simplification
    let item = convert_type_alias_from_source(
        "type ReadonlyAll<T> = { readonly [K in keyof T]: T[K] } & {};",
    );
    // Should NOT be TypeAlias { ty: T } — the readonly modifier makes it non-identity
    assert!(
        !matches!(
            &item,
            crate::ir::Item::TypeAlias { ty: RustType::Named { name, type_args }, .. }
            if name == "T" && type_args.is_empty()
        ),
        "readonly mapped type should not be simplified to T"
    );
}

#[test]
fn test_intersection_empty_object_removal_produces_struct() {
    let item = convert_type_alias_from_source("type A = { x: number } & {};");
    match item {
        crate::ir::Item::Struct { name, fields, .. } => {
            assert_eq!(name, "A");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "x");
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_intersection_union_distribution_produces_enum() {
    let item = convert_type_alias_from_source(
        "type X = { base: string } & ({ a: number } | { b: boolean });",
    );
    match item {
        crate::ir::Item::Enum {
            name,
            serde_tag,
            variants,
            ..
        } => {
            assert_eq!(name, "X");
            assert!(serde_tag.is_none(), "non-discriminated should have no tag");
            assert_eq!(variants.len(), 2);
            // Each variant should have base field
            for v in &variants {
                assert!(
                    v.fields.iter().any(|f| f.name == "base"),
                    "variant {} missing base field",
                    v.name
                );
            }
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_intersection_union_discriminated_produces_tagged_enum() {
    let item = convert_type_alias_from_source(
        r#"type D = { base: string } & ({ kind: "a"; x: number } | { kind: "b"; y: string });"#,
    );
    match item {
        crate::ir::Item::Enum {
            serde_tag,
            variants,
            ..
        } => {
            assert_eq!(serde_tag.as_deref(), Some("kind"));
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "A");
            assert_eq!(variants[1].name, "B");
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_intersection_union_duplicate_field_variant_overrides_base() {
    let item = convert_type_alias_from_source(
        "type D = { name: string; age: number } & ({ name: number; x: boolean } | { y: string });",
    );
    match item {
        crate::ir::Item::Enum { variants, .. } => {
            // Variant0 has name: number from variant (overrides base name: string)
            let v0 = &variants[0];
            let name_field = v0.fields.iter().find(|f| f.name == "name").unwrap();
            assert_eq!(
                name_field.ty,
                RustType::F64,
                "variant field should override base"
            );
            // name should appear only once
            assert_eq!(
                v0.fields.iter().filter(|f| f.name == "name").count(),
                1,
                "name should not be duplicated"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_intersection_fallback_mapped_type_produces_embedded_field() {
    let item =
        convert_type_alias_from_source("type M<T> = { x: string } & { [K in keyof T]: T[K] };");
    match item {
        crate::ir::Item::Struct { fields, .. } => {
            // Should have x field + embedded _1 field
            assert!(fields.iter().any(|f| f.name == "x"));
            assert!(
                fields.iter().any(|f| f.name == "_1"),
                "mapped type member should be embedded as _1"
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_discriminated_union_duplicate_discriminant_falls_back_to_general() {
    // Duplicate discriminant value "a" → should NOT produce serde-tagged enum
    let item = convert_type_alias_from_source(
        r#"type Dup = { kind: "a"; x: number } | { kind: "a"; y: string };"#,
    );
    match &item {
        crate::ir::Item::Enum { serde_tag, .. } => {
            assert!(
                serde_tag.is_none(),
                "duplicate discriminant values should produce non-tagged enum, got tag: {:?}",
                serde_tag
            );
        }
        _ => {
            // General union or other representation — acceptable as long as it's not tagged
        }
    }
}

#[test]
fn test_discriminated_union_unique_discriminant_produces_tagged_enum() {
    let item = convert_type_alias_from_source(
        r#"type DU = { kind: "a"; x: number } | { kind: "b"; y: string };"#,
    );
    match item {
        crate::ir::Item::Enum {
            serde_tag,
            variants,
            ..
        } => {
            assert_eq!(serde_tag.as_deref(), Some("kind"));
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "A");
            assert_eq!(variants[1].name, "B");
        }
        other => panic!("expected tagged Enum, got {other:?}"),
    }
}
