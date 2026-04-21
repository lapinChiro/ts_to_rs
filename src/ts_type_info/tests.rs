//! Unit tests for `convert_to_ts_type_info`.
//!
//! One test per `TsTypeInfo` variant family (keyword / composite /
//! reference / literal / structural / advanced). The shared `parse_type`
//! helper wraps a `type X = <src>` alias and extracts the inner
//! `TsType`.

use super::*;

fn parse_type(src: &str) -> swc_ecma_ast::TsType {
    // Parse `type X = <src>` and extract the type
    let full = format!("type X = {src};");
    let module = crate::parser::parse_typescript(&full).expect("parse failed");
    let decl = match &module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
            swc_ecma_ast::Decl::TsTypeAlias(alias),
        )) => alias,
        _ => panic!("expected type alias"),
    };
    decl.type_ann.as_ref().clone()
}

#[test]
fn keyword_types() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("string")).unwrap(),
        TsTypeInfo::String
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("number")).unwrap(),
        TsTypeInfo::Number
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("boolean")).unwrap(),
        TsTypeInfo::Boolean
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("void")).unwrap(),
        TsTypeInfo::Void
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("null")).unwrap(),
        TsTypeInfo::Null
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("undefined")).unwrap(),
        TsTypeInfo::Undefined
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("never")).unwrap(),
        TsTypeInfo::Never
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("any")).unwrap(),
        TsTypeInfo::Any
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("unknown")).unwrap(),
        TsTypeInfo::Unknown
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("object")).unwrap(),
        TsTypeInfo::Object
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("bigint")).unwrap(),
        TsTypeInfo::BigInt
    );
}

#[test]
fn array_type() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("string[]")).unwrap(),
        TsTypeInfo::Array(Box::new(TsTypeInfo::String))
    );
}

#[test]
fn type_ref() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("Foo")).unwrap(),
        TsTypeInfo::TypeRef {
            name: "Foo".to_string(),
            type_args: vec![],
        }
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("Map<string, number>")).unwrap(),
        TsTypeInfo::TypeRef {
            name: "Map".to_string(),
            type_args: vec![TsTypeInfo::String, TsTypeInfo::Number],
        }
    );
}

#[test]
fn union_type() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("string | number")).unwrap(),
        TsTypeInfo::Union(vec![TsTypeInfo::String, TsTypeInfo::Number])
    );
}

#[test]
fn intersection_type() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("Foo & Bar")).unwrap(),
        TsTypeInfo::Intersection(vec![
            TsTypeInfo::TypeRef {
                name: "Foo".to_string(),
                type_args: vec![]
            },
            TsTypeInfo::TypeRef {
                name: "Bar".to_string(),
                type_args: vec![]
            },
        ])
    );
}

#[test]
fn tuple_type() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("[string, number]")).unwrap(),
        TsTypeInfo::Tuple(vec![TsTypeInfo::String, TsTypeInfo::Number])
    );
}

#[test]
fn function_type() {
    let info = convert_to_ts_type_info(&parse_type("(x: string) => number")).unwrap();
    assert_eq!(
        info,
        TsTypeInfo::Function {
            params: vec![TsParamInfo {
                name: "x".to_string(),
                ty: TsTypeInfo::String,
                optional: false,
            }],
            return_type: Box::new(TsTypeInfo::Number),
        }
    );
}

#[test]
fn function_type_with_optional_param() {
    let info = convert_to_ts_type_info(&parse_type("(x: string, y?: number) => void")).unwrap();
    assert_eq!(
        info,
        TsTypeInfo::Function {
            params: vec![
                TsParamInfo {
                    name: "x".to_string(),
                    ty: TsTypeInfo::String,
                    optional: false,
                },
                TsParamInfo {
                    name: "y".to_string(),
                    ty: TsTypeInfo::Number,
                    optional: true,
                }
            ],
            return_type: Box::new(TsTypeInfo::Void),
        }
    );
}

#[test]
fn literal_types() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("\"hello\"")).unwrap(),
        TsTypeInfo::Literal(TsLiteralKind::String("hello".to_string()))
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("42")).unwrap(),
        TsTypeInfo::Literal(TsLiteralKind::Number(42.0))
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("true")).unwrap(),
        TsTypeInfo::Literal(TsLiteralKind::Boolean(true))
    );
}

#[test]
fn type_literal_properties() {
    let info = convert_to_ts_type_info(&parse_type("{ name: string; age?: number }")).unwrap();
    match &info {
        TsTypeInfo::TypeLiteral(lit) => {
            assert_eq!(lit.fields.len(), 2);
            assert_eq!(lit.fields[0].name, "name");
            assert_eq!(lit.fields[0].ty, TsTypeInfo::String);
            assert!(!lit.fields[0].optional);
            assert_eq!(lit.fields[1].name, "age");
            assert_eq!(lit.fields[1].ty, TsTypeInfo::Number);
            assert!(lit.fields[1].optional);
            assert!(lit.methods.is_empty());
            assert!(lit.call_signatures.is_empty());
            assert!(lit.construct_signatures.is_empty());
            assert!(lit.index_signatures.is_empty());
        }
        _ => panic!("expected TypeLiteral, got {:?}", info),
    }
}

