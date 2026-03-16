//! Intermediate Representation (IR) for Rust code generation.
//!
//! The IR sits between the SWC TypeScript AST and Rust source code generation.
//! It models the subset of Rust constructs needed for Phase 1 of ts_to_rs.

/// Represents a Rust type.
#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
    /// `()` (unit type, corresponds to TypeScript `void`)
    Unit,
    /// `String`
    String,
    /// `f64`
    F64,
    /// `bool`
    Bool,
    /// `Option<T>`
    Option(Box<RustType>),
    /// `Vec<T>`
    Vec(Box<RustType>),
    /// A function type: `impl Fn(T1, T2) -> R`
    Fn {
        /// Parameter types
        params: Vec<RustType>,
        /// Return type
        return_type: Box<RustType>,
    },
    /// `Result<T, E>`
    Result {
        /// Ok type
        ok: Box<RustType>,
        /// Err type
        err: Box<RustType>,
    },
    /// A tuple type: `(T1, T2, ...)`
    Tuple(Vec<RustType>),
    /// `Box<dyn std::any::Any>` (corresponds to TypeScript `any` and `unknown`)
    Any,
    /// `!` (never type, corresponds to TypeScript `never`)
    Never,
    /// A user-defined named type, optionally with generic type arguments (e.g., `Point`, `Box<T>`)
    Named {
        /// Type name
        name: String,
        /// Generic type arguments (empty if not generic)
        type_args: Vec<RustType>,
    },
}

/// Visibility modifier for items.
#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    /// `pub`
    Public,
    /// `pub(crate)`
    PubCrate,
    /// No visibility modifier (private by default)
    Private,
}

/// A value associated with an enum variant.
#[derive(Debug, Clone, PartialEq)]
pub enum EnumValue {
    /// A numeric discriminant (e.g., `Active = 1`)
    Number(i64),
    /// A string value (e.g., `Up = "UP"`)
    Str(String),
    /// A computed expression (e.g., `Read = 1 << 0`)
    Expr(String),
}

/// A variant of an enum, with an optional value.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    /// Variant name
    pub name: String,
    /// Optional discriminant or string value
    pub value: Option<EnumValue>,
    /// Optional data type for tuple-like variants (e.g., `String(String)`, `F64(f64)`)
    pub data: Option<RustType>,
    /// Named fields for struct-like variants (discriminated unions)
    pub fields: Vec<StructField>,
}

/// A named field in a struct.
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    /// Field visibility (defaults to inheriting from the parent struct)
    pub vis: Option<Visibility>,
    /// Field name
    pub name: String,
    /// Field type
    pub ty: RustType,
}

/// A pattern in a match arm.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    /// A literal value pattern (e.g., `1`, `"hello"`)
    Literal(Expr),
    /// A wildcard pattern (`_`)
    Wildcard,
}

/// An arm in a `match` expression.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Patterns for this arm. Multiple patterns represent `a | b | _`.
    pub patterns: Vec<MatchPattern>,
    /// Arm body.
    pub body: Vec<Stmt>,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Parameter name
    pub name: String,
    /// Parameter type (`None` for closures where type inference applies)
    pub ty: Option<RustType>,
}

/// An associated constant inside an `impl` block (e.g., `pub const MAX: f64 = 100.0;`).
#[derive(Debug, Clone, PartialEq)]
pub struct AssocConst {
    /// Visibility
    pub vis: Visibility,
    /// Constant name
    pub name: String,
    /// Type
    pub ty: RustType,
    /// Value expression
    pub value: Expr,
}

/// A method inside an `impl` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    /// Visibility
    pub vis: Visibility,
    /// Method name
    pub name: String,
    /// Whether this method takes `&self` or `&mut self` (false for associated functions like `new`)
    pub has_self: bool,
    /// Whether this method takes `&mut self` instead of `&self` (e.g., setters)
    pub has_mut_self: bool,
    /// Parameters (excluding `self`)
    pub params: Vec<Param>,
    /// Return type (`None` means `()`)
    pub return_type: Option<RustType>,
    /// Method body (`None` for trait method signatures, `Some` for implementations)
    pub body: Option<Vec<Stmt>>,
}

