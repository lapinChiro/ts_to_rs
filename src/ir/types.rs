//! 型の形を表す IR プリミティブ: `TypeParam`, `TraitRef`, `RustType`, `Visibility`。

/// ジェネリック型パラメータ（名前 + オプショナルな制約 + デフォルト値）。
///
/// 型パラメータ `T` によって制約/デフォルトの型表現を切り替える:
/// - `TypeParam<RustType>` (= `TypeParam`): Rust 型制約。IR・TypeRegistry・Generator で使用。
/// - `TypeParam<TsTypeInfo>`: TS 型制約。registry の collection フェーズで使用。
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam<T = RustType> {
    /// 型パラメータ名（例: "T"）
    pub name: String,
    /// 制約（例: `T extends Foo` → `Some(Named("Foo"))`）
    pub constraint: Option<T>,
    /// デフォルト値（例: `T = string` → `Some(String)`）。
    /// TypeScript ではデフォルト付き型パラメータは省略可能。
    pub default: Option<T>,
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

/// Rust 整数型および `f32` を表す (I-387)。
///
/// `f64` / `bool` / `String` / `()` は `RustType` 本体に専用 variant があるため
/// ここには含めない。`usize`/`isize` 含む整数型と `f32` のみ。
///
/// 設計メモ: `src/ir/expr.rs::PrimitiveType` は「式定数 (`f64::NAN` 等) の所属型」を
/// 表す別概念なので命名衝突を避けるため当 enum は `PrimitiveIntKind` とする。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveIntKind {
    /// `usize`
    Usize,
    /// `isize`
    Isize,
    /// `i8`
    I8,
    /// `i16`
    I16,
    /// `i32`
    I32,
    /// `i64`
    I64,
    /// `i128`
    I128,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `u64`
    U64,
    /// `u128`
    U128,
    /// `f32`
    F32,
}

