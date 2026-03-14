use super::*;
use crate::ir::StructField;
use crate::parser::parse_typescript;
use swc_ecma_ast::{Decl, ModuleItem, Stmt};

/// Helper: parse TS source and extract the first TsInterfaceDecl.
fn parse_interface(source: &str) -> TsInterfaceDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(decl))) => *decl.clone(),
        _ => panic!("expected TsInterfaceDecl"),
    }
}

/// Helper: parse TS source and extract the first TsTypeAliasDecl.
fn parse_type_alias(source: &str) -> TsTypeAliasDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(decl))) => *decl.clone(),
        _ => panic!("expected TsTypeAliasDecl"),
    }
}

// -- convert_ts_type tests --

#[test]
fn test_convert_ts_type_string() {
    let decl = parse_interface("interface T { x: string; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::String);
}

#[test]
fn test_convert_ts_type_number() {
    let decl = parse_interface("interface T { x: number; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::F64);
}

#[test]
fn test_convert_ts_type_boolean() {
    let decl = parse_interface("interface T { x: boolean; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Bool);
}

#[test]
fn test_convert_ts_type_array_bracket() {
    let decl = parse_interface("interface T { x: string[]; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Vec(Box::new(RustType::String)));
}

#[test]
fn test_convert_ts_type_array_generic() {
    let decl = parse_interface("interface T { x: Array<number>; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Vec(Box::new(RustType::F64)));
}

#[test]
fn test_convert_ts_type_union_null() {
    let decl = parse_interface("interface T { x: string | null; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Option(Box::new(RustType::String)));
}

#[test]
fn test_convert_ts_type_union_undefined() {
    let decl = parse_interface("interface T { x: number | undefined; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Option(Box::new(RustType::F64)));
}

// -- convert_interface tests --

#[test]
fn test_convert_interface_basic() {
    let decl = parse_interface("interface Foo { name: string; age: number; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Struct {
            vis,
            name,
            type_params,
            fields,
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Foo");
            assert!(type_params.is_empty());
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "name");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "age");
            assert_eq!(fields[1].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_optional_field() {
    let decl = parse_interface("interface Bar { label?: string; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].name, "label");
            assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_optional_union_null_no_double_wrap() {
    // `name?: string | null` should be `Option<String>`, not `Option<Option<String>>`
    let decl = parse_interface("interface Baz { name?: string | null; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_vec_field() {
    let decl = parse_interface("interface Qux { items: number[]; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].ty, RustType::Vec(Box::new(RustType::F64)));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_with_type_params() {
    let decl = parse_interface("interface Container<T> { value: T; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Struct { type_params, .. } => {
            assert_eq!(type_params, vec!["T".to_string()]);
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_with_multiple_type_params() {
    let decl = parse_interface("interface Pair<A, B> { first: A; second: B; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Struct { type_params, .. } => {
            assert_eq!(type_params, vec!["A".to_string(), "B".to_string()]);
        }
        _ => panic!("expected Item::Struct"),
    }
}

// -- convert_interface with method signatures --

#[test]
fn test_convert_interface_method_only_generates_trait() {
    let decl = parse_interface("interface Greeter { greet(name: string): string; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Trait { vis, name, methods } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Greeter");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "greet");
            assert!(methods[0].has_self);
            assert_eq!(methods[0].params.len(), 1);
            assert_eq!(methods[0].params[0].name, "name");
            assert_eq!(methods[0].params[0].ty, Some(RustType::String));
            assert_eq!(methods[0].return_type, Some(RustType::String));
        }
        _ => panic!("expected Item::Trait, got {:?}", item),
    }
}

#[test]
fn test_convert_interface_method_no_args_void_return() {
    let decl = parse_interface("interface Runner { run(): void; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Trait { methods, .. } => {
            assert_eq!(methods[0].name, "run");
            assert!(methods[0].has_self);
            assert!(methods[0].params.is_empty());
            assert_eq!(methods[0].return_type, None);
        }
        _ => panic!("expected Item::Trait"),
    }
}

#[test]
fn test_convert_interface_method_multiple_params() {
    let decl = parse_interface("interface Math { add(a: number, b: number): number; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Trait { methods, .. } => {
            assert_eq!(methods[0].params.len(), 2);
            assert_eq!(methods[0].params[0].name, "a");
            assert_eq!(methods[0].params[1].name, "b");
            assert_eq!(methods[0].return_type, Some(RustType::F64));
        }
        _ => panic!("expected Item::Trait"),
    }
}

#[test]
fn test_convert_interface_properties_only_still_struct() {
    let decl = parse_interface("interface Point { x: number; y: number; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    assert!(matches!(item, Item::Struct { .. }));
}

#[test]
fn test_convert_interface_method_with_type_params() {
    let decl = parse_interface("interface Repo<T> { find(id: string): T; save(item: T): void; }");
    let item = convert_interface(&decl, Visibility::Public).unwrap();

    match item {
        Item::Trait { name, methods, .. } => {
            assert_eq!(name, "Repo");
            assert_eq!(methods.len(), 2);
            assert_eq!(methods[0].name, "find");
            assert_eq!(methods[1].name, "save");
        }
        _ => panic!("expected Item::Trait"),
    }
}

// -- convert_type_alias tests --

#[test]
fn test_convert_type_alias_object_literal() {
    let decl = parse_type_alias("type Point = { x: number; y: number; };");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();

    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, RustType::F64);
            assert_eq!(fields[1].name, "y");
            assert_eq!(fields[1].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_type_alias_with_type_params() {
    let decl = parse_type_alias("type Pair<A, B> = { first: A; second: B; };");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();

    match item {
        Item::Struct { type_params, .. } => {
            assert_eq!(type_params, vec!["A".to_string(), "B".to_string()]);
        }
        _ => panic!("expected Item::Struct"),
    }
}

// -- convert_ts_type: generic type arguments --

#[test]
fn test_convert_ts_type_named_with_type_args() {
    // `Container<string>` should become Named { name: "Container", type_args: [String] }
    let decl = parse_interface("interface T { x: Container<string>; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(
        ty,
        RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::String],
        }
    );
}

#[test]
fn test_convert_ts_type_named_with_multiple_type_args() {
    let decl = parse_interface("interface T { x: Pair<string, number>; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(
        ty,
        RustType::Named {
            name: "Pair".to_string(),
            type_args: vec![RustType::String, RustType::F64],
        }
    );
}

#[test]
fn test_convert_ts_type_named_without_type_args() {
    let decl = parse_interface("interface T { x: Point; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(
        ty,
        RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        }
    );
}

// -- convert_ts_type: function types --

#[test]
fn test_convert_ts_type_fn_type() {
    // `callback: (x: number) => string` → Fn { params: [F64], return_type: String }
    let decl = parse_interface("interface T { callback: (x: number) => string; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(
        ty,
        RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::String),
        }
    );
}

#[test]
fn test_convert_ts_type_fn_type_no_params() {
    let decl = parse_interface("interface T { callback: () => boolean; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(
        ty,
        RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::Bool),
        }
    );
}

// -- convert_ts_type: keyword types (any, unknown, never) --

#[test]
fn test_convert_ts_type_any() {
    let decl = parse_interface("interface T { x: any; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Any);
}

#[test]
fn test_convert_ts_type_unknown() {
    let decl = parse_interface("interface T { x: unknown; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Any);
}

#[test]
fn test_convert_ts_type_never() {
    let decl = parse_interface("interface T { x: never; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Never);
}

// -- convert_type_alias: string literal union --

#[test]
fn test_convert_type_alias_string_literal_union_produces_enum() {
    let decl = parse_type_alias(r#"type Direction = "up" | "down" | "left" | "right";"#);
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Direction");
            assert_eq!(variants.len(), 4);
            assert_eq!(variants[0].name, "Up");
            assert_eq!(
                variants[0].value,
                Some(crate::ir::EnumValue::Str("up".to_string()))
            );
            assert_eq!(variants[1].name, "Down");
            assert_eq!(variants[2].name, "Left");
            assert_eq!(variants[3].name, "Right");
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_string_literal_union_two_members() {
    let decl = parse_type_alias(r#"type Status = "active" | "inactive";"#);
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Status");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Active");
            assert_eq!(variants[1].name, "Inactive");
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_string_literal_union_single_member() {
    let decl = parse_type_alias(r#"type Only = "only";"#);
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Only");
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].name, "Only");
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_string_literal_union_kebab_case() {
    let decl = parse_type_alias(r#"type X = "foo-bar" | "baz-qux";"#);
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].name, "FooBar");
            assert_eq!(variants[1].name, "BazQux");
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_numeric_literal_union_produces_enum() {
    let decl = parse_type_alias("type Code = 200 | 404 | 500;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Code");
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "V200");
            assert_eq!(variants[0].value, Some(EnumValue::Number(200)));
            assert!(variants[0].data.is_none());
            assert_eq!(variants[1].name, "V404");
            assert_eq!(variants[1].value, Some(EnumValue::Number(404)));
            assert_eq!(variants[2].name, "V500");
            assert_eq!(variants[2].value, Some(EnumValue::Number(500)));
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_numeric_literal_union_two_members() {
    let decl = parse_type_alias("type Code = 200 | 404;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Code");
            assert_eq!(variants.len(), 2);
        }
        _ => panic!("expected Item::Enum"),
    }
}

// -- convert_type_alias: primitive union --

#[test]
fn test_convert_type_alias_primitive_union_two_types() {
    let decl = parse_type_alias("type Value = string | number;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Value");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[0].data, Some(RustType::String));
            assert!(variants[0].value.is_none());
            assert_eq!(variants[1].name, "F64");
            assert_eq!(variants[1].data, Some(RustType::F64));
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_primitive_union_three_types() {
    let decl = parse_type_alias("type Any = string | number | boolean;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Any");
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[1].name, "F64");
            assert_eq!(variants[2].name, "Bool");
        }
        _ => panic!("expected Item::Enum"),
    }
}

// -- convert_type_alias: mixed union --

#[test]
fn test_convert_type_alias_mixed_union_string_and_number_literal() {
    let decl = parse_type_alias(r#"type Mixed = "ok" | 404;"#);
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Mixed");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Ok");
            assert_eq!(variants[0].value, Some(EnumValue::Str("ok".to_string())));
            assert!(variants[0].data.is_none());
            assert_eq!(variants[1].name, "V404");
            assert_eq!(variants[1].value, Some(EnumValue::Number(404)));
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_nullable_union_with_multiple_types() {
    // `type Opt = string | number | null` → enum (nullable wrapping is future work)
    let decl = parse_type_alias("type Opt = string | number | null;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Opt");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[1].name, "F64");
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_non_object_returns_error() {
    let decl = parse_type_alias("type Name = string;");
    let result = convert_type_alias(&decl, Visibility::Public);
    assert!(result.is_err());
}

#[test]
fn test_convert_ts_type_void_returns_unit() {
    let decl = parse_interface("interface T { callback: () => void; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    // The callback type is `() => void`, which is a TsFnType
    // whose return type is void. We check the return type is Unit.
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(
        ty,
        RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::Unit),
        }
    );
}

// -- convert_type_alias: function type body --

#[test]
fn test_convert_type_alias_function_type_single_param() {
    let decl = parse_type_alias("type Handler = (req: Request) => Response;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();

    match item {
        Item::TypeAlias {
            vis,
            name,
            type_params,
            ty,
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Handler");
            assert!(type_params.is_empty());
            assert_eq!(
                ty,
                RustType::Fn {
                    params: vec![RustType::Named {
                        name: "Request".to_string(),
                        type_args: vec![],
                    }],
                    return_type: Box::new(RustType::Named {
                        name: "Response".to_string(),
                        type_args: vec![],
                    }),
                }
            );
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_function_type_no_params() {
    let decl = parse_type_alias("type Factory = () => Widget;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();

    match item {
        Item::TypeAlias { ty, .. } => {
            assert_eq!(
                ty,
                RustType::Fn {
                    params: vec![],
                    return_type: Box::new(RustType::Named {
                        name: "Widget".to_string(),
                        type_args: vec![],
                    }),
                }
            );
        }
        _ => panic!("expected Item::TypeAlias"),
    }
}

#[test]
fn test_convert_type_alias_function_type_void_return() {
    let decl = parse_type_alias("type Callback = (x: number) => void;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();

    match item {
        Item::TypeAlias { ty, .. } => {
            assert_eq!(
                ty,
                RustType::Fn {
                    params: vec![RustType::F64],
                    return_type: Box::new(RustType::Unit),
                }
            );
        }
        _ => panic!("expected Item::TypeAlias"),
    }
}

#[test]
fn test_convert_type_alias_function_type_multiple_params() {
    let decl = parse_type_alias("type ErrorHandler = (err: string, ctx: Context) => Response;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();

    match item {
        Item::TypeAlias { ty, .. } => match ty {
            RustType::Fn { params, .. } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], RustType::String);
            }
            _ => panic!("expected RustType::Fn"),
        },
        _ => panic!("expected Item::TypeAlias"),
    }
}

// -- convert_ts_type: tuple types --

#[test]
fn test_convert_ts_type_tuple_two_elements() {
    let decl = parse_interface("interface T { x: [string, number]; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Tuple(vec![RustType::String, RustType::F64]));
}

#[test]
fn test_convert_ts_type_tuple_single_element() {
    let decl = parse_interface("interface T { x: [boolean]; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Tuple(vec![RustType::Bool]));
}

#[test]
fn test_convert_ts_type_tuple_empty() {
    let decl = parse_interface("interface T { x: []; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(ty, RustType::Tuple(vec![]));
}

#[test]
fn test_convert_ts_type_tuple_nested() {
    let decl = parse_interface("interface T { x: [[string, number], boolean]; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(
        ty,
        RustType::Tuple(vec![
            RustType::Tuple(vec![RustType::String, RustType::F64]),
            RustType::Bool,
        ])
    );
}

#[test]
fn test_convert_type_alias_tuple_type() {
    let decl = parse_type_alias("type Pair = [string, number];");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();

    match item {
        Item::TypeAlias {
            vis,
            name,
            type_params,
            ty,
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Pair");
            assert!(type_params.is_empty());
            assert_eq!(ty, RustType::Tuple(vec![RustType::String, RustType::F64]));
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_function_type_with_generics() {
    let decl = parse_type_alias("type Mapper<T, U> = (item: T) => U;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();

    match item {
        Item::TypeAlias { type_params, .. } => {
            assert_eq!(type_params, vec!["T".to_string(), "U".to_string()]);
        }
        _ => panic!("expected Item::TypeAlias"),
    }
}

#[test]
fn test_convert_ts_type_indexed_access_string_key_returns_associated_type() {
    let decl = parse_interface("interface T { x: E['Bindings']; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
    assert_eq!(
        ty,
        RustType::Named {
            name: "E::Bindings".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn test_convert_ts_type_indexed_access_non_string_key_returns_error() {
    let decl = parse_interface("interface T { x: E[0]; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let result = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann);
    assert!(result.is_err());
}

#[test]
fn test_convert_type_alias_conditional_filter_returns_type_alias_with_true_branch() {
    let decl = parse_type_alias("type Filter<T> = T extends string ? T : never;");
    let items = convert_type_alias_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Filter".to_string(),
            type_params: vec!["T".to_string()],
            ty: RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            },
        }
    );
}

#[test]
fn test_convert_type_alias_conditional_simple_returns_type_alias_with_true_branch() {
    let decl = parse_type_alias("type ToNum<T> = T extends string ? number : boolean;");
    let items = convert_type_alias_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "ToNum".to_string(),
            type_params: vec!["T".to_string()],
            ty: RustType::F64,
        }
    );
}

#[test]
fn test_convert_type_alias_conditional_predicate_returns_bool() {
    let decl = parse_type_alias("type IsString<T> = T extends string ? true : false;");
    let items = convert_type_alias_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "IsString".to_string(),
            type_params: vec!["T".to_string()],
            ty: RustType::Bool,
        }
    );
}

#[test]
fn test_convert_type_alias_conditional_infer_returns_associated_type() {
    let decl = parse_type_alias("type Unwrap<T> = T extends Promise<infer U> ? U : never;");
    let items = convert_type_alias_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Unwrap".to_string(),
            type_params: vec!["T".to_string()],
            ty: RustType::Named {
                name: "<T as Promise>::Output".to_string(),
                type_args: vec![],
            },
        }
    );
}

#[test]
fn test_convert_type_alias_conditional_nested_generates_comment_and_placeholder() {
    // Nested conditional types are not supported by Tier 1 — should produce fallback
    let decl = parse_type_alias(
        "type Foo<T> = T extends string ? T extends \"a\" ? number : boolean : never;",
    );
    let items = convert_type_alias_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 2);
    // First item should be a comment containing original TS info
    match &items[0] {
        Item::Comment(text) => {
            assert!(text.contains("TODO"));
            assert!(text.contains("Foo"));
        }
        _ => panic!("expected Item::Comment, got {:?}", items[0]),
    }
    // Second item should be a placeholder TypeAlias with Unit type
    assert_eq!(
        items[1],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec!["T".to_string()],
            ty: RustType::Unit,
        }
    );
}

#[test]
fn test_convert_type_alias_discriminated_union_two_variants_generates_serde_tagged_enum() {
    let decl = parse_type_alias(
        r#"type Event = { kind: "click", x: number } | { kind: "hover", y: number };"#,
    );
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    assert_eq!(
        item,
        Item::Enum {
            vis: Visibility::Public,
            name: "Event".to_string(),
            serde_tag: Some("kind".to_string()),
            variants: vec![
                EnumVariant {
                    name: "Click".to_string(),
                    value: Some(EnumValue::Str("click".to_string())),
                    data: None,
                    fields: vec![StructField {
                        name: "x".to_string(),
                        ty: RustType::F64,
                    }],
                },
                EnumVariant {
                    name: "Hover".to_string(),
                    value: Some(EnumValue::Str("hover".to_string())),
                    data: None,
                    fields: vec![StructField {
                        name: "y".to_string(),
                        ty: RustType::F64,
                    }],
                },
            ],
        }
    );
}

#[test]
fn test_convert_type_alias_discriminated_union_three_variants_generates_serde_tagged_enum() {
    let decl = parse_type_alias(
        r#"type Shape = { tag: "circle", r: number } | { tag: "rect", w: number, h: number } | { tag: "line" };"#,
    );
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match &item {
        Item::Enum {
            serde_tag,
            variants,
            ..
        } => {
            assert_eq!(serde_tag, &Some("tag".to_string()));
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "Circle");
            assert_eq!(variants[0].fields.len(), 1); // r
            assert_eq!(variants[1].name, "Rect");
            assert_eq!(variants[1].fields.len(), 2); // w, h
            assert_eq!(variants[2].name, "Line");
            assert!(variants[2].fields.is_empty());
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_discriminated_union_no_extra_fields_generates_unit_variants() {
    let decl = parse_type_alias(r#"type Status = { kind: "active" } | { kind: "inactive" };"#);
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match &item {
        Item::Enum {
            serde_tag,
            variants,
            ..
        } => {
            assert_eq!(serde_tag, &Some("kind".to_string()));
            assert!(variants.iter().all(|v| v.fields.is_empty()));
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_discriminated_union_tag_field_type_generates_serde_tag() {
    let decl = parse_type_alias(
        r#"type Msg = { type: "text", body: string } | { type: "image", url: string };"#,
    );
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match &item {
        Item::Enum { serde_tag, .. } => {
            assert_eq!(serde_tag, &Some("type".to_string()));
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_union_without_common_discriminant_falls_through() {
    // No common string literal field → should fall through to existing union handling
    let decl = parse_type_alias(r#"type Mixed = { x: number } | { y: string };"#);
    let result = convert_type_alias(&decl, Visibility::Public);
    // This should not produce a discriminated union — it may error or produce a different Item
    // The key assertion is that it does NOT produce an Enum with serde_tag
    if let Ok(Item::Enum { serde_tag, .. }) = result {
        assert_eq!(serde_tag, None);
    }
}

#[test]
fn test_convert_interface_call_signature_single_generates_fn_type_alias() {
    let decl = parse_interface("interface Callback { (x: number): string }");
    let items = convert_interface_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Callback".to_string(),
            type_params: vec![],
            ty: RustType::Fn {
                params: vec![RustType::F64],
                return_type: Box::new(RustType::String),
            },
        }
    );
}

#[test]
fn test_convert_interface_call_signature_overload_uses_longest() {
    let decl = parse_interface(
        "interface Overloaded { (x: number): string; (x: number, y: string): boolean }",
    );
    let items = convert_interface_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::TypeAlias { ty, .. } => match ty {
            RustType::Fn { params, .. } => {
                assert_eq!(params.len(), 2);
            }
            _ => panic!("expected RustType::Fn"),
        },
        _ => panic!("expected Item::TypeAlias"),
    }
}

#[test]
fn test_convert_interface_call_signature_no_params_generates_fn_type() {
    let decl = parse_interface("interface Factory { (): void }");
    let items = convert_interface_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Factory".to_string(),
            type_params: vec![],
            ty: RustType::Fn {
                params: vec![],
                return_type: Box::new(RustType::Unit),
            },
        }
    );
}

#[test]
fn test_convert_interface_mixed_props_and_methods_generates_struct_and_trait() {
    let decl = parse_interface("interface Ctx { name: string; greet(msg: string): void }");
    let items = convert_interface_items(&decl, Visibility::Public).unwrap();
    assert_eq!(items.len(), 3);
    // First: struct with properties
    match &items[0] {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "Ctx");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "name");
        }
        _ => panic!("expected Item::Struct, got {:?}", items[0]),
    }
    // Second: trait with methods
    match &items[1] {
        Item::Trait { name, methods, .. } => {
            assert_eq!(name, "CtxTrait");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "greet");
        }
        _ => panic!("expected Item::Trait, got {:?}", items[1]),
    }
    // Third: impl trait for struct
    match &items[2] {
        Item::Impl {
            struct_name,
            for_trait,
            ..
        } => {
            assert_eq!(struct_name, "Ctx");
            assert_eq!(for_trait.as_deref(), Some("CtxTrait"));
        }
        _ => panic!("expected Item::Impl, got {:?}", items[2]),
    }
}

// -- convert_type_alias: union with type references --

#[test]
fn test_convert_type_alias_union_type_refs_generates_data_enum() {
    let decl = parse_type_alias("type R = Success | Failure;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "R");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Success");
            assert_eq!(
                variants[0].data,
                Some(RustType::Named {
                    name: "Success".to_string(),
                    type_args: vec![],
                })
            );
            assert!(variants[0].value.is_none());
            assert_eq!(variants[1].name, "Failure");
            assert_eq!(
                variants[1].data,
                Some(RustType::Named {
                    name: "Failure".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_union_type_ref_and_keyword_generates_data_enum() {
    let decl = parse_type_alias("type V = string | MyType;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "V");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[0].data, Some(RustType::String));
            assert_eq!(variants[1].name, "MyType");
            assert_eq!(
                variants[1].data,
                Some(RustType::Named {
                    name: "MyType".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_union_generic_type_ref_generates_data_enum() {
    let decl = parse_type_alias("type R = Response | Promise<Response>;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "R");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Response");
            assert_eq!(
                variants[0].data,
                Some(RustType::Named {
                    name: "Response".to_string(),
                    type_args: vec![],
                })
            );
            assert_eq!(variants[1].name, "Promise");
            assert_eq!(
                variants[1].data,
                Some(RustType::Named {
                    name: "Promise".to_string(),
                    type_args: vec![RustType::Named {
                        name: "Response".to_string(),
                        type_args: vec![],
                    }],
                })
            );
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

// -- intersection type tests --

#[test]
fn test_convert_type_alias_intersection_two_type_lits_generates_struct() {
    let decl = parse_type_alias("type Combined = { name: string } & { age: number };");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "Combined");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "name");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "age");
            assert_eq!(fields[1].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_intersection_three_type_lits_generates_struct() {
    let decl = parse_type_alias("type C = { a: string } & { b: number } & { c: boolean };");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "C");
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0].name, "a");
            assert_eq!(fields[1].name, "b");
            assert_eq!(fields[2].name, "c");
        }
        _ => panic!("expected Item::Struct, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_intersection_optional_field_generates_option() {
    let decl = parse_type_alias("type C = { name: string } & { nick?: string };");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "name");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "nick");
            assert_eq!(fields[1].ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::Struct, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_intersection_duplicate_field_returns_error() {
    let decl = parse_type_alias("type C = { x: string } & { x: number };");
    let result = convert_type_alias(&decl, Visibility::Public);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("duplicate field"),
        "expected 'duplicate field' in error, got: {err_msg}"
    );
}

#[test]
fn test_convert_type_alias_intersection_type_ref_returns_error() {
    let decl = parse_type_alias("type C = Foo & Bar;");
    let result = convert_type_alias(&decl, Visibility::Public);
    assert!(result.is_err());
}

#[test]
fn test_convert_ts_type_intersection_returns_error_with_clear_message() {
    let decl = parse_interface("interface T { x: { a: string } & { b: number }; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let result = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("intersection"),
        "expected 'intersection' in error, got: {err_msg}"
    );
}

// -- nullable union type alias tests --

#[test]
fn test_convert_type_alias_nullable_single_keyword_generates_option_alias() {
    let decl = parse_type_alias("type MaybeString = string | null;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "MaybeString");
            assert_eq!(ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_nullable_single_type_ref_generates_option_alias() {
    let decl = parse_type_alias("type MaybeUser = MyType | null;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "MaybeUser");
            assert_eq!(
                ty,
                RustType::Option(Box::new(RustType::Named {
                    name: "MyType".to_string(),
                    type_args: vec![],
                }))
            );
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_nullable_undefined_generates_option_alias() {
    let decl = parse_type_alias("type MaybeNum = number | undefined;");
    let item = convert_type_alias(&decl, Visibility::Public).unwrap();
    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "MaybeNum");
            assert_eq!(ty, RustType::Option(Box::new(RustType::F64)));
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}
