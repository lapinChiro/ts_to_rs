//! 型の形を表す IR プリミティブ: `TypeParam`, `TraitRef`, `RustType`, `Visibility`。

/// ジェネリック型パラメータ（名前 + オプショナルな制約）。
///
/// 型パラメータ `T` によって制約の型表現を切り替える:
/// - `TypeParam<RustType>` (= `TypeParam`): Rust 型制約。IR・TypeRegistry・Generator で使用。
/// - `TypeParam<TsTypeInfo>`: TS 型制約。registry の collection フェーズで使用。
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam<T = RustType> {
    /// 型パラメータ名（例: "T"）
    pub name: String,
    /// 制約（例: `T extends Foo` → `Some(Named("Foo"))`）
    pub constraint: Option<T>,
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
    /// `std::convert::Infallible` (corresponds to TypeScript `never`)
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
    /// 限定パス型: `<Self as Trait<Args>>::Item`（associated type 参照）
    ///
    /// 用途: TypeScript の conditional type の `infer` 抽出。
    /// 例えば `T extends Promise<infer U> ? U : never` は、ヘルパ trait `Promise<Output>`
    /// を導入したうえで `<T as Promise>::Output` という qualified path として表現する。
    ///
    /// `pipeline-integrity.md` に従い、display-formatted 文字列を `Named.name` に
    /// 詰め込むのではなく構造化して保持する。
    QSelf {
        /// `<` の中の self 型（例: `T`）
        qself: Box<RustType>,
        /// 限定の対象 trait（例: `Promise<U>`）
        trait_ref: TraitRef,
        /// `::` の後ろの associated item 名（例: `Output`）
        item: String,
    },
}

impl RustType {
    /// Returns true if this type references the given type parameter name.
    pub fn uses_param(&self, param: &str) -> bool {
        match self {
            RustType::Named { name, type_args } => {
                name == param || type_args.iter().any(|a| a.uses_param(param))
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
            RustType::QSelf {
                qself, trait_ref, ..
            } => {
                qself.uses_param(param)
                    || trait_ref.name == param
                    || trait_ref.type_args.iter().any(|a| a.uses_param(param))
            }
            _ => false,
        }
    }

    /// Wraps `self` in `Option<T>`, preventing double-wrapping.
    ///
    /// If `self` is already `Option<_>`, returns it unchanged.
    pub fn wrap_optional(self) -> RustType {
        match self {
            RustType::Option(_) => self,
            _ => RustType::Option(Box::new(self)),
        }
    }
}

/// Visibility modifier for items.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    /// `pub`
    Public,
    /// `pub(crate)`
    PubCrate,
    /// No visibility modifier (private by default)
    Private,
}