#[test]
fn type_literal_methods() {
    let info = convert_to_ts_type_info(&parse_type("{ greet(name: string): void }")).unwrap();
    match &info {
        TsTypeInfo::TypeLiteral(lit) => {
            assert!(lit.fields.is_empty());
            assert_eq!(lit.methods.len(), 1);
            assert_eq!(lit.methods[0].name, "greet");
            assert_eq!(lit.methods[0].params.len(), 1);
            assert_eq!(lit.methods[0].params[0].name, "name");
            assert_eq!(lit.methods[0].params[0].ty, TsTypeInfo::String);
            assert_eq!(lit.methods[0].return_type, Some(TsTypeInfo::Void));
        }
        _ => panic!("expected TypeLiteral, got {:?}", info),
    }
}

#[test]
fn type_literal_call_signature() {
    let info = convert_to_ts_type_info(&parse_type("{ (x: number): string }")).unwrap();
    match &info {
        TsTypeInfo::TypeLiteral(lit) => {
            assert_eq!(lit.call_signatures.len(), 1);
            assert_eq!(lit.call_signatures[0].params.len(), 1);
            assert_eq!(lit.call_signatures[0].params[0].name, "x");
            assert_eq!(lit.call_signatures[0].params[0].ty, TsTypeInfo::Number);
            assert_eq!(lit.call_signatures[0].return_type, Some(TsTypeInfo::String));
        }
        _ => panic!("expected TypeLiteral, got {:?}", info),
    }
}

#[test]
fn type_literal_index_signature() {
    let info = convert_to_ts_type_info(&parse_type("{ [key: string]: number }")).unwrap();
    match &info {
        TsTypeInfo::TypeLiteral(lit) => {
            assert_eq!(lit.index_signatures.len(), 1);
            assert_eq!(lit.index_signatures[0].param_name, "key");
            assert_eq!(lit.index_signatures[0].param_type, TsTypeInfo::String);
            assert_eq!(lit.index_signatures[0].value_type, TsTypeInfo::Number);
            assert!(!lit.index_signatures[0].readonly);
        }
        _ => panic!("expected TypeLiteral, got {:?}", info),
    }
}

#[test]
fn nullable_union() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("string | null")).unwrap(),
        TsTypeInfo::Union(vec![TsTypeInfo::String, TsTypeInfo::Null])
    );
    assert_eq!(
        convert_to_ts_type_info(&parse_type("number | undefined")).unwrap(),
        TsTypeInfo::Union(vec![TsTypeInfo::Number, TsTypeInfo::Undefined])
    );
}

#[test]
fn keyof_type() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("keyof Foo")).unwrap(),
        TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
            name: "Foo".to_string(),
            type_args: vec![],
        }))
    );
}

#[test]
fn typeof_query() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("typeof myVar")).unwrap(),
        TsTypeInfo::TypeQuery("myVar".to_string())
    );
}

#[test]
fn readonly_stripped() {
    assert_eq!(
        convert_to_ts_type_info(&parse_type("readonly string[]")).unwrap(),
        TsTypeInfo::Readonly(Box::new(TsTypeInfo::Array(Box::new(TsTypeInfo::String))))
    );
}

#[test]
fn conditional_type() {
    let info = convert_to_ts_type_info(&parse_type("T extends string ? number : boolean")).unwrap();
    match info {
        TsTypeInfo::Conditional {
            check,
            extends,
            true_type,
            false_type,
        } => {
            assert_eq!(
                *check,
                TsTypeInfo::TypeRef {
                    name: "T".to_string(),
                    type_args: vec![]
                }
            );
            assert_eq!(*extends, TsTypeInfo::String);
            assert_eq!(*true_type, TsTypeInfo::Number);
            assert_eq!(*false_type, TsTypeInfo::Boolean);
        }
        _ => panic!("expected Conditional"),
    }
}

#[test]
fn indexed_access_type() {
    let info = convert_to_ts_type_info(&parse_type("Foo[\"bar\"]")).unwrap();
    match info {
        TsTypeInfo::IndexedAccess { object, index } => {
            assert_eq!(
                *object,
                TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![]
                }
            );
            assert_eq!(
                *index,
                TsTypeInfo::Literal(TsLiteralKind::String("bar".to_string()))
            );
        }
        _ => panic!("expected IndexedAccess"),
    }
}

#[test]
fn type_predicate() {
    // `x is string` is a type predicate
    let full = "function isString(x: any): x is string {}";
    let module = crate::parser::parse_typescript(full).expect("parse failed");
    let fn_decl = match &module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(f))) => f,
        _ => panic!("expected fn decl"),
    };
    let ret_type = fn_decl.function.return_type.as_ref().expect("return type");
    let info = convert_to_ts_type_info(&ret_type.type_ann).unwrap();
    assert_eq!(info, TsTypeInfo::TypePredicate);
}

#[test]
fn constructor_type() {
    let info = convert_to_ts_type_info(&parse_type("new (config: string) => Service")).unwrap();
    assert_eq!(
        info,
        TsTypeInfo::Function {
            params: vec![TsParamInfo {
                name: "config".to_string(),
                ty: TsTypeInfo::String,
                optional: false,
            }],
            return_type: Box::new(TsTypeInfo::TypeRef {
                name: "Service".to_string(),
                type_args: vec![],
            }),
        }
    );
}

#[test]
fn mapped_type() {
    let info = convert_to_ts_type_info(&parse_type("{ [K in keyof T]: T[K] }")).unwrap();
    match info {
        TsTypeInfo::Mapped {
            type_param,
            constraint,
            value,
            ..
        } => {
            assert_eq!(type_param, "K");
            assert_eq!(
                *constraint,
                TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
                    name: "T".to_string(),
                    type_args: vec![],
                }))
            );
            assert!(value.is_some());
        }
        _ => panic!("expected Mapped, got {:?}", info),
    }
}
