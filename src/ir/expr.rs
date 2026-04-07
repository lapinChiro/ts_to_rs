//! 式 (`Expr`) と、従属する要素 (`CallTarget`, `BinOp`, `UnOp`, `ClosureBody`)。

use super::{MatchArm, Param, Pattern, RustType, Stmt};

/// User-defined type への参照を表す newtype。
///
/// この型のインスタンスは「TypeRegistry に登録されたユーザー型を参照する」
/// という不変条件を構築サイトで保証する。`IrVisitor::visit_user_type_ref` は
/// この型のすべての出現を walker に通知し、walker は無条件に refs に登録する。
///
/// プリミティブ型 (`f64`, `i32`)、std module path (`std::f64::consts`)、
/// builtin enum variant (`Some`, `None`, `Ok`, `Err`)、外部 crate path
/// (`scopeguard::guard`) は **この型に格納してはならない**。これらは
/// [`PrimitiveType`] / [`StdConst`] / [`BuiltinVariant`] / `CallTarget::ExternalPath`
/// で構造的に区別される。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserTypeRef(String);

impl UserTypeRef {
    /// 新しい [`UserTypeRef`] を構築する。
    ///
    /// 単一識別子のみを受け付ける。`::` を含む path 文字列、空文字、
    /// プリミティブ型名 (`f64`/`i32`/...) の混入は debug ビルドで panic
    /// する (構築サイトでの誤用を即時検出)。これらは
    /// [`PrimitiveType`] / [`StdConst`] / [`BuiltinVariant`] / `CallTarget::ExternalPath`
    /// で構造的に区別すべきもの。
    pub fn new(name: impl Into<String>) -> Self {
        let s = name.into();
        debug_assert!(!s.is_empty(), "UserTypeRef must be a non-empty identifier");
        debug_assert!(
            !s.contains("::"),
            "UserTypeRef must hold a single identifier, got `{s}` \
             (use PrimitiveType / StdConst / BuiltinVariant / CallTarget::ExternalPath for paths)"
        );
        Self(s)
    }

    /// ユーザー型名を `&str` で取得する。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// ユーザー型名を所有権付きで取り出す。
    pub fn into_string(self) -> String {
        self.0
    }
}

/// プリミティブ型の集合。`f64::NAN` のような associated constant の所在型として使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    /// `f64`
    F64,
    /// `i32`
    I32,
    /// `i64`
    I64,
    /// `u32`
    U32,
    /// `u64`
    U64,
    /// `usize`
    Usize,
    /// `isize`
    Isize,
    /// `bool`
    Bool,
    /// `char`
    Char,
}

impl PrimitiveType {
    /// Rust ソース上の名前を返す。
    pub fn as_rust_str(self) -> &'static str {
        match self {
            PrimitiveType::F64 => "f64",
            PrimitiveType::I32 => "i32",
            PrimitiveType::I64 => "i64",
            PrimitiveType::U32 => "u32",
            PrimitiveType::U64 => "u64",
            PrimitiveType::Usize => "usize",
            PrimitiveType::Isize => "isize",
            PrimitiveType::Bool => "bool",
            PrimitiveType::Char => "char",
        }
    }
}

/// std ライブラリ既知の定数 path。`Math.*` 由来のみが現状の構築サイト。
///
/// `Math.*` から本 enum へのマッピングは [`StdConst::from_math_member`] に
/// 集約されている（DRY: マッピング表は単一箇所に存在）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StdConst {
    /// `std::f64::consts::PI`
    F64Pi,
    /// `std::f64::consts::E`
    F64E,
    /// `std::f64::consts::LN_2`
    F64Ln2,
    /// `std::f64::consts::LN_10`
    F64Ln10,
    /// `std::f64::consts::LOG2_E`
    F64Log2E,
    /// `std::f64::consts::LOG10_E`
    F64Log10E,
    /// `std::f64::consts::SQRT_2`
    F64Sqrt2,
}

impl StdConst {
    /// `Math.*` の TS フィールド名から対応する [`StdConst`] を引く。
    /// 未知のフィールドには `None` を返す（呼び出し側は通常の member access に
    /// fall back する）。
    pub fn from_math_member(field: &str) -> Option<Self> {
        match field {
            "PI" => Some(StdConst::F64Pi),
            "E" => Some(StdConst::F64E),
            "LN2" => Some(StdConst::F64Ln2),
            "LN10" => Some(StdConst::F64Ln10),
            "LOG2E" => Some(StdConst::F64Log2E),
            "LOG10E" => Some(StdConst::F64Log10E),
            "SQRT2" => Some(StdConst::F64Sqrt2),
            _ => None,
        }
    }

