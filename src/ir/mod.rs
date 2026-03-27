//! Intermediate Representation (IR) for Rust code generation.
//!
//! The IR sits between the SWC TypeScript AST and Rust source code generation.
//! It models the subset of Rust constructs needed for Phase 1 of ts_to_rs.

/// ジェネリック型パラメータ（名前 + オプショナルな制約）。
///
/// IR と TypeRegistry の両方で使用される。
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    /// 型パラメータ名（例: "T"）
    pub name: String,
    /// 制約（例: `T extends Foo` → `Some(Named("Foo"))`）
    pub constraint: Option<RustType>,
}

/// trait への参照（名前 + 型引数）。
///
/// `impl TraitName<T>` の `TraitName<T>` や `trait Foo: Bar<T>` の `Bar<T>` を表す。
#[derive(Debug, Clone, PartialEq)]
pub struct TraitRef {
    /// trait 名
    pub name: String,
    /// 型引数（例: `Trait<String, T>` → `[String, Named("T")]`）
    pub type_args: Vec<RustType>,
}

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
    /// `serde_json::Value` (corresponds to TypeScript `any` and `unknown`).
    ///
    /// For `any`-typed function parameters with typeof/instanceof checks, the transformer
    /// generates a custom enum via lazy type materialization (`any_narrowing.rs`) and
    /// replaces this with `RustType::Named`. This fallback is used only when no
    /// typeof/instanceof usage is detected.
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
    /// A reference type: `&T` (e.g., `&dyn Greeter`)
    Ref(Box<RustType>),
    /// A trait object type: `dyn T` (e.g., `dyn Greeter`)
    ///
    /// Used with `Ref` for `&dyn Trait` parameters and with `Named { name: "Box" }` for `Box<dyn Trait>`.
    DynTrait(String),
}

impl RustType {
    /// Returns true if this type references the given type parameter name.
    pub fn uses_param(&self, param: &str) -> bool {
        match self {
            RustType::Named { name, type_args } => {
                name == param
                    // Handle qualified paths like `<T as Promise>::Output`
                    || name.contains(&format!("<{param} "))
                    || name.contains(&format!("<{param}>"))
                    || type_args.iter().any(|a| a.uses_param(param))
            }
            RustType::Option(inner) | RustType::Vec(inner) | RustType::Ref(inner) => {
                inner.uses_param(param)
            }
            RustType::Result { ok, err } => ok.uses_param(param) || err.uses_param(param),
            RustType::Tuple(elems) => elems.iter().any(|e| e.uses_param(param)),
            RustType::Fn {
                params,
                return_type,
            } => params.iter().any(|p| p.uses_param(param)) || return_type.uses_param(param),
            RustType::DynTrait(name) => name == param,
            _ => false,
        }
    }

    /// 型パラメータ名を具体型に置換する。
    ///
    /// `bindings` は型パラメータ名 → 具体型のマッピング。
    /// `Named { name: "T" }` が `bindings` に存在すれば具体型に置換し、
    /// それ以外のバリアントは再帰的に処理する。
    pub fn substitute(&self, bindings: &std::collections::HashMap<String, RustType>) -> RustType {
        match self {
            RustType::Named { name, type_args } => {
                if type_args.is_empty() {
                    if let Some(concrete) = bindings.get(name.as_str()) {
                        return concrete.clone();
                    }
                }
                RustType::Named {
                    name: name.clone(),
                    type_args: type_args.iter().map(|a| a.substitute(bindings)).collect(),
                }
            }
            RustType::Vec(inner) => RustType::Vec(Box::new(inner.substitute(bindings))),
            RustType::Option(inner) => RustType::Option(Box::new(inner.substitute(bindings))),
            RustType::Ref(inner) => RustType::Ref(Box::new(inner.substitute(bindings))),
            RustType::Result { ok, err } => RustType::Result {
                ok: Box::new(ok.substitute(bindings)),
                err: Box::new(err.substitute(bindings)),
            },
            RustType::Tuple(elems) => {
                RustType::Tuple(elems.iter().map(|e| e.substitute(bindings)).collect())
            }
            RustType::Fn {
                params,
                return_type,
            } => RustType::Fn {
                params: params.iter().map(|p| p.substitute(bindings)).collect(),
                return_type: Box::new(return_type.substitute(bindings)),
            },
            other => other.clone(),
        }
    }
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

/// struct フィールド名を有効な Rust 識別子文字列に変換する（文字レベルのサニタイズ）。
///
/// TypeScript のオブジェクトキーは任意の文字列だが、Rust の識別子には制約がある。
/// 以下の変換を適用:
/// 1. ハイフン → アンダースコア（`Content-Type` → `Content_Type`）
/// 2. ブラケット除去（`foo[]` → `foo`）
/// 3. `_` のみ → `_field`（Rust では `_` は破棄パターン）
/// 4. 先頭が数字 → `_` プレフィクス
/// 5. 空文字列 → `_empty`
///
/// 注意: Rust 予約語のエスケープ（`r#` プレフィクス）は行わない。
/// それは generator の `escape_ident` の責務。
pub fn sanitize_field_name(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '-' => sanitized.push('_'),
            '[' | ']' => {}
            _ => sanitized.push(ch),
        }
    }

    if sanitized == "_" {
        return "_field".to_string();
    }

    if sanitized.starts_with(|c: char| c.is_ascii_digit()) {
        sanitized.insert(0, '_');
    }

    if sanitized.is_empty() {
        return "_empty".to_string();
    }

    sanitized
}