/// 既存専用 variant を持たない Rust std コレクション・スマートポインタ種別 (I-387)。
///
/// `Vec` / `Option` / `Result` / `Tuple` は `RustType` 本体の専用 variant を使用するため
/// ここには含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StdCollectionKind {
    /// `Box<T>`
    Box,
    /// `HashMap<K, V>`
    HashMap,
    /// `BTreeMap<K, V>`
    BTreeMap,
    /// `HashSet<T>`
    HashSet,
    /// `BTreeSet<T>`
    BTreeSet,
    /// `VecDeque<T>`
    VecDeque,
    /// `Rc<T>`
    Rc,
    /// `Arc<T>`
    Arc,
    /// `Mutex<T>`
    Mutex,
    /// `RwLock<T>`
    RwLock,
    /// `RefCell<T>`
    RefCell,
    /// `Cell<T>`
    Cell,
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
    /// A user-defined named type, optionally with generic type arguments (e.g., `Point`, `HTTPException`).
    ///
    /// **I-387 以降、本 variant は user 定義型のみに限定される**。型変数は `TypeVar`、
    /// Rust std 整数型は `Primitive`、std コレクションは `StdCollection` を使用する。
    Named {
        /// Type name
        name: String,
        /// Generic type arguments (empty if not generic)
        type_args: Vec<RustType>,
    },
    /// 型パラメータ参照 (I-387)。`convert_ts_type` が
    /// `SyntheticTypeRegistry::is_in_type_param_scope(name)` の判定で構築する。
    ///
    /// 下流コード (`substitute` / `collect_type_vars` / `TypeRefCollector`) は
    /// `Named` との区別を構造的に行えるようになり、名前文字列マッチに依存しない。
    TypeVar {
        /// 型変数名 (例: "T", "U", "E")
        name: String,
    },
    /// Rust 整数型および `f32` (I-387)。`f64`/`bool`/`String`/`()` は専用 variant を使う。
    Primitive(PrimitiveIntKind),
    /// Rust std コレクション / スマートポインタ (I-387)。
    ///
    /// `Vec`/`Option`/`Result`/`Tuple` 以外の std 汎用コンテナを構造化する。
    StdCollection {
        /// コレクション種別
        kind: StdCollectionKind,
        /// 型引数 (例: `HashMap<K, V>` → `[K, V]`、`Box<T>` → `[T]`)
        args: Vec<RustType>,
    },
    /// A reference type: `&T` (e.g., `&dyn Greeter`)
    Ref(Box<RustType>),
    /// A trait object type: `dyn T` (e.g., `dyn Greeter`)
    ///
    /// Used with `Ref` for `&dyn Trait` parameters and with `StdCollection { kind: Box, .. }` for `Box<dyn Trait>` (I-387).
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
            RustType::TypeVar { name } => name == param,
            RustType::StdCollection { args, .. } => args.iter().any(|a| a.uses_param(param)),
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

    /// Wraps the type in `Option<T>` when `optional` is `true`, returns unchanged otherwise.
    ///
    /// Canonical encoding site for TS's `x?: T` optional parameter (and
    /// optional field) semantics. Delegates to [`Self::wrap_optional`] for
    /// idempotency (double-wrap prevention).
    ///
    /// Call this at every site that converts a TS callable-like parameter to
    /// IR, so that `Option<T>` consistently represents "caller may omit"
    /// across interface methods, class methods, constructors, callable
    /// interfaces, embedded fn types, fn type aliases, and registry method
    /// signatures. See I-040.
    pub fn wrap_if_optional(self, optional: bool) -> RustType {
        if optional {
            self.wrap_optional()
        } else {
            self
        }
    }

    /// Unwraps `Promise<T>` to `T`. Non-Promise types are returned unchanged.
    ///
    /// INV-6: Single source of truth for Promise unwrapping across the codebase.
    pub fn unwrap_promise(self) -> RustType {
        match self {
            RustType::Named {
                ref name,
                ref type_args,
            } if name == "Promise" && type_args.len() == 1 => type_args[0].clone(),
            other => other,
        }
    }

    /// Returns true if this type is `Promise<T>`.
    pub fn is_promise(&self) -> bool {
        matches!(self, RustType::Named { name, type_args } if name == "Promise" && type_args.len() == 1)
    }

    /// Structurally conservative check for whether a type implements `Copy`.
    ///
    /// Returns true only when the IR can prove `Copy` implementation purely
    /// from the type structure. Returns false for any type whose `Copy`-ness
    /// depends on external context — user-defined types (`Named`), type
    /// parameters (`TypeVar`), trait objects (`DynTrait`), and runtime-typed
    /// values (`Any` / `serde_json::Value`).
    ///
    /// Used by expression-context `??=` emission (I-142) to decide between
    /// `*x.get_or_insert_with(|| d)` (the `&mut T` deref yields `T` by copy
    /// when `T: Copy`) and `x.get_or_insert_with(|| d).clone()` (the `&mut T`
    /// is cloned to avoid a move). Picking wrong emits broken code, not a
    /// silent semantic change — Rust's compiler rejects a `*` on `&mut T` for
    /// non-`Copy` `T`. Conservative (false) is the safe default.
    pub fn is_copy_type(&self) -> bool {
        match self {
            // Primitives + unit are Copy
            RustType::Unit | RustType::F64 | RustType::Bool => true,
            RustType::Primitive(_) => true,
            // References are always Copy regardless of the pointee
            RustType::Ref(_) => true,
            // Never inhabits no values, but is Copy
            RustType::Never => true,
            // Container-like types: Copy iff all components are Copy
            RustType::Option(inner) => inner.is_copy_type(),
            RustType::Tuple(elems) => elems.iter().all(RustType::is_copy_type),
            // Not Copy: heap-allocated / interior-mutable / trait-object / generic
            RustType::String
            | RustType::Vec(_)
            | RustType::Result { .. }
            | RustType::Any
            | RustType::Fn { .. }
            | RustType::DynTrait(_)
            | RustType::QSelf { .. }
            | RustType::StdCollection { .. }
            | RustType::Named { .. }
            | RustType::TypeVar { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_promise_extracts_inner() {
        let promise = RustType::Named {
            name: "Promise".to_string(),
            type_args: vec![RustType::String],
        };
        assert_eq!(promise.unwrap_promise(), RustType::String);
    }

    #[test]
    fn unwrap_promise_passthrough_non_promise() {
        assert_eq!(RustType::F64.unwrap_promise(), RustType::F64);
        assert_eq!(RustType::String.unwrap_promise(), RustType::String);
    }

    #[test]
    fn unwrap_promise_passthrough_named_non_promise() {
        let named = RustType::Named {
            name: "MyType".to_string(),
            type_args: vec![RustType::String],
        };
        assert_eq!(named.clone().unwrap_promise(), named);
    }

    #[test]
    fn unwrap_promise_no_type_args() {
        // Promise without type args → passthrough (not a valid Promise<T>)
        let bare = RustType::Named {
            name: "Promise".to_string(),
            type_args: vec![],
        };
        assert_eq!(bare.clone().unwrap_promise(), bare);
    }

    #[test]
    fn is_promise_true_for_promise_t() {
        let promise = RustType::Named {
            name: "Promise".to_string(),
            type_args: vec![RustType::String],
        };
        assert!(promise.is_promise());
    }

    #[test]
    fn is_promise_false_for_non_promise() {
        assert!(!RustType::F64.is_promise());
        assert!(!RustType::Named {
            name: "MyType".to_string(),
            type_args: vec![RustType::String],
        }
        .is_promise());
    }

    #[test]
    fn wrap_optional_avoids_double_wrap() {
        let opt = RustType::Option(Box::new(RustType::String));
        assert_eq!(opt.clone().wrap_optional(), opt);
    }

    #[test]
    fn wrap_optional_wraps_non_option() {
        assert_eq!(
            RustType::String.wrap_optional(),
            RustType::Option(Box::new(RustType::String))
        );
    }

    #[test]
    fn wrap_if_optional_true_wraps() {
        assert_eq!(
            RustType::F64.wrap_if_optional(true),
            RustType::Option(Box::new(RustType::F64))
        );
    }

    #[test]
    fn wrap_if_optional_false_passthrough() {
        assert_eq!(RustType::F64.wrap_if_optional(false), RustType::F64);
    }

    #[test]
    fn wrap_if_optional_true_idempotent() {
        let already_opt = RustType::Option(Box::new(RustType::String));
        assert_eq!(already_opt.clone().wrap_if_optional(true), already_opt);
    }

    #[test]
    fn wrap_if_optional_preserves_inner_type_var() {
        let tv = RustType::TypeVar {
            name: "T".to_string(),
        };
        assert_eq!(
            tv.clone().wrap_if_optional(true),
            RustType::Option(Box::new(tv))
        );
    }

    // -- is_copy_type tests (I-142) --

    #[test]
    fn is_copy_type_true_for_primitives() {
        assert!(RustType::Unit.is_copy_type());
        assert!(RustType::F64.is_copy_type());
        assert!(RustType::Bool.is_copy_type());
        assert!(RustType::Never.is_copy_type());
        assert!(RustType::Primitive(PrimitiveIntKind::I32).is_copy_type());
        assert!(RustType::Primitive(PrimitiveIntKind::Usize).is_copy_type());
        assert!(RustType::Primitive(PrimitiveIntKind::F32).is_copy_type());
    }

    #[test]
    fn is_copy_type_false_for_non_copy_leaves() {
        assert!(!RustType::String.is_copy_type());
        assert!(!RustType::Any.is_copy_type());
        assert!(!RustType::Vec(Box::new(RustType::F64)).is_copy_type());
    }

    #[test]
    fn is_copy_type_ref_is_always_copy() {
        // &T is Copy regardless of T
        assert!(RustType::Ref(Box::new(RustType::String)).is_copy_type());
        assert!(RustType::Ref(Box::new(RustType::Vec(Box::new(RustType::F64)))).is_copy_type());
    }

    #[test]
    fn is_copy_type_option_delegates_to_inner() {
        assert!(RustType::Option(Box::new(RustType::F64)).is_copy_type());
        assert!(RustType::Option(Box::new(RustType::Bool)).is_copy_type());
        assert!(!RustType::Option(Box::new(RustType::String)).is_copy_type());
        assert!(!RustType::Option(Box::new(RustType::Vec(Box::new(RustType::F64)))).is_copy_type());
    }

    #[test]
    fn is_copy_type_tuple_delegates_to_all_elements() {
        assert!(RustType::Tuple(vec![RustType::F64, RustType::Bool]).is_copy_type());
        assert!(!RustType::Tuple(vec![RustType::F64, RustType::String]).is_copy_type());
        // Empty tuple is `()` which is Copy
        assert!(RustType::Tuple(vec![]).is_copy_type());
    }

    #[test]
    fn is_copy_type_false_for_generics_and_trait_objects() {
        // User-defined types: cannot prove Copy structurally
        assert!(!RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        }
        .is_copy_type());
        // TypeVar: depends on external bound
        assert!(!RustType::TypeVar {
            name: "T".to_string(),
        }
        .is_copy_type());
        // DynTrait: trait objects are !Sized → !Copy
        assert!(!RustType::DynTrait("Greeter".to_string()).is_copy_type());
    }

    #[test]
    fn is_copy_type_false_for_fn_and_result_and_std_collections() {
        assert!(!RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::Unit),
        }
        .is_copy_type());
        assert!(!RustType::Result {
            ok: Box::new(RustType::F64),
            err: Box::new(RustType::String),
        }
        .is_copy_type());
        assert!(!RustType::StdCollection {
            kind: StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        }
        .is_copy_type());
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