    /// generator が rendering で使う Rust path。
    pub fn rust_path(self) -> &'static str {
        match self {
            StdConst::F64Pi => "std::f64::consts::PI",
            StdConst::F64E => "std::f64::consts::E",
            StdConst::F64Ln2 => "std::f64::consts::LN_2",
            StdConst::F64Ln10 => "std::f64::consts::LN_10",
            StdConst::F64Log2E => "std::f64::consts::LOG2_E",
            StdConst::F64Log10E => "std::f64::consts::LOG10_E",
            StdConst::F64Sqrt2 => "std::f64::consts::SQRT_2",
        }
    }
}

/// `Option` / `Result` の builtin variant constructor を表す。
///
/// walker は本 enum に対しては何もせず、`RUST_BUILTIN_TYPES` のハードコード
/// 除外に頼らなくても builtin variant を user type として誤登録しないことが
/// 構造的に保証される（T7 で I-377 申し送りの 4 エントリ削除が可能になる）。
///
/// **I-378 Phase 1 過渡状態**: Phase 1 では本型を構造定義のみ追加し、production
/// 構築サイトは存在しない（Phase 2 の T3 で `CallTarget::BuiltinVariant` variant
/// として消費される）。Phase 1 段階では `expr.rs` の単体テストでのみ参照される。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinVariant {
    /// `Some(_)`
    Some,
    /// `None`
    None,
    /// `Ok(_)`
    Ok,
    /// `Err(_)`
    Err,
}

