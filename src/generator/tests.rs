use super::*;
use crate::ir::{
    BinOp, EnumValue, EnumVariant, Expr, Item, Method, Param, RustType, Stmt, StructField,
    TraitRef, TypeParam, Visibility,
};

// --- Item::Use tests ---

#[test]
fn test_generate_use_single() {
    let item = Item::Use {
        vis: Visibility::Private,
        path: "crate::bar".to_string(),
        names: vec!["Foo".to_string()],
    };
    assert_eq!(generate(&[item]), "use crate::bar::Foo;");
}

#[test]
fn test_generate_use_multiple() {
    let item = Item::Use {
        vis: Visibility::Private,
        path: "crate::bar".to_string(),
        names: vec!["A".to_string(), "B".to_string()],
    };
    assert_eq!(generate(&[item]), "use crate::bar::{A, B};");
}

#[test]
fn test_generate_pub_use_single() {
    let item = Item::Use {
        vis: Visibility::Public,
        path: "crate::bar".to_string(),
        names: vec!["Foo".to_string()],
    };
    assert_eq!(generate(&[item]), "pub use crate::bar::Foo;");
}

#[test]
fn test_generate_pub_use_multiple() {
    let item = Item::Use {
        vis: Visibility::Public,
        path: "crate::baz".to_string(),
        names: vec!["A".to_string(), "B".to_string()],
    };
    assert_eq!(generate(&[item]), "pub use crate::baz::{A, B};");
}

// --- Item::Struct tests ---

#[test]
fn test_generate_struct_public() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        fields: vec![
            StructField {
                vis: None,
                name: "name".to_string(),
                ty: RustType::String,
            },
            StructField {
                vis: None,
                name: "age".to_string(),
                ty: RustType::F64,
            },
        ],
        is_unit_struct: false,
    };
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
pub struct Foo {
    pub name: String,
    pub age: f64,
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_struct_private() {
    let item = Item::Struct {
        vis: Visibility::Private,
        name: "Bar".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: None,
            name: "x".to_string(),
            ty: RustType::Bool,
        }],
        is_unit_struct: false,
    };
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
struct Bar {
    x: bool,
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_struct_with_type_params() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Container".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        fields: vec![StructField {
            vis: None,
            name: "value".to_string(),
            ty: RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            },
        }],
        is_unit_struct: false,
    };
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
pub struct Container<T> {
    pub value: T,
}";
    assert_eq!(generate(&[item]), expected);
}

// --- Item::Struct unit struct (is_unit_struct) tests ---

#[test]
fn test_generate_unit_struct_marker() {
    let item = Item::Struct {
        vis: Visibility::Private,
        name: "GetCookieImpl".to_string(),
        type_params: vec![],
        fields: vec![],
        is_unit_struct: true,
    };
    assert_eq!(
        generate(&[item]),
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\nstruct GetCookieImpl;"
    );
}

#[test]
fn test_generate_non_unit_empty_struct_keeps_braces() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Empty".to_string(),
        type_params: vec![],
        fields: vec![],
        is_unit_struct: false,
    };
    let output = generate(&[item]);
    assert!(
        output.contains("struct Empty {"),
        "non-unit empty struct should use braces: {output}"
    );
}

// --- Item::Enum tests ---

#[test]
fn test_generate_enum_numeric_auto() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Color".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Red".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Green".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Blue".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
        ],
    };
    let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Color {
    Red = 0,
    Green = 1,
    Blue = 2,
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", *self as i64)
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_enum_numeric_explicit() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Status".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Active".to_string(),
                value: Some(EnumValue::Number(1)),
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Inactive".to_string(),
                value: Some(EnumValue::Number(0)),
                data: None,
                fields: vec![],
            },
        ],
    };
    let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Status {
    Active = 1,
    Inactive = 0,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", *self as i64)
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_enum_string() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Direction".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Up".to_string(),
                value: Some(EnumValue::Str("UP".to_string())),
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Down".to_string(),
                value: Some(EnumValue::Str("DOWN".to_string())),
                data: None,
                fields: vec![],
            },
        ],
    };
    let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn as_str(&self) -> &str {
        match self {
            Direction::Up => \"UP\",
            Direction::Down => \"DOWN\",
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", self.as_str())
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_enum_private() {
    let item = Item::Enum {
        vis: Visibility::Private,
        name: "Color".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "Red".to_string(),
            value: None,
            data: None,
            fields: vec![],
        }],
    };
    let result = generate(&[item]);
    assert!(!result.contains("pub enum"));
    assert!(result.contains("enum Color"));
}