/// Top-level item in a Rust file or module.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A comment block. Each line is prefixed with `// ` in the generated output.
    Comment(String),
    /// A `use` statement: `use path::{names};` or `pub use path::{names};`
    Use {
        /// Visibility (`Private` for `use`, `Public` for `pub use`)
        vis: Visibility,
        /// Module path (e.g., `crate::bar`)
        path: String,
        /// Imported names (e.g., `["Foo", "Bar"]`)
        names: Vec<String>,
    },
    /// A `struct` with named fields.
    Struct {
        /// Visibility
        vis: Visibility,
        /// Struct name
        name: String,
        /// Generic type parameters (e.g., `["T", "U"]`)
        type_params: Vec<String>,
        /// Named fields
        fields: Vec<StructField>,
    },
    /// An `enum` with variants that may have numeric or string values.
    Enum {
        /// Visibility
        vis: Visibility,
        /// Enum name
        name: String,
        /// Optional serde tag field name for discriminated unions (e.g., `"kind"`)
        serde_tag: Option<String>,
        /// Enum variants
        variants: Vec<EnumVariant>,
    },
    /// A `trait` definition.
    Trait {
        /// Visibility
        vis: Visibility,
        /// Trait name (e.g., `AnimalTrait`)
        name: String,
        /// Method signatures (body is empty — signatures only)
        methods: Vec<Method>,
    },
    /// An `impl` block for a struct, optionally implementing a trait.
    Impl {
        /// Struct name this impl is for
        struct_name: String,
        /// If `Some`, this is a trait impl: `impl TraitName for StructName`
        for_trait: Option<String>,
        /// Associated constants (e.g., `pub const MAX: f64 = 100.0;`)
        consts: Vec<AssocConst>,
        /// Methods in the impl block
        methods: Vec<Method>,
    },
    /// A `type` alias: `type Foo = Bar;`
    TypeAlias {
        /// Visibility
        vis: Visibility,
        /// Alias name
        name: String,
        /// Generic type parameters (e.g., `["T", "U"]`)
        type_params: Vec<String>,
        /// The aliased type
        ty: RustType,
    },
    /// A `fn` declaration.
    Fn {
        /// Visibility
        vis: Visibility,
        /// Whether this is an `async fn`
        is_async: bool,
        /// Function name
        name: String,
        /// Generic type parameters (e.g., `["T", "U"]`)
        type_params: Vec<String>,
        /// Parameters
        params: Vec<Param>,
        /// Return type (`None` means `()`)
        return_type: Option<RustType>,
        /// Function body
        body: Vec<Stmt>,
    },
}

