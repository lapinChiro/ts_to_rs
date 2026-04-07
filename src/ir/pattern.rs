//! 構造化された Rust pattern IR ノード。
//!
//! `MatchArm::patterns` および `Stmt::IfLet` / `Stmt::WhileLet` /
//! `Expr::IfLet` / `Expr::Matches` の `pattern` フィールドで使用される。
//!
//! # 設計方針
//!
//! - IR は **構造化データ** のみを保持し、display-formatted 文字列は保存しない
//!   (`.claude/rules/pipeline-integrity.md`)。文字列化は generator の
//!   `render_pattern` の責務
//! - I-380 で `TupleStruct` / `Struct` / `UnitStruct` の `path: Vec<String>` を
//!   構造化 [`PatternCtor`] に置換した。これにより walker は variant 形状から
//!   user type 参照を構造的に判定でき、`PATTERN_LANG_BUILTINS` ハードコード除外
//!   リストが不要になった
//! - `UnitStruct` と `TupleStruct { fields: vec![] }` は区別する:
//!   前者 → `None`、後者 → `None()` と rendering 差が明示される
//! - `Binding::subpat` は `x @ 1..=5` 等のサブパターン束縛用

use super::{BuiltinVariant, Expr, UserTypeRef};

/// Pattern の constructor 種別を構造化する。
///
/// I-380 で I-377 の暫定形 `path: Vec<String>` を分解した結果。各 variant は
/// 単一の意味論を担い、walker は variant 形状から user type 参照を構造的に
/// 判定できる ([`crate::ir::visit::walk_pattern_ctor`] を参照)。
///
/// `CallTarget` の対応する分類との対称性:
/// - `Builtin(BuiltinVariant)` ↔ `CallTarget::BuiltinVariant(_)`
/// - `UserEnumVariant { enum_ty, variant }` ↔ `CallTarget::UserEnumVariantCtor { .. }`
/// - `UserStruct(ty)` ↔ `CallTarget::UserTupleCtor(_)`
#[derive(Debug, Clone, PartialEq)]
pub enum PatternCtor {
    /// `Some(_)` / `None` / `Ok(_)` / `Err(_)` — Option/Result builtin variant。
    /// walker: 何もしない (builtin であることが型で保証される)。
    Builtin(BuiltinVariant),

    /// `Color::Red(_)` / `Shape::Circle { .. }` — ユーザー定義 enum の variant constructor。
    /// walker: `enum_ty` を user type ref として登録。
    UserEnumVariant {
        /// 親 enum 型への参照。
        enum_ty: UserTypeRef,
        /// variant 名 (単一識別子)。
        variant: String,
    },

    /// `Foo { .. }` / `Wrapper(_)` — ユーザー定義 struct (tuple struct or named struct) パターン。
    /// walker: `0` (型) を user type ref として登録。
    UserStruct(UserTypeRef),
}

impl PatternCtor {
    /// builtin `None` constructor かどうか。
    fn is_builtin_none(&self) -> bool {
        matches!(self, PatternCtor::Builtin(BuiltinVariant::None))
    }
}

