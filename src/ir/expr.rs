//! 式 (`Expr`) と、従属する要素 (`CallTarget`, `BinOp`, `UnOp`, `ClosureBody`)。

use super::{MatchArm, Param, Pattern, RustType, Stmt};

/// The target of an [`Expr::FnCall`].
///
/// `Expr::FnCall` previously used a single `name: String` field to represent six
/// semantically distinct kinds of call targets (free function, module-qualified
/// call, `Option`/`Result` variant constructor, tuple struct constructor, synthetic
/// enum variant constructor, and `super(args)`). The walker in
/// `external_struct_generator` had to disambiguate these with a Rust-naming-convention
/// heuristic ("uppercase head → type reference"), which produces both false
/// negatives (lowercase class names) and false positives (uppercase free functions).
///
/// `CallTarget` replaces that string with a structured representation:
///
/// - [`CallTarget::Path`] covers every call whose callee is a path of identifiers.
///   The walker consults the explicit [`type_ref`] field instead of parsing segments.
/// - [`CallTarget::Super`] is the `super(args)` call in class inheritance context.
///
/// The `segments` field holds a language-agnostic list of identifiers, following the
/// `pipeline-integrity` rule (`.claude/rules/pipeline-integrity.md`) that IR types must
/// not store display-formatted strings (the `::` separator is Rust-specific and is the
/// generator's responsibility).
///
/// [`type_ref`]: CallTarget::Path::type_ref
#[derive(Debug, Clone, PartialEq)]
pub enum CallTarget {
    /// A path call: `foo(x)`, `Color::Red(x)`, `MyClass::new(x)`, `Some(x)`, etc.
    ///
    /// `segments` holds the callee path as a list of identifiers. The generator joins
    /// them with `::` when emitting Rust source.
    ///
    /// `type_ref` records the user-defined type that this call references, if any.
    /// The walker uses it as-is for the reference graph, without parsing `segments`:
    ///
    /// | call form                 | segments                     | type_ref        |
    /// |---------------------------|------------------------------|-----------------|
    /// | `foo(x)`                  | `["foo"]`                    | `None`          |
    /// | `std::mem::take(x)`       | `["std","mem","take"]`       | `None`          |
    /// | `Some(x)` / `Ok(x)`       | `["Some"]` / `["Ok"]`        | `None` (builtin)|
    /// | `_f(x)` / `__iife(x)`     | `["_f"]` / `["__iife"]`      | `None`          |
    /// | `Wrapper(x)`              | `["Wrapper"]`                | `Some("Wrapper")`|
    /// | `Color::Red(x)`           | `["Color","Red"]`            | `Some("Color")` |
    /// | `MyClass::new(x)`         | `["MyClass","new"]`          | `Some("MyClass")`|
    Path {
        /// Identifier segments of the callee path.
        segments: Vec<String>,
        /// User-defined type referenced by this call, used by the reference walker.
        type_ref: Option<String>,
    },
    /// `super(args)` — the parent constructor call in a class inheritance context.
    Super,
}

impl CallTarget {
    /// Returns the single identifier if this target is a single-segment [`Path`].
    ///
    /// Used as a pattern-match helper to replace former string-literal comparisons
    /// such as `name == "Err"` with structural checks like
    /// `target.as_simple() == Some("Err")`.
    ///
    /// [`Path`]: CallTarget::Path
    pub fn as_simple(&self) -> Option<&str> {
        if let CallTarget::Path { segments, .. } = self {
            if segments.len() == 1 {
                return Some(segments[0].as_str());
            }
        }
        None
    }

    /// Constructs a single-segment [`Path`] with no type reference.
    ///
    /// Used for free functions, Option/Result builtin variant constructors,
    /// and local-variable callees (e.g. `_f(x)`, `__iife(x)`).
    ///
    /// [`Path`]: CallTarget::Path
    pub fn simple(name: impl Into<String>) -> Self {
        CallTarget::Path {
            segments: vec![name.into()],
            type_ref: None,
        }
    }