/// A statement inside a function body.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let [mut] <name>[: <ty>] = <init>;`
    Let {
        /// Whether the binding is mutable
        mutable: bool,
        /// Variable name
        name: String,
        /// Optional explicit type annotation
        ty: Option<RustType>,
        /// Initializer expression
        init: Option<Expr>,
    },
    /// `if <condition> { ... } [else { ... }]`
    If {
        /// Condition expression
        condition: Expr,
        /// Then branch body
        then_body: Vec<Stmt>,
        /// Optional else branch body
        else_body: Option<Vec<Stmt>>,
    },
    /// `['label:] while <condition> { ... }`
    While {
        /// Optional loop label (e.g., `'outer`)
        label: Option<String>,
        /// Loop condition
        condition: Expr,
        /// Loop body
        body: Vec<Stmt>,
    },
    /// `['label:] for <var> in <iterable> { ... }`
    ForIn {
        /// Optional loop label (e.g., `'outer`)
        label: Option<String>,
        /// Loop variable name
        var: String,
        /// Iterable expression (e.g., a range or collection)
        iterable: Expr,
        /// Loop body
        body: Vec<Stmt>,
    },
    /// `['label:] loop { ... }`
    Loop {
        /// Optional loop label (e.g., `'outer`)
        label: Option<String>,
        /// Loop body
        body: Vec<Stmt>,
    },
    /// `break ['label] [value];`
    Break {
        /// Optional target label
        label: Option<String>,
        /// Optional value expression (e.g., `break 'try_block Err(...)`)
        value: Option<Expr>,
    },
    /// `continue ['label];`
    Continue {
        /// Optional target label
        label: Option<String>,
    },
    /// `return [<expr>];`
    Return(Option<Expr>),
    /// A bare expression statement (e.g., a function call).
    Expr(Expr),
    /// A tail expression (implicit return without `return` keyword).
    TailExpr(Expr),
    /// `if let <pattern> = <expr> { ... } [else { ... }]`
    IfLet {
        /// Pattern to match (e.g., `"Err(e)"`)
        pattern: String,
        /// Expression to match against
        expr: Expr,
        /// Then branch body
        then_body: Vec<Stmt>,
        /// Optional else branch body
        else_body: Option<Vec<Stmt>>,
    },
    /// `match <expr> { <arms> }`
    Match {
        /// Expression to match against
        expr: Expr,
        /// Match arms
        arms: Vec<MatchArm>,
    },
    /// A labeled block: `'label: { body... }`
    ///
    /// Used for try/catch expansion where the labeled block captures the result.
    LabeledBlock {
        /// Block label (e.g., `try_block`)
        label: String,
        /// Block body
        body: Vec<Stmt>,
    },
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A numeric literal: `3.14`
    NumberLit(f64),
    /// A boolean literal: `true` / `false`
    BoolLit(bool),
    /// A string literal: `"hello"`
    StringLit(String),
    /// An identifier: `foo`
    Ident(String),
    /// A `format!("<template>", <args...>)` macro call.
    FormatMacro {
        /// Format string template
        template: String,
        /// Format arguments
        args: Vec<Expr>,
    },
    /// A field access: `object.field`
    FieldAccess {
        /// The object expression (e.g., `self`)
        object: Box<Expr>,
        /// The field name
        field: String,
    },
    /// A method call: `expr.method(args)`
    MethodCall {
        /// The receiver expression
        object: Box<Expr>,
        /// Method name
        method: String,
        /// Arguments
        args: Vec<Expr>,
    },
    /// A struct initializer: `Self { field1: val1, field2: val2 }` or with struct update
    /// syntax `Self { field1: val1, ..base }`.
    StructInit {
        /// Struct name (e.g., `Self`)
        name: String,
        /// Field name-value pairs
        fields: Vec<(String, Expr)>,
        /// Optional base expression for struct update syntax (`..base`)
        base: Option<Box<Expr>>,
    },
    /// An assignment: `<target> = <value>`
    Assign {
        /// Assignment target
        target: Box<Expr>,
        /// Assigned value
        value: Box<Expr>,
    },
    /// A unary operation: `<op><operand>` (e.g., `!x`, `-x`)
    UnaryOp {
        /// Operator
        op: UnOp,
        /// Operand
        operand: Box<Expr>,
    },
    /// A binary operation: `<left> <op> <right>`
    BinaryOp {
        /// Left operand
        left: Box<Expr>,
        /// Operator
        op: BinOp,
        /// Right operand
        right: Box<Expr>,
    },
    /// A range expression: `start..end` or `start..` (open-ended)
    Range {
        /// Start of range (inclusive)
        start: Option<Box<Expr>>,
        /// End of range (exclusive, `None` for open-ended `start..`)
        end: Option<Box<Expr>>,
    },
    /// A function call: `name(args)` (e.g., `Ok(x)`, `Err("msg".to_string())`)
    FnCall {
        /// Function name
        name: String,
        /// Arguments
        args: Vec<Expr>,
    },
    /// A closure: `|params| body` or `|params| { body }`
    Closure {
        /// Closure parameters
        params: Vec<Param>,
        /// Optional return type annotation
        return_type: Option<RustType>,
        /// Closure body
        body: ClosureBody,
    },
    /// A vec macro: `vec![a, b, c]`
    Vec {
        /// Elements of the vec
        elements: Vec<Expr>,
    },
    /// An `if` expression: `if cond { then } else { else }`
    If {
        /// Condition expression
        condition: Box<Expr>,
        /// Then branch expression
        then_expr: Box<Expr>,
        /// Else branch expression
        else_expr: Box<Expr>,
    },
    /// A macro call: `name!(args)` (e.g., `println!("{:?}", x)`)
    MacroCall {
        /// Macro name (without `!`)
        name: String,
        /// Arguments
        args: Vec<Expr>,
    },
    /// An await expression: `expr.await`
    Await(Box<Expr>),
    /// A dereference expression: `*expr`
    Deref(Box<Expr>),
    /// A reference expression: `&expr`
    Ref(Box<Expr>),
    /// The unit value: `()`
    Unit,
    /// An integer literal: `42`
    IntLit(i64),
    /// An index access expression: `object[index]`
    Index {
        /// The object expression (e.g., `arr`)
        object: Box<Expr>,
        /// The index expression (e.g., `0`)
        index: Box<Expr>,
    },
    /// A type cast expression: `expr as target_type`
    Cast {
        /// The expression being cast
        expr: Box<Expr>,
        /// The target type
        target: RustType,
    },
    /// A block expression: `{ stmt1; stmt2; tail_expr }`
    Block(Vec<Stmt>),
}