/// camelCase を snake_case に変換する。
///
/// 連続する大文字は略語として扱い、最後の大文字を次の単語の先頭とする。
/// 例: `"byteLength"` → `"byte_length"`, `"toISOString"` → `"to_iso_string"`
pub fn camel_to_snake(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                let prev_upper = chars[i - 1].is_uppercase();
                let next_lower = chars.get(i + 1).is_some_and(|c| c.is_lowercase());
                if !prev_upper || next_lower {
                    result.push('_');
                }
            }
            result.push(ch.to_lowercase().next().unwrap_or(ch));
        } else {
            result.push(ch);
        }
    }
    result
}

/// A pattern in a match arm.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    /// A literal value pattern (e.g., `1`, `"hello"`)
    Literal(Expr),
    /// A wildcard pattern (`_`)
    Wildcard,
    /// An enum variant pattern (e.g., `Shape::Circle { radius, .. }`)
    EnumVariant {
        /// Fully qualified variant name (e.g., `"Shape::Circle"`)
        path: String,
        /// Field names to bind in the pattern. Empty means `{ .. }`.
        bindings: Vec<String>,
    },
}

/// An arm in a `match` expression.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Patterns for this arm. Multiple patterns represent `a | b | _`.
    pub patterns: Vec<MatchPattern>,
    /// Optional match guard: `_ if guard_expr => { ... }`.
    pub guard: Option<Expr>,
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
        /// Generic type parameters
        type_params: Vec<TypeParam>,
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
        /// Generic type parameters
        type_params: Vec<TypeParam>,
        /// Supertrait bounds (e.g., `[TraitRef("Animal"), TraitRef("Debug")]` → `trait Dog: Animal + Debug`)
        supertraits: Vec<TraitRef>,
        /// Method signatures (body is empty — signatures only)
        methods: Vec<Method>,
        /// Associated type declarations (e.g., `type Output;`)
        associated_types: Vec<String>,
    },
    /// An `impl` block for a struct, optionally implementing a trait.
    Impl {
        /// Struct name this impl is for
        struct_name: String,
        /// Generic type parameters (e.g., `impl<T> Foo<T>`)
        type_params: Vec<TypeParam>,
        /// If `Some`, this is a trait impl: `impl TraitName<T> for StructName<T>`
        for_trait: Option<TraitRef>,
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
        type_params: Vec<TypeParam>,
        /// The aliased type
        ty: RustType,
    },
    /// A `fn` declaration.
    Fn {
        /// Visibility
        vis: Visibility,
        /// Attributes (e.g., `["tokio::main"]` → `#[tokio::main]`)
        attributes: Vec<String>,
        /// Whether this is an `async fn`
        is_async: bool,
        /// Function name
        name: String,
        /// Generic type parameters
        type_params: Vec<TypeParam>,
        /// Parameters
        params: Vec<Param>,
        /// Return type (`None` means `()`)
        return_type: Option<RustType>,
        /// Function body
        body: Vec<Stmt>,
    },
    /// Raw Rust code emitted verbatim by the generator.
    ///
    /// Used for helper functions whose structure is not worth modelling in IR
    /// (e.g., `js_typeof`). Should be used sparingly — prefer structured IR.
    RawCode(String),
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
        /// Pattern to match (e.g., `"Some(x)"`)
        pattern: String,
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
        /// Pattern to match (e.g., `"Some(x)"`, `"Enum::Variant(x)"`)
        pattern: String,
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
        /// Pattern to match against (raw Rust pattern string)
        pattern: String,
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
mod tests;