#[test]
fn test_generate_enum_data_variants() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Value".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "String".to_string(),
                value: None,
                data: Some(RustType::String),
                fields: vec![],
            },
            EnumVariant {
                name: "F64".to_string(),
                value: None,
                data: Some(RustType::F64),
                fields: vec![],
            },
            EnumVariant {
                name: "Bool".to_string(),
                value: None,
                data: Some(RustType::Bool),
                fields: vec![],
            },
        ],
    };
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    F64(f64),
    Bool(bool),
}";
    assert_eq!(generate(&[item]), expected);
}

// --- Item::Const tests ---

#[test]
fn test_generate_const_private_number() {
    let item = Item::Const {
        vis: Visibility::Private,
        name: "MY_VAL".to_string(),
        ty: RustType::F64,
        value: Expr::NumberLit(42.0),
    };
    assert_eq!(generate(&[item]), "const MY_VAL: f64 = 42.0;");
}

#[test]
fn test_generate_const_public_string() {
    let item = Item::Const {
        vis: Visibility::Public,
        name: "GREETING".to_string(),
        ty: RustType::String,
        value: Expr::StringLit("hello".to_string()),
    };
    assert_eq!(generate(&[item]), "pub const GREETING: String = \"hello\";");
}

#[test]
fn test_generate_const_unit_struct_init() {
    let item = Item::Const {
        vis: Visibility::Private,
        name: "getCookie".to_string(),
        ty: RustType::Named {
            name: "GetCookieImpl".to_string(),
            type_args: vec![],
        },
        value: Expr::StructInit {
            name: "GetCookieImpl".to_string(),
            fields: vec![],
            base: None,
        },
    };
    assert_eq!(
        generate(&[item]),
        "const getCookie: GetCookieImpl = GetCookieImpl;"
    );
}

// --- Item::Fn tests ---

#[test]
fn test_generate_fn_simple_return() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "add".to_string(),
        type_params: vec![],
        params: vec![
            Param {
                name: "a".to_string(),
                ty: Some(RustType::F64),
            },
            Param {
                name: "b".to_string(),
                ty: Some(RustType::F64),
            },
        ],
        return_type: Some(RustType::F64),
        body: vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::Ident("b".to_string())),
        })],
    };
    let expected = "\
pub fn add(a: f64, b: f64) -> f64 {
    a + b
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_fn_no_return_type() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "greet".to_string(),
        type_params: vec![],
        params: vec![Param {
            name: "name".to_string(),
            ty: Some(RustType::String),
        }],
        return_type: None,
        body: vec![Stmt::Expr(Expr::Ident("println!".to_string()))],
    };
    let expected = "\
fn greet(name: String) {
    println!;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_fn_no_params() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "get_value".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: Some(RustType::F64),
        body: vec![Stmt::TailExpr(Expr::NumberLit(42.0))],
    };
    let expected = "\
pub fn get_value() -> f64 {
    42.0
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_fn_with_type_params() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "identity".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
        }],
        return_type: Some(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }),
        body: vec![Stmt::TailExpr(Expr::Ident("x".to_string()))],
    };
    let expected = "\
pub fn identity<T>(x: T) -> T {
    x
}";
    assert_eq!(generate(&[item]), expected);
}

// --- Item::Impl tests ---

