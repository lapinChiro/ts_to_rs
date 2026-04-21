//! `type X = ...` alias resolution tests.
//!
//! Covers all `resolve_*_for_registry` paths (Pass C / intersection /
//! TypeLiteral) via `type X = ...` declarations. Intentionally scoped to
//! `type_alias_*` prefixed tests so that each new intersection / utility /
//! mapped / edge-case variant adds its test here without cross-concern
//! growth.
//!
//! Sub-groups inside this file (kept together because they all exercise
//! the same resolution pipeline with varying input shapes):
//! - **Simple ref / utility / pick**: pass-C TypeRef → direct field copy
//! - **Intersection variants**: 2-member / unresolvable / unregistered +
//!   literal / 3-member / optional / generic / enum-ref / method-bearing
//! - **Signature-bearing aliases**: call / construct / method signatures
//!   in a `type X = { ... }` literal
//! - **Edge placeholder cases**: index-sig-only / resolve-failure /
//!   mapped-type → pass-1 placeholder preservation

use super::*;

// ── G5: パス C (TypeRef) ──

#[test]
fn test_type_alias_type_ref_resolves_fields() {
    let module = parse_typescript(
        "interface Body { text: string; json: boolean }
         type BodyCache = Body;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("BodyCache")
        .expect("BodyCache should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 2);
        assert!(fields
            .iter()
            .any(|f| f.name == "text" && f.ty == RustType::String));
        assert!(fields
            .iter()
            .any(|f| f.name == "json" && f.ty == RustType::Bool));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_type_ref_with_utility_type() {
    let module = parse_typescript(
        "interface Body { text: string; json: boolean }
         type BodyCache = Partial<Body>;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("BodyCache")
        .expect("BodyCache should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(
            fields.len(),
            2,
            "Partial<Body> should have 2 Optional fields, got: {fields:?}"
        );
        assert!(
            fields.iter().any(|f| f.name == "text"
                && matches!(&f.ty, RustType::Option(inner) if **inner == RustType::String)),
            "text field should be Option<String>, got: {fields:?}"
        );
        assert!(
            fields.iter().any(|f| f.name == "json"
                && matches!(&f.ty, RustType::Option(inner) if **inner == RustType::Bool)),
            "json field should be Option<Bool>, got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_type_ref_with_pick() {
    let module = parse_typescript(
        "interface Body { text: string; json: boolean }
         type TextOnly = Pick<Body, 'text'>;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("TextOnly").expect("TextOnly should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 1, "Pick should have 1 field, got: {fields:?}");
        assert_eq!(fields[0].name, "text");
        assert_eq!(fields[0].ty, RustType::String);
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_simple_ref_copies_methods() {
    let module = parse_typescript(
        "interface Greeter { name: string; greet(msg: string): void; }
         type MyGreeter = Greeter;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("MyGreeter")
        .expect("MyGreeter should be registered");
    if let TypeDef::Struct {
        fields, methods, ..
    } = def
    {
        assert_eq!(fields.len(), 1, "should copy 1 field, got: {fields:?}");
        assert_eq!(fields[0].name, "name");
        assert!(
            methods.contains_key("greet"),
            "should copy 'greet' method from Greeter"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G10: intersection TsTypeLit & TsTypeLit ──

#[test]
fn test_type_alias_intersection_two_type_lits() {
    let module = parse_typescript("type Both = { x: number } & { y: string };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Both").expect("Both should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 2);
        assert!(fields
            .iter()
            .any(|f| f.name == "x" && f.ty == RustType::F64));
        assert!(fields
            .iter()
            .any(|f| f.name == "y" && f.ty == RustType::String));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G11: intersection で未宣言 TypeRef は _N field として埋め込み ──

#[test]
fn test_type_alias_intersection_unresolvable_refs_embedded_as_fields() {
    // Foo, Bar は未宣言 → registry に未登録 → resolve_ts_type で Named として解決
    // → _N field として埋め込み（暗黙無視ではなく明示的に参照を保持）
    let module = parse_typescript("type X = Foo & Bar;").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(
            fields.len(),
            2,
            "unresolvable TypeRefs should be embedded as _N fields, got: {fields:?}"
        );
        assert!(fields.iter().any(|f| f.name == "_0"));
        assert!(fields.iter().any(|f| f.name == "_1"));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G12: intersection で未宣言 TypeRef + TypeLiteral ──

#[test]
fn test_type_alias_intersection_unregistered_ref_with_literal() {
    // UnknownType は _0 field として埋め込み、TypeLiteral の y は通常フィールド
    let module = parse_typescript("type X = UnknownType & { y: string };").unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("X")
        .expect("X should be registered with merged fields");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(
            fields.len(),
            2,
            "should have _0 (UnknownType) + y, got: {fields:?}"
        );
        assert!(
            fields.iter().any(|f| f.name == "_0"),
            "UnknownType should be _0 field"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.name == "y" && f.ty == RustType::String),
            "y should be String"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: 3+ members ──

#[test]
fn test_type_alias_intersection_three_members() {
    let module = parse_typescript(
        "interface A { x: number; }
         interface B { y: string; }
         type X = A & B & { z: boolean };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 3, "expected 3 merged fields, got {fields:?}");
        assert!(fields
            .iter()
            .any(|f| f.name == "x" && f.ty == RustType::F64));
        assert!(fields
            .iter()
            .any(|f| f.name == "y" && f.ty == RustType::String));
        assert!(fields
            .iter()
            .any(|f| f.name == "z" && f.ty == RustType::Bool));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: TypeLiteral optional field ──

#[test]
fn test_type_alias_intersection_type_literal_optional_field() {
    let module = parse_typescript(
        "interface Base { id: number; }
         type X = Base & { opt?: string; };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        // Base から来る id フィールドの検証
        assert!(
            fields
                .iter()
                .any(|f| f.name == "id" && f.ty == RustType::F64),
            "id field should be merged from Base, got: {fields:?}"
        );
        // optional フィールドの検証
        let opt_field = fields.iter().find(|f| f.name == "opt").expect("opt field");
        assert!(
            opt_field.optional,
            "opt field should have optional=true, got: {:?}",
            opt_field
        );
        assert!(
            matches!(&opt_field.ty, RustType::Option(inner) if **inner == RustType::String),
            "opt field type should be Option<String>, got: {:?}",
            opt_field.ty
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: TypeRef with type args (instantiation) ──

#[test]
fn test_type_alias_intersection_generic_type_ref_instantiated() {
    let module = parse_typescript(
        "interface Container<T> { value: T; }
         type X = Container<string> & { extra: number; };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 2, "expected 2 fields, got {fields:?}");
        // Container<string> → value: String (instantiated, not TypeVar)
        let value_field = fields
            .iter()
            .find(|f| f.name == "value")
            .expect("value field");
        assert_eq!(
            value_field.ty,
            RustType::String,
            "value should be String (instantiated), got: {:?}",
            value_field.ty
        );
        assert!(
            fields
                .iter()
                .any(|f| f.name == "extra" && f.ty == RustType::F64),
            "extra field should be F64, got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: TypeRef to non-Struct (Enum) ──

#[test]
fn test_type_alias_intersection_enum_ref_embedded_as_field() {
    let module = parse_typescript(
        "type Status = 'active' | 'inactive';
         type X = { id: number } & Status;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        // id フィールドは TypeLiteral から
        assert!(
            fields
                .iter()
                .any(|f| f.name == "id" && f.ty == RustType::F64),
            "should have id field, got: {fields:?}"
        );
        // Status (Enum) は Struct ではないため _N field として埋め込まれる
        assert!(
            fields.iter().any(|f| f.name.starts_with('_')),
            "Enum TypeRef should be embedded as _N field, got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: TypeLiteral member with method ──

#[test]
fn test_type_alias_intersection_type_literal_with_method() {
    let module = parse_typescript(
        "interface Named { name: string; }
         type X = Named & { greet(): void; };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct {
        fields, methods, ..
    } = def
    {
        assert!(
            fields.iter().any(|f| f.name == "name"),
            "should have 'name' field from Named"
        );
        assert!(
            methods.contains_key("greet"),
            "should have 'greet' method from type literal"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── type alias with method/call/construct signatures ──

#[test]
fn test_type_alias_with_method_signature() {
    let module = parse_typescript("type Handler = { handle(x: string): void; }").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Handler").expect("Handler should be registered");
    if let TypeDef::Struct { methods, .. } = def {
        let sigs = methods.get("handle").expect("handle method should exist");
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].params.len(), 1);
        assert_eq!(sigs[0].params[0].name, "x");
        assert_eq!(sigs[0].params[0].ty, RustType::String);
        assert_eq!(sigs[0].return_type, Some(RustType::Unit));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_with_call_signature_and_field() {
    // call-sig-only は try_collect_fn_type_alias で Function になる。
    // call sig + field の mixed case がパス A で Struct になることを検証。
    let module =
        parse_typescript("type Callable = { (input: string): number; name: string; }").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Callable").expect("Callable should be registered");
    if let TypeDef::Struct {
        call_signatures,
        fields,
        ..
    } = def
    {
        assert_eq!(call_signatures.len(), 1, "should have 1 call signature");
        assert_eq!(call_signatures[0].params.len(), 1);
        assert_eq!(call_signatures[0].params[0].name, "input");
        assert_eq!(call_signatures[0].return_type, Some(RustType::F64));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "name");
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_with_construct_signature() {
    let module = parse_typescript("type Factory = { new(config: string): Factory; }").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Factory").expect("Factory should be registered");
    if let TypeDef::Struct { constructor, .. } = def {
        let ctors = constructor.as_ref().expect("constructor should be present");
        assert_eq!(ctors.len(), 1);
        assert_eq!(ctors[0].params.len(), 1);
        assert_eq!(ctors[0].params[0].name, "config");
        assert_eq!(ctors[0].params[0].ty, RustType::String);
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── テストカバレッジ補完 (G1-G6) ──

/// G1: Index signature のみの type alias → pass-1 placeholder のまま（TypeDef::Alias 別 PRD）
#[test]
fn test_type_alias_index_sig_only_stays_placeholder() {
    let module = parse_typescript("type Dict = { [key: string]: number; }").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Dict").expect("pass-1 should register placeholder");
    if let TypeDef::Struct { fields, .. } = def {
        // index signature のみの TypeLiteral は空 fields の struct として登録される
        assert!(
            fields.is_empty(),
            "index-sig-only type alias should have empty fields (TypeDef::Alias 別 PRD)"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

/// G4: generic intersection — TypeVar 保持確認
#[test]
fn test_type_alias_generic_intersection_preserves_type_var() {
    let module = parse_typescript(
        "interface Base { id: number; }
         type WithBase<T> = Base & { extra: T; };",
    )
    .unwrap();
    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let reg = build_registry_with_synthetic(&module, &mut synthetic);
    let def = reg.get("WithBase").expect("WithBase should be registered");
    if let TypeDef::Struct {
        fields,
        type_params,
        ..
    } = def
    {
        // T は TypeVar として残る（monomorphize されない）
        let extra_field = fields
            .iter()
            .find(|f| f.name == "extra")
            .expect("extra field");
        assert!(
            matches!(&extra_field.ty, RustType::TypeVar { name } if name == "T"),
            "T should be TypeVar in registry, got: {:?}",
            extra_field.ty
        );
        assert!(type_params.iter().any(|tp| tp.name == "T"));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

/// G5: resolve 失敗時、TypeDef 全体が登録されない（pass-1 placeholder のまま）
///
/// `type X = { x: typeof UnknownVar }` のように resolve_ts_type が失敗するフィールド型を含む場合、
/// resolve_struct_for_registry が Err を返し、Pass 1 placeholder（空 struct）が残る。
#[test]
fn test_type_alias_resolve_failure_stays_placeholder() {
    // `typeof UnknownVar` は registry に UnknownVar がないためエラー
    let module = parse_typescript("type X = { value: typeof UnknownVar; };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("pass-1 should register placeholder");
    if let TypeDef::Struct { fields, .. } = def {
        assert!(
            fields.is_empty(),
            "resolve failure should keep pass-1 placeholder (empty fields), got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

/// G6: mapped type alias → resolve_ts_type で HashMap に解決され、
/// パス C で Named 以外のため登録スキップ
#[test]
fn test_type_alias_mapped_type_stays_placeholder() {
    let module = parse_typescript("type Mapped<T> = { [K in keyof T]: string };").unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("Mapped")
        .expect("pass-1 should register placeholder");
    if let TypeDef::Struct { fields, .. } = def {
        // mapped type は HashMap に resolve されるが TypeDef::Struct として登録できない
        // → pass-1 placeholder (空 fields) のまま
        assert!(
            fields.is_empty(),
            "mapped type alias should have empty fields (HashMap cannot be TypeDef::Struct)"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}