/// Binary operators supported in the IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Mod,
    /// `==`
    Eq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `&&`
    LogicalAnd,
    /// `||`
    LogicalOr,
}

impl BinOp {
    /// Returns the Rust source representation of this operator.
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::NotEq => "!=",
            BinOp::Lt => "<",
            BinOp::LtEq => "<=",
            BinOp::Gt => ">",
            BinOp::GtEq => ">=",
            BinOp::LogicalAnd => "&&",
            BinOp::LogicalOr => "||",
        }
    }

    /// Returns the precedence level (higher = binds tighter).
    ///
    /// Based on Rust operator precedence:
    /// <https://doc.rust-lang.org/reference/expressions.html#expression-precedence>
    pub fn precedence(self) -> u8 {
        match self {
            BinOp::LogicalOr => 3,
            BinOp::LogicalAnd => 4,
            BinOp::Eq | BinOp::NotEq => 5,
            BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => 6,
            BinOp::Add | BinOp::Sub => 8,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 9,
        }
    }
}

/// Unary operators supported in the IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `!` (logical NOT)
    Not,
    /// `-` (negation)
    Neg,
}

impl UnOp {
    /// Returns the Rust source representation of this operator.
    pub fn as_str(self) -> &'static str {
        match self {
            UnOp::Not => "!",
            UnOp::Neg => "-",
        }
    }
}