/// Rust pattern grammar を構造化表現した IR ノード。
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_` — ワイルドカード。
    Wildcard,

    /// 値リテラルパターン (`1`, `"hello"`, `true`, `3.14`)。
    ///
    /// # 不変条件
    ///
    /// `Literal` は **純粋な値リテラル** (`Expr::IntLit` / `Expr::NumberLit` /
    /// `Expr::StringLit` / `Expr::BoolLit`) のみを保持する。enum variant 名や
    /// 修飾パス (`Direction::Up`, `Color::Red`) は `UnitStruct` / `TupleStruct`
    /// で表現すること。
    Literal(Expr),

    /// 変数束縛 (`x`, `mut x`, `x @ 1..=5`)。
    Binding {
        /// 束縛する変数名
        name: String,
        /// `mut` 修飾子の有無
        is_mut: bool,
        /// `@` サブパターン (`x @ Foo(_)` 等)
        subpat: Option<Box<Pattern>>,
    },

    /// タプル構造体 / タプル variant (`Some(x)`, `Color::Red(x, y)`, `Ok(v)`, `Err(e)`)。
    TupleStruct {
        /// 構造化された constructor 種別。
        ctor: PatternCtor,
        /// タプル要素のサブパターン
        fields: Vec<Pattern>,
    },

    /// 構造体 / struct variant (`Shape::Circle { radius, .. }`, `Foo { x, y }`)。
    Struct {
        /// 構造化された constructor 種別。
        ctor: PatternCtor,
        /// 名前付きフィールドとサブパターンの対
        fields: Vec<(String, Pattern)>,
        /// 末尾の `..` 有無
        rest: bool,
    },

    /// Unit variant / unit struct (`None`, `Color::Green`)。
    UnitStruct {
        /// 構造化された constructor 種別。
        ctor: PatternCtor,
    },

    /// Or パターン (`a | b | c`)。
    Or(Vec<Pattern>),

    /// Range パターン (`1..=5`, `..10`)。
    Range {
        /// 範囲開始 (`None` = 開始省略)
        start: Option<Box<Expr>>,
        /// 範囲終了 (`None` = 終了省略)
        end: Option<Box<Expr>>,
        /// `..=` (inclusive) なら `true`、`..` (exclusive) なら `false`
        inclusive: bool,
    },

    /// 参照パターン (`&x`, `&mut x`)。
    Ref {
        /// `&mut` なら `true`
        mutable: bool,
        /// 参照されるサブパターン
        inner: Box<Pattern>,
    },

    /// タプルパターン (`(a, b, c)`)。
    Tuple(Vec<Pattern>),
}

impl Pattern {
    /// Unit variant `None` パターンかどうかを構造的に判定する。
    ///
    /// I-380 で `path[0] == "None"` 文字列比較を [`PatternCtor::Builtin`] の
    /// 構造的判定に置き換えた。
    pub fn is_none_unit(&self) -> bool {
        matches!(self, Pattern::UnitStruct { ctor } if ctor.is_builtin_none())
    }

    /// 単一セグメントの `Binding` ショートカット (`mut` なし、subpat なし)。
    pub fn binding(name: impl Into<String>) -> Pattern {
        Pattern::Binding {
            name: name.into(),
            is_mut: false,
            subpat: None,
        }
    }

    /// `Some(binding_name)` パターン構築ショートカット。
    pub fn some_binding(name: impl Into<String>) -> Pattern {
        Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::Some),
            fields: vec![Pattern::binding(name)],
        }
    }

    /// `None` パターン構築ショートカット。
    pub fn none() -> Pattern {
        Pattern::UnitStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_none_unit ---

    #[test]
    fn is_none_unit_true_for_none_pattern() {
        assert!(Pattern::none().is_none_unit());
    }

    #[test]
    fn is_none_unit_false_for_user_unit_struct() {
        let pat = Pattern::UnitStruct {
            ctor: PatternCtor::UserStruct(UserTypeRef::new("Empty")),
        };
        assert!(!pat.is_none_unit());
    }

    #[test]
    fn is_none_unit_false_for_other_builtin() {
        let pat = Pattern::UnitStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::Some),
        };
        assert!(!pat.is_none_unit());
    }

    #[test]
    fn is_none_unit_false_for_tuple_struct_none_shaped() {
        // `None()` (tuple-struct) is not the bare `None` unit pattern.
        let pat = Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::None),
            fields: vec![],
        };
        assert!(!pat.is_none_unit());
    }

    #[test]
    fn is_none_unit_false_for_wildcard() {
        assert!(!Pattern::Wildcard.is_none_unit());
    }

    #[test]
    fn is_none_unit_false_for_binding() {
        assert!(!Pattern::binding("x").is_none_unit());
    }

    // --- binding ---

    #[test]
    fn binding_creates_plain_name_binding() {
        assert_eq!(
            Pattern::binding("foo"),
            Pattern::Binding {
                name: "foo".to_string(),
                is_mut: false,
                subpat: None,
            }
        );
    }

    #[test]
    fn binding_accepts_impl_into_string() {
        let a = Pattern::binding("x");
        let b = Pattern::binding(String::from("x"));
        assert_eq!(a, b);
    }

    // --- some_binding ---

    #[test]
    fn some_binding_wraps_in_some_tuple_struct() {
        assert_eq!(
            Pattern::some_binding("v"),
            Pattern::TupleStruct {
                ctor: PatternCtor::Builtin(BuiltinVariant::Some),
                fields: vec![Pattern::Binding {
                    name: "v".to_string(),
                    is_mut: false,
                    subpat: None,
                }],
            }
        );
    }

    #[test]
    fn some_binding_is_not_none_unit() {
        assert!(!Pattern::some_binding("x").is_none_unit());
    }

    // --- none ---

    #[test]
    fn none_creates_builtin_none_unit_struct() {
        assert_eq!(
            Pattern::none(),
            Pattern::UnitStruct {
                ctor: PatternCtor::Builtin(BuiltinVariant::None),
            }
        );
    }

    #[test]
    fn none_round_trips_through_is_none_unit() {
        assert!(Pattern::none().is_none_unit());
    }

    // --- PatternCtor ---

    #[test]
    fn pattern_ctor_builtin_construction() {
        let _ = PatternCtor::Builtin(BuiltinVariant::Some);
        let _ = PatternCtor::Builtin(BuiltinVariant::None);
        let _ = PatternCtor::Builtin(BuiltinVariant::Ok);
        let _ = PatternCtor::Builtin(BuiltinVariant::Err);
    }

    #[test]
    fn pattern_ctor_user_enum_variant_construction() {
        let c = PatternCtor::UserEnumVariant {
            enum_ty: UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        };
        if let PatternCtor::UserEnumVariant { enum_ty, variant } = c {
            assert_eq!(enum_ty.as_str(), "Color");
            assert_eq!(variant, "Red");
        } else {
            panic!();
        }
    }

    #[test]
    fn pattern_ctor_user_struct_construction() {
        let c = PatternCtor::UserStruct(UserTypeRef::new("Foo"));
        if let PatternCtor::UserStruct(ty) = c {
            assert_eq!(ty.as_str(), "Foo");
        } else {
            panic!();
        }
    }

    #[test]
    fn pattern_ctor_eq_round_trip() {
        let a = PatternCtor::UserEnumVariant {
            enum_ty: UserTypeRef::new("E"),
            variant: "V".to_string(),
        };
        let b = PatternCtor::UserEnumVariant {
            enum_ty: UserTypeRef::new("E"),
            variant: "V".to_string(),
        };
        assert_eq!(a, b);
    }
}
