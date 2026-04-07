//! 関数本体の文 (`Stmt`) と、`match` アーム (`MatchArm`)。

use super::{Expr, Pattern, RustType};

/// An arm in a `match` expression.
///
/// `patterns` uses the structured [`Pattern`] enum (see `pattern.rs`). Before
/// I-377 this was `Vec<MatchPattern>` with a `Verbatim(String)` variant that
/// forced the walker to parse pattern strings with an uppercase-head
/// heuristic. The heuristic produced false negatives for lowercase class
/// names and required hardcoding `Some`/`None`/`Ok`/`Err` into
/// `RUST_BUILTIN_TYPES`. Both band-aids are removed now that patterns are
/// structured.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Patterns for this arm. Multiple patterns represent `a | b | _`.
    pub patterns: Vec<Pattern>,
    /// Optional match guard: `_ if guard_expr => { ... }`.
    pub guard: Option<Expr>,
    /// Arm body.
    pub body: Vec<Stmt>,
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
    /// `['label:] while let <pattern> = <expr> { ... }`
    WhileLet {
        /// Optional loop label (e.g., `'outer`)
        label: Option<String>,
        /// Pattern to match (e.g., `Some(x)`)
        pattern: Pattern,
        /// Expression to match against
        expr: Expr,
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
        /// Pattern to match (e.g., `Err(e)`)
        pattern: Pattern,
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