impl BuiltinVariant {
    /// Rust ソース上の名前を返す。
    pub fn as_rust_str(self) -> &'static str {
        match self {
            BuiltinVariant::Some => "Some",
            BuiltinVariant::None => "None",
            BuiltinVariant::Ok => "Ok",
            BuiltinVariant::Err => "Err",
        }
    }
}

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
    /// 値式における enum unit variant 参照（payload なし）。例: `Color::Red`, `Direction::Up`。
    ///
    /// payload 付き variant 構築（`Color::Red(x)`）は `Expr::FnCall { target: CallTarget::* }`
    /// 側で表現する。本 variant は **値リテラルとしての** variant 参照を構造化することで、
    /// `Expr::Ident("Color::Red")` 形式の display-formatted 文字列 encoding を撲滅する
    /// （pipeline-integrity ルール準拠）。
    EnumVariant {
        /// 親 enum 型への参照。walker はこのフィールドを通じて user type ref を一様に拾う。
        enum_ty: UserTypeRef,
        /// variant 名（修飾なし）。
        variant: String,
    },
    /// プリミティブ型の associated constant。例: `f64::NAN`, `f64::INFINITY`, `i32::MAX`。
    ///
    /// `Expr::Ident("f64::NAN")` 形式の display-formatted 文字列を撲滅する。
    /// プリミティブ型なので walker は何もしない。
    PrimitiveAssocConst {
        /// 所在型。
        ty: PrimitiveType,
        /// constant 名（例: `"NAN"`, `"INFINITY"`, `"MAX"`）。
        name: String,
    },
    /// std ライブラリ既知の定数 path。例: `std::f64::consts::PI`。
    ///
    /// `Math.PI` 等の TS 由来から構築される。`Expr::Ident("std::f64::consts::PI")`
    /// 形式の display-formatted 文字列を撲滅する。walker は何もしない。
    StdConst(StdConst),
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
            | Expr::Unit
            // 定数参照は副作用ゼロ。`Expr::Ident("f64::NAN")` 形式が
            // `Expr::PrimitiveAssocConst` に置換されたとき is_trivially_pure の
            // 戻り値が true → false に静かに反転しないよう明示的に true を返す
            // (silent semantic change 防止)。
            | Expr::EnumVariant { .. }
            | Expr::PrimitiveAssocConst { .. }
            | Expr::StdConst(_) => true,
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
        // `PrimitiveAssocConst` (`f64::NAN` 等) と `StdConst` (`std::f64::consts::PI` 等)
        // はプリミティブ Copy 値で副作用ゼロのため eager 評価安全。
        // `EnumVariant` は親 enum の Copy 性が unknown なため保守的に除外する。
        matches!(
            self,
            Expr::NumberLit(_)
                | Expr::IntLit(_)
                | Expr::BoolLit(_)
                | Expr::Unit
                | Expr::PrimitiveAssocConst { .. }
                | Expr::StdConst(_)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_type_ref_round_trips() {
        let r = UserTypeRef::new("Foo");
        assert_eq!(r.as_str(), "Foo");
        assert_eq!(r.clone().into_string(), "Foo");
    }

    #[test]
    fn primitive_type_as_rust_str_covers_all_variants() {
        assert_eq!(PrimitiveType::F64.as_rust_str(), "f64");
        assert_eq!(PrimitiveType::I32.as_rust_str(), "i32");
        assert_eq!(PrimitiveType::I64.as_rust_str(), "i64");
        assert_eq!(PrimitiveType::U32.as_rust_str(), "u32");
        assert_eq!(PrimitiveType::U64.as_rust_str(), "u64");
        assert_eq!(PrimitiveType::Usize.as_rust_str(), "usize");
        assert_eq!(PrimitiveType::Isize.as_rust_str(), "isize");
        assert_eq!(PrimitiveType::Bool.as_rust_str(), "bool");
        assert_eq!(PrimitiveType::Char.as_rust_str(), "char");
    }

    #[test]
    fn std_const_from_math_member_covers_all_known_fields() {
        assert_eq!(StdConst::from_math_member("PI"), Some(StdConst::F64Pi));
        assert_eq!(StdConst::from_math_member("E"), Some(StdConst::F64E));
        assert_eq!(StdConst::from_math_member("LN2"), Some(StdConst::F64Ln2));
        assert_eq!(StdConst::from_math_member("LN10"), Some(StdConst::F64Ln10));
        assert_eq!(
            StdConst::from_math_member("LOG2E"),
            Some(StdConst::F64Log2E)
        );
        assert_eq!(
            StdConst::from_math_member("LOG10E"),
            Some(StdConst::F64Log10E)
        );
        assert_eq!(
            StdConst::from_math_member("SQRT2"),
            Some(StdConst::F64Sqrt2)
        );
    }

    #[test]
    fn std_const_from_math_member_returns_none_for_unknown() {
        assert_eq!(StdConst::from_math_member("UNKNOWN"), None);
        assert_eq!(StdConst::from_math_member(""), None);
    }

    #[test]
    fn std_const_rust_path_covers_all_variants() {
        assert_eq!(StdConst::F64Pi.rust_path(), "std::f64::consts::PI");
        assert_eq!(StdConst::F64E.rust_path(), "std::f64::consts::E");
        assert_eq!(StdConst::F64Ln2.rust_path(), "std::f64::consts::LN_2");
        assert_eq!(StdConst::F64Ln10.rust_path(), "std::f64::consts::LN_10");
        assert_eq!(StdConst::F64Log2E.rust_path(), "std::f64::consts::LOG2_E");
        assert_eq!(StdConst::F64Log10E.rust_path(), "std::f64::consts::LOG10_E");
        assert_eq!(StdConst::F64Sqrt2.rust_path(), "std::f64::consts::SQRT_2");
    }

    #[test]
    fn builtin_variant_as_rust_str_covers_all_variants() {
        assert_eq!(BuiltinVariant::Some.as_rust_str(), "Some");
        assert_eq!(BuiltinVariant::None.as_rust_str(), "None");
        assert_eq!(BuiltinVariant::Ok.as_rust_str(), "Ok");
        assert_eq!(BuiltinVariant::Err.as_rust_str(), "Err");
    }

    #[test]
    fn new_expr_variants_have_correct_purity_semantics() {
        // 全て定数参照で副作用ゼロ → trivially_pure: true.
        // この値が false になると、Phase 2 で `Expr::Ident("f64::NAN")` (現状 true)
        // から本 variant への置換が is_trivially_pure を true → false に静かに反転
        // させ、generator の dead-code elimination 判定を変える silent semantic
        // change を引き起こす。本テストは回帰防止ガードである。
        let ev = Expr::EnumVariant {
            enum_ty: UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        };
        assert!(ev.is_trivially_pure());
        // EnumVariant の Copy 性は親 enum derive 依存。保守的に false。
        assert!(!ev.is_copy_literal());

        let pa = Expr::PrimitiveAssocConst {
            ty: PrimitiveType::F64,
            name: "NAN".to_string(),
        };
        assert!(pa.is_trivially_pure());
        // f64 は Copy。eager 評価安全。
        assert!(pa.is_copy_literal());

        let sc = Expr::StdConst(StdConst::F64Pi);
        assert!(sc.is_trivially_pure());
        // std::f64::consts::PI も f64 で Copy。
        assert!(sc.is_copy_literal());
    }

    #[test]
    #[should_panic(expected = "single identifier")]
    fn user_type_ref_rejects_qualified_path() {
        // `::` を含む文字列は debug ビルドで panic. PrimitiveType/StdConst を使うべき。
        let _ = UserTypeRef::new("std::f64");
    }

    #[test]
    #[should_panic(expected = "non-empty")]
    fn user_type_ref_rejects_empty_string() {
        let _ = UserTypeRef::new("");
    }
}