#[test]
fn test_generate_impl_new() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "new".to_string(),
            is_async: false,
            has_self: false,
            has_mut_self: false,
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::F64),
            }],
            return_type: Some(RustType::Named {
                name: "Self".to_string(),
                type_args: vec![],
            }),
            body: Some(vec![Stmt::TailExpr(Expr::Ident("Self { x }".to_string()))]),
        }],
    };
    let expected = "\
impl Foo {
    pub fn new(x: f64) -> Self {
        Self { x }
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_impl_self_method() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "get_name".to_string(),
            is_async: false,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::String),
            body: Some(vec![Stmt::TailExpr(Expr::FieldAccess {
                object: Box::new(Expr::Ident("self".to_string())),
                field: "name".to_string(),
            })]),
        }],
    };
    let expected = "\
impl Foo {
    pub fn get_name(&self) -> String {
        self.name
    }
}";
    assert_eq!(generate(&[item]), expected);
}

// --- Method::is_async tests ---

#[test]
fn test_generate_async_trait_method_sig() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Handler".to_string(),
        type_params: vec![],
        supertraits: vec![],
        methods: vec![Method {
            vis: Visibility::Private,
            name: "handle".to_string(),
            is_async: true,
            has_self: true,
            has_mut_self: false,
            params: vec![Param {
                name: "req".to_string(),
                ty: Some(RustType::String),
            }],
            return_type: Some(RustType::String),
            body: None,
        }],
        associated_types: vec![],
    };
    let output = generate(&[item]);
    assert!(
        output.contains("async fn handle"),
        "trait method should have async keyword: {output}"
    );
}

#[test]
fn test_generate_async_impl_method() {
    let item = Item::Impl {
        struct_name: "MyHandler".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "process".to_string(),
            is_async: true,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::String),
            body: Some(vec![Stmt::TailExpr(Expr::StringLit("done".to_string()))]),
        }],
    };
    let output = generate(&[item]);
    assert!(
        output.contains("pub async fn process"),
        "impl method should have async keyword: {output}"
    );
}

// --- Multiple items ---

#[test]
fn test_generate_multiple_items_separated_by_blank_line() {
    let items = vec![
        Item::Struct {
            vis: Visibility::Public,
            name: "A".to_string(),
            type_params: vec![],
            fields: vec![],
            is_unit_struct: false,
        },
        Item::Struct {
            vis: Visibility::Public,
            name: "B".to_string(),
            type_params: vec![],
            fields: vec![],
            is_unit_struct: false,
        },
    ];
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
pub struct A {
}

#[derive(Debug, Clone, PartialEq)]
pub struct B {
}";
    assert_eq!(generate(&items), expected);
}

// --- Item::Trait tests ---

#[test]
fn test_generate_trait() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "AnimalTrait".to_string(),
        type_params: vec![],
        methods: vec![Method {
            vis: Visibility::Private,
            name: "speak".to_string(),
            is_async: false,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::String),
            body: None,
        }],
        supertraits: vec![],
        associated_types: vec![],
    };
    let expected = "\
pub trait AnimalTrait {
    fn speak(&self) -> String;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_trait_with_supertraits_outputs_bounds() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Dog".to_string(),
        type_params: vec![],
        supertraits: vec![
            TraitRef {
                name: "Animal".to_string(),
                type_args: vec![],
            },
            TraitRef {
                name: "Debug".to_string(),
                type_args: vec![],
            },
        ],
        methods: vec![Method {
            vis: Visibility::Private,
            name: "bark".to_string(),
            is_async: false,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: None,
            body: None,
        }],
        associated_types: vec![],
    };
    let expected = "\
pub trait Dog: Animal + Debug {
    fn bark(&self);
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_impl_for_trait() {
    let item = Item::Impl {
        struct_name: "Dog".to_string(),
        type_params: vec![],
        for_trait: Some(TraitRef {
            name: "AnimalTrait".to_string(),
            type_args: vec![],
        }),
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Private,
            name: "speak".to_string(),
            is_async: false,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::String),
            body: Some(vec![Stmt::TailExpr(Expr::FieldAccess {
                object: Box::new(Expr::Ident("self".to_string())),
                field: "name".to_string(),
            })]),
        }],
    };
    let expected = "\
impl AnimalTrait for Dog {
    fn speak(&self) -> String {
        self.name
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_escape_ident_fn_name_reserved_word_adds_r_hash() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "match".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
    };
    let output = generate(&[item]);
    assert!(
        output.contains("fn r#match()"),
        "expected r#match in: {output}"
    );
}

