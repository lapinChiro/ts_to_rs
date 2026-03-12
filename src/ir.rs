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
    /// A user-defined named type (e.g., `Point`)
    Named(String),
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
    /// A `fn` declaration.
    Fn {
        /// Visibility
        vis: Visibility,
        /// Function name
        name: String,
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
    /// A binary operation: `<left> <op> <right>`
    BinaryOp {
        /// Left operand
        left: Box<Expr>,
        /// Operator (e.g., `+`, `-`, `==`, `>`)
        op: String,
        /// Right operand
        right: Box<Expr>,
    },
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
    fn test_expr_return() {
        let stmt = Stmt::Return(Some(Expr::NumberLit(1.0)));
        match stmt {
            Stmt::Return(Some(Expr::NumberLit(n))) => assert_eq!(n, 1.0),
            _ => panic!("expected Return"),
        }
    }
}
