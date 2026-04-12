//! トップレベル宣言: `Item` と、その構成要素 (`EnumValue`, `EnumVariant`, `StructField`,
//! `Param`, `AssocConst`, `Method`)。

use super::{Expr, RustType, Stmt, TraitRef, TypeParam, Visibility};

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
    /// Whether this is an `async fn` method
    pub is_async: bool,
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
        /// Generic type parameters
        type_params: Vec<TypeParam>,
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
    /// A module-level `const` declaration: `const NAME: Ty = value;`
    ///
    /// Used for callable interface marker struct instances
    /// (e.g., `const getCookie: GetCookieImpl = GetCookieImpl;`).
    Const {
        /// Visibility
        vis: Visibility,
        /// Constant name
        name: String,
        /// Type
        ty: RustType,
        /// Value expression
        value: Expr,
    },
    /// Raw Rust code emitted verbatim by the generator.
    ///
    /// Used for helper functions whose structure is not worth modelling in IR
    /// (e.g., `js_typeof`). Should be used sparingly — prefer structured IR.
    RawCode(String),
}

impl Item {
    /// Item の識別名を返す。
    ///
    /// 命名対象の Item（`Struct`, `Enum`, `Trait`, `TypeAlias`, `Fn`, `Impl`）は
    /// `Some(name)` を返す。`Comment` / `RawCode` / `Use` のように単一の識別名を
    /// 持たない Item は `None` を返す。
    ///
    /// 合成型の参照グラフ構築や placement 判定など、Item を名前で索引する用途で
    /// 使用する。
    pub fn canonical_name(&self) -> Option<&str> {
        match self {
            Item::Struct { name, .. }
            | Item::Enum { name, .. }
            | Item::Trait { name, .. }
            | Item::TypeAlias { name, .. }
            | Item::Fn { name, .. }
            | Item::Const { name, .. } => Some(name),
            Item::Impl { struct_name, .. } => Some(struct_name),
            Item::Comment(_) | Item::RawCode(_) | Item::Use { .. } => None,
        }
    }
}