#[test]
fn test_generate_fn_with_attributes_outputs_attr_lines() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec!["tokio::main".to_string()],
        is_async: true,
        name: "main".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
    };
    let expected = "\
#[tokio::main]
async fn main() {
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_fn_without_attributes_no_attr_lines() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: true,
        name: "not_main".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
    };
    let output = generate(&[item]);
    assert!(
        !output.contains("#["),
        "expected no attributes in: {output}"
    );
}

#[test]
fn test_escape_ident_struct_field_reserved_word_adds_r_hash() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "type".to_string(),
            ty: RustType::String,
        }],
        is_unit_struct: false,
    };
    let output = generate(&[item]);
    assert!(
        output.contains("r#type: String"),
        "expected r#type in: {output}"
    );
}

// --- Expr::Regex tests ---

#[test]
fn test_generate_regex_backslash_pattern_uses_raw_string() {
    let expr = Expr::Regex {
        pattern: r"\d+".to_string(),
        global: false,
        sticky: false,
    };
    let output = generate_expr(&expr);
    assert_eq!(output, r#"Regex::new(r"\d+").unwrap()"#);
}

#[test]
fn test_generate_regex_quote_pattern_uses_raw_hash_string() {
    let expr = Expr::Regex {
        pattern: r#"a"b"#.to_string(),
        global: false,
        sticky: false,
    };
    let output = generate_expr(&expr);
    assert_eq!(output, r###"Regex::new(r#"a"b"#).unwrap()"###);
}

#[test]
fn test_generate_regex_simple_pattern_uses_raw_string() {
    let expr = Expr::Regex {
        pattern: "pattern".to_string(),
        global: false,
        sticky: false,
    };
    let output = generate_expr(&expr);
    assert_eq!(output, r#"Regex::new(r"pattern").unwrap()"#);
}

// --- I-218: Item::Impl type_params ---

#[test]
fn test_generate_impl_with_type_params() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        for_trait: None,
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl<T> Foo<T> {\n}");
}

#[test]
fn test_generate_impl_with_constraint() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(RustType::Named {
                name: "Clone".to_string(),
                type_args: vec![],
            }),
            default: None,
        }],
        for_trait: None,
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl<T: Clone> Foo<T> {\n}");
}

#[test]
fn test_generate_impl_for_trait_with_type_params() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        for_trait: Some(TraitRef {
            name: "Display".to_string(),
            type_args: vec![],
        }),
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl<T> Display for Foo<T> {\n}");
}

#[test]
fn test_generate_impl_for_trait_with_trait_type_args() {
    let item = Item::Impl {
        struct_name: "FooData".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        for_trait: Some(TraitRef {
            name: "Container".to_string(),
            type_args: vec![RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }],
        }),
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl<T> Container<T> for FooData<T> {\n}");
}

#[test]
fn test_generate_impl_for_trait_with_concrete_type_args() {
    let item = Item::Impl {
        struct_name: "Child".to_string(),
        type_params: vec![],
        for_trait: Some(TraitRef {
            name: "ParentTrait".to_string(),
            type_args: vec![RustType::String],
        }),
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl ParentTrait<String> for Child {\n}");
}

#[test]
fn test_generate_trait_with_supertrait_type_args() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        supertraits: vec![TraitRef {
            name: "Bar".to_string(),
            type_args: vec![RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }],
        }],
        methods: vec![],
        associated_types: vec![],
    };
    let output = generate_item(&item);
    assert!(
        output.starts_with("pub trait Foo<T>: Bar<T>"),
        "expected 'pub trait Foo<T>: Bar<T>', got: {output}"
    );
}