/// The body of a closure expression.
#[derive(Debug, Clone, PartialEq)]
pub enum ClosureBody {
    /// A single expression: `|x| x + 1`
    Expr(Box<Expr>),
    /// A block body: `|x| { let y = x + 1; y }`
    Block(Vec<Stmt>),
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_item_struct() {
        let item = Item::Struct {
            vis: Visibility::Public,
            name: "Point".to_string(),
            type_params: vec![],
            fields: vec![
                StructField {
                    vis: None,
                    name: "x".to_string(),
                    ty: RustType::F64,
                },
                StructField {
                    vis: None,
                    name: "y".to_string(),
                    ty: RustType::Option(Box::new(RustType::F64)),
                },
            ],
        };
        match item {
            Item::Struct { name, fields, .. } => {
                assert_eq!(name, "Point");
                assert_eq!(fields.len(), 2);
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_item_enum_no_values() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Color".to_string(),
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
            ],
        };
        match item {
            Item::Enum { name, variants, .. } => {
                assert_eq!(name, "Color");
                assert_eq!(variants.len(), 2);
                assert!(variants[0].value.is_none());
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_item_enum_numeric_values() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Status".to_string(),
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
        match &item {
            Item::Enum { variants, .. } => {
                assert_eq!(variants[0].value, Some(EnumValue::Number(1)));
                assert_eq!(variants[1].value, Some(EnumValue::Number(0)));
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_item_enum_string_values() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Direction".to_string(),
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
        match &item {
            Item::Enum { variants, .. } => {
                assert_eq!(variants[0].value, Some(EnumValue::Str("UP".to_string())));
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_item_fn() {
        let item = Item::Fn {
            vis: Visibility::Public,
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
            body: vec![],
        };
        match item {
            Item::Fn { name, params, .. } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
            }
            _ => panic!("expected Fn"),
        }
    }

    #[test]
    fn test_stmt_let() {
        let stmt = Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: None,
            init: Some(Expr::NumberLit(42.0)),
        };
        match stmt {
            Stmt::Let { name, mutable, .. } => {
                assert_eq!(name, "x");
                assert!(!mutable);
            }
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn test_stmt_let_mut() {
        let stmt = Stmt::Let {
            mutable: true,
            name: "count".to_string(),
            ty: Some(RustType::F64),
            init: Some(Expr::NumberLit(0.0)),
        };
        match stmt {
            Stmt::Let { mutable, .. } => assert!(mutable),
            _ => panic!("expected Let"),
        }
    }

    #[test]
    fn test_stmt_if_else() {
        let stmt = Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![],
            else_body: Some(vec![]),
        };
        match stmt {
            Stmt::If { else_body, .. } => assert!(else_body.is_some()),
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn test_stmt_if_no_else() {
        let stmt = Stmt::If {
            condition: Expr::BoolLit(false),
            then_body: vec![],
            else_body: None,
        };
        match stmt {
            Stmt::If { else_body, .. } => assert!(else_body.is_none()),
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn test_expr_literals() {
        let _n = Expr::NumberLit(2.71);
        let _b = Expr::BoolLit(true);
        let _s = Expr::StringLit("hello".to_string());
    }

    #[test]
    fn test_expr_ident() {
        let e = Expr::Ident("foo".to_string());
        match e {
            Expr::Ident(name) => assert_eq!(name, "foo"),
            _ => panic!("expected Ident"),
        }
    }

    #[test]
    fn test_expr_format_macro() {
        let e = Expr::FormatMacro {
            template: "Hello, {}!".to_string(),
            args: vec![Expr::Ident("name".to_string())],
        };
        match e {
            Expr::FormatMacro { template, args } => {
                assert_eq!(template, "Hello, {}!");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected FormatMacro"),
        }
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
    fn test_stmt_while() {
        let stmt = Stmt::While {
            label: None,
            condition: Expr::BoolLit(true),
            body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
        };
        match stmt {
            Stmt::While {
                condition, body, ..
            } => {
                assert_eq!(condition, Expr::BoolLit(true));
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn test_stmt_for_in() {
        let stmt = Stmt::ForIn {
            label: None,
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Some(Box::new(Expr::NumberLit(0.0))),
                end: Some(Box::new(Expr::NumberLit(10.0))),
            },
            body: vec![],
        };
        match stmt {
            Stmt::ForIn {
                var,
                iterable,
                body,
                ..
            } => {
                assert_eq!(var, "i");
                assert!(matches!(iterable, Expr::Range { .. }));
                assert!(body.is_empty());
            }
            _ => panic!("expected ForIn"),
        }
    }

    #[test]
    fn test_expr_range() {
        let expr = Expr::Range {
            start: Some(Box::new(Expr::NumberLit(0.0))),
            end: Some(Box::new(Expr::NumberLit(5.0))),
        };
        match expr {
            Expr::Range { start, end } => {
                assert_eq!(*start.unwrap(), Expr::NumberLit(0.0));
                assert_eq!(*end.unwrap(), Expr::NumberLit(5.0));
            }
            _ => panic!("expected Range"),
        }
    }

    #[test]
    fn test_expr_fn_call_err() {
        let expr = Expr::FnCall {
            name: "Err".to_string(),
            args: vec![Expr::StringLit("something went wrong".to_string())],
        };
        match expr {
            Expr::FnCall { name, args } => {
                assert_eq!(name, "Err");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected FnCall"),
        }
    }

    #[test]
    fn test_expr_fn_call_ok() {
        let expr = Expr::FnCall {
            name: "Ok".to_string(),
            args: vec![Expr::NumberLit(42.0)],
        };
        match expr {
            Expr::FnCall { name, args } => {
                assert_eq!(name, "Ok");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected FnCall"),
        }
    }

    #[test]
    fn test_expr_return() {
        let stmt = Stmt::Return(Some(Expr::NumberLit(1.0)));
        match stmt {
            Stmt::Return(Some(Expr::NumberLit(n))) => assert_eq!(n, 1.0),
            _ => panic!("expected Return"),
        }
    }
}