    /// Returns true if this target is a [`Path`] whose segments exactly match the
    /// given slice, used as a structural replacement for former string-literal
    /// comparisons like `name == "scopeguard::guard"`.
    ///
    /// [`Path`]: CallTarget::Path
    pub fn is_path(&self, expected: &[&str]) -> bool {
        match self {
            CallTarget::Path { segments, .. } => {
                segments.len() == expected.len()
                    && segments.iter().zip(expected).all(|(a, b)| a == b)
            }
            CallTarget::Super => false,
        }
    }

    /// Constructs a two-segment associated-path [`Path`] that references the type
    /// named by the first segment.
    ///
    /// Used for associated function calls (`MyClass::new(x)`) and synthetic enum
    /// variant constructors (`Color::Red(x)`). `type_ref` is set to `type_name`,
    /// so the reference walker will register the type in the graph.
    ///
    /// [`Path`]: CallTarget::Path
    pub fn assoc(type_name: impl Into<String>, member: impl Into<String>) -> Self {
        let type_name = type_name.into();
        CallTarget::Path {
            segments: vec![type_name.clone(), member.into()],
            type_ref: Some(type_name),
        }
    }

    /// Constructs a multi-segment [`Path`] with no type reference from a slice of
    /// identifier segments.
    ///
    /// Used for calls to std library or external crate paths such as
    /// `std::mem::take(x)`, `std::fs::write(...)`, `HashMap::from(v)` — these are
    /// not user-defined types and must not be registered in the reference graph.
    ///
    /// This is the multi-segment counterpart to [`simple`] (single segment, no
    /// type reference) and the complement to [`assoc`] (which sets `type_ref`
    /// for associated function calls on user types).
    ///
    /// [`Path`]: CallTarget::Path
    /// [`simple`]: CallTarget::simple
    /// [`assoc`]: CallTarget::assoc
    pub fn path(segments: &[&str]) -> Self {
        CallTarget::Path {
            segments: segments.iter().map(|s| (*s).to_string()).collect(),
            type_ref: None,
        }
    }
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
    /// A function call: `target(args)` (e.g., `Ok(x)`, `Err("msg".to_string())`,
    /// `Color::Red(x)`, `super(a, b)`).
    ///
    /// See [`CallTarget`] for the structured representation of the callee and the
    /// rationale for replacing the former `name: String` field.
    FnCall {
        /// Structured callee: a path of identifiers or `super`.
        target: CallTarget,
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
    /// A tuple literal: `(a, b, c)`
    Tuple {
        /// Elements of the tuple
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
    /// An `if let` expression: `if let pattern = expr { then } else { else }`
    IfLet {
        /// Pattern to match (e.g., `Some(x)`, `Enum::Variant(x)`).
        ///
        /// `Box`ed because `Pattern::Literal(Expr)` forms a cycle with `Expr::IfLet`;
        /// boxing here breaks the cycle so the enum has finite size.
        pattern: Box<Pattern>,
        /// Expression to match against
        expr: Box<Expr>,
        /// Then branch expression (pattern matched)
        then_expr: Box<Expr>,
        /// Else branch expression (pattern not matched)
        else_expr: Box<Expr>,
    },
    /// A macro call: `name!(args)` (e.g., `println!("{:?}", x)`)
    MacroCall {
        /// Macro name (without `!`)
        name: String,
        /// Arguments
        args: Vec<Expr>,
        /// Per-argument flag: true → use `{:?}` (Debug), false → use `{}` (Display)
        use_debug: Vec<bool>,
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
    IntLit(i128),
    /// Raw Rust code that is emitted verbatim by the generator.
    ///
    /// Used for helper functions whose body is more naturally expressed as
    /// literal Rust than as IR nodes (e.g., `js_typeof`'s match expression).
    RawCode(String),
    /// A runtime `typeof` check: `js_typeof(&operand)`.
    ///
    /// Used when the operand's type cannot be statically resolved (e.g., `any`/`unknown` types).
    /// The generator emits a `js_typeof` helper function that maps `serde_json::Value`
    /// variants to JavaScript typeof strings at runtime.
    RuntimeTypeof {
        /// The operand expression
        operand: Box<Expr>,
    },
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
    /// A `matches!` macro expression: `matches!(expr, pattern)`
    Matches {
        /// Expression to test
        expr: Box<Expr>,
        /// Pattern to match against.
        ///
        /// `Box`ed for the same recursive-type reason as [`Expr::IfLet::pattern`].
        pattern: Box<Pattern>,
    },
    /// A block expression: `{ stmt1; stmt2; tail_expr }`
    Block(Vec<Stmt>),
    /// A match expression: `match expr { arms }`
    Match {
        /// Expression to match against
        expr: Box<Expr>,
        /// Match arms
        arms: Vec<MatchArm>,
    },
    /// A compiled regex literal: `Regex::new("pattern").unwrap()`
    ///
    /// Preserves the `g` (global) and `y` (sticky) flags from the original TypeScript regex.
    /// The `g` flag affects method selection (e.g., `replace` vs `replace_all`).
    /// The `y` flag has no Rust equivalent and generates a warning comment.
    Regex {
        /// The regex pattern with inline flags embedded (e.g., `"(?i)pattern"`)
        pattern: String,
        /// Whether the `g` (global) flag was present
        global: bool,
        /// Whether the `y` (sticky) flag was present
        sticky: bool,
    },
}

impl Expr {
    /// Returns true if the expression has no observable side effects and can be safely dropped.
    ///
    /// Conservative: returns false for anything that might have side effects (function calls,
    /// method calls, assignments, macros, etc.). Only returns true for expressions that are
    /// provably pure:
    /// - Literals (`NumberLit`, `IntLit`, `StringLit`, `BoolLit`, `Unit`)
    /// - Identifiers (`Ident`)
    /// - Field access on pure objects (`FieldAccess`)
    /// - Transparent wrappers around pure expressions (`Ref`, `Deref`)
    /// - Known-pure method calls (`to_string`, `clone`, `to_owned`) on pure receivers
    pub fn is_trivially_pure(&self) -> bool {
        match self {
            Expr::NumberLit(_)
            | Expr::IntLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::Ident(_)
            | Expr::Unit => true,
            Expr::Ref(inner) | Expr::Deref(inner) => inner.is_trivially_pure(),
            Expr::FieldAccess { object, .. } => object.is_trivially_pure(),
            // Transpiler-generated conversion methods with no side effects
            Expr::MethodCall { object, method, .. }
                if matches!(method.as_str(), "to_string" | "clone" | "to_owned") =>
            {
                object.is_trivially_pure()
            }
            _ => false,
        }
    }

    /// Returns true if the expression is a cheap Copy literal safe for eager evaluation.
    ///
    /// Used to decide `unwrap_or` (eager) vs `unwrap_or_else` (lazy):
    /// - Copy literals are cheap and have no ownership/allocation concerns → `unwrap_or`
    /// - Everything else (String allocation, side effects, non-Copy move) → `unwrap_or_else`
    pub fn is_copy_literal(&self) -> bool {
        matches!(
            self,
            Expr::NumberLit(_) | Expr::IntLit(_) | Expr::BoolLit(_) | Expr::Unit
        )
    }
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
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `>>>` (unsigned right shift — Rust: `(x as u32) >> (n as u32)`)
    UShr,
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
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::UShr => ">>",
        }
    }

    /// Returns `true` if this operator is a bitwise operator.
    pub fn is_bitwise(self) -> bool {
        matches!(
            self,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr | BinOp::UShr
        )
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
            BinOp::BitOr => 4,
            BinOp::BitXor => 5,
            BinOp::BitAnd => 6,
            BinOp::Shl | BinOp::Shr | BinOp::UShr => 7,
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
