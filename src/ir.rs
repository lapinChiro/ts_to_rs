//! Intermediate Representation (IR) for Rust code generation.
//!
//! The IR sits between the SWC TypeScript AST and Rust source code generation.
//! It models the subset of Rust constructs needed for Phase 1 of ts_to_rs.

/// Represents a Rust type.
#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
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
    /// No visibility modifier (private by default)
    Private,
}

/// A named field in a struct.
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    /// Field name
    pub name: String,
    /// Field type
    pub ty: RustType,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub ty: RustType,
}

/// A method inside an `impl` block.
#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    /// Visibility
    pub vis: Visibility,
    /// Method name
    pub name: String,
    /// Whether this method takes `&self` (false for associated functions like `new`)
    pub has_self: bool,
    /// Parameters (excluding `self`)
    pub params: Vec<Param>,
    /// Return type (`None` means `()`)
    pub return_type: Option<RustType>,
    /// Method body
    pub body: Vec<Stmt>,
}

/// Top-level item in a Rust file or module.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A `use` statement: `use path::{names};`
    Use {
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
    /// An `enum` with unit (string-like) variants.
    Enum {
        /// Visibility
        vis: Visibility,
        /// Enum name
        name: String,
        /// Variant names
        variants: Vec<String>,
    },
    /// An `impl` block for a struct.
    Impl {
        /// Struct name this impl is for
        struct_name: String,
        /// Methods in the impl block
        methods: Vec<Method>,
    },
    /// A `fn` declaration.
    Fn {
        /// Visibility
        vis: Visibility,
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
    /// `while <condition> { ... }`
    While {
        /// Loop condition
        condition: Expr,
        /// Loop body
        body: Vec<Stmt>,
    },
    /// `for <var> in <iterable> { ... }`
    ForIn {
        /// Loop variable name
        var: String,
        /// Iterable expression (e.g., a range or collection)
        iterable: Expr,
        /// Loop body
        body: Vec<Stmt>,
    },
    /// `return [<expr>];`
    Return(Option<Expr>),
    /// A bare expression statement (e.g., a function call).
    Expr(Expr),
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
    /// A struct initializer: `Self { field1: val1, field2: val2 }`
    StructInit {
        /// Struct name (e.g., `Self`)
        name: String,
        /// Field name-value pairs
        fields: Vec<(String, Expr)>,
    },
    /// An assignment: `<target> = <value>`
    Assign {
        /// Assignment target
        target: Box<Expr>,
        /// Assigned value
        value: Box<Expr>,
    },
    /// A binary operation: `<left> <op> <right>`
    BinaryOp {
        /// Left operand
        left: Box<Expr>,
        /// Operator (e.g., `+`, `-`, `==`, `>`)
        op: String,
        /// Right operand
        right: Box<Expr>,
    },
    /// A range expression: `start..end`
    Range {
        /// Start of range (inclusive)
        start: Box<Expr>,
        /// End of range (exclusive)
        end: Box<Expr>,
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
                    name: "x".to_string(),
                    ty: RustType::F64,
                },
                StructField {
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
    fn test_item_enum() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Direction".to_string(),
            variants: vec!["North".to_string(), "South".to_string()],
        };
        match item {
            Item::Enum { name, variants, .. } => {
                assert_eq!(name, "Direction");
                assert_eq!(variants.len(), 2);
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_item_fn() {
        let item = Item::Fn {
            vis: Visibility::Public,
            name: "add".to_string(),
            type_params: vec![],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: RustType::F64,
                },
                Param {
                    name: "b".to_string(),
                    ty: RustType::F64,
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
            condition: Expr::BoolLit(true),
            body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
        };
        match stmt {
            Stmt::While { condition, body } => {
                assert_eq!(condition, Expr::BoolLit(true));
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn test_stmt_for_in() {
        let stmt = Stmt::ForIn {
            var: "i".to_string(),
            iterable: Expr::Range {
                start: Box::new(Expr::NumberLit(0.0)),
                end: Box::new(Expr::NumberLit(10.0)),
            },
            body: vec![],
        };
        match stmt {
            Stmt::ForIn {
                var,
                iterable,
                body,
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
            start: Box::new(Expr::NumberLit(0.0)),
            end: Box::new(Expr::NumberLit(5.0)),
        };
        match expr {
            Expr::Range { start, end } => {
                assert_eq!(*start, Expr::NumberLit(0.0));
                assert_eq!(*end, Expr::NumberLit(5.0));
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
