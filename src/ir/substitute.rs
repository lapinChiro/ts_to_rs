//! Type-parameter substitution on IR nodes via [`IrFolder`].
//!
//! `substitute` replaces type-parameter references (`RustType::Named { name: "T", .. }`)
//! with concrete types from a binding map, used during monomorphization. I-377 以前は
//! 各 IR 型ごとに手書きの `substitute` メソッドが実装されており、新しい variant が
//! 追加されるたびに更新漏れが発生するリスクがあった。現在の実装は `IrFolder` 骨格
//! （`src/ir/fold.rs`）に委譲し、変換ロジックは `Substitute::fold_rust_type` の 1 箇所
//! に集約されている。走査自体は `walk_*` 関数が担当するため、新 variant を追加した
//! 際も `walk_*` の 1 箇所を更新するだけで全 substitute 呼び出しに反映される。
//!
//! 公開 API として提供している `impl X { pub fn substitute(&self, ..) -> X }`
//! メソッド群は既存呼び出し側（registry / transformer）との互換維持のため残しており、
//! 各実装は `Substitute` folder に委譲する薄いラッパーとなっている。

use super::fold::{walk_rust_type, IrFolder};
use super::*;
use std::collections::HashMap;

/// 型パラメータ置換用の [`IrFolder`] 実装。
///
/// `RustType::Named { name: "T", type_args: [] }` が `bindings` に存在する場合、
/// 対応する具体型で置換する。その他の variant は `walk_rust_type` に委譲し、
/// 子ノード（`Option<T>` / `Vec<T>` / `Result<T, E>` / `Fn(T) -> U` / `QSelf` 等）
/// を再帰的に fold する。
///
/// パターン内のリテラル式（`Pattern::Literal(Expr::Cast { target: T })` 等）も
/// `walk_pattern` → `fold_expr` → `walk_expr` の経路で正しく走査され、
/// 型パラメータが置換される。
pub(crate) struct Substitute<'a> {
    pub bindings: &'a HashMap<String, RustType>,
}

impl<'a> IrFolder for Substitute<'a> {
    fn fold_rust_type(&mut self, ty: RustType) -> RustType {
        if let RustType::Named { name, type_args } = &ty {
            if type_args.is_empty() {
                if let Some(concrete) = self.bindings.get(name.as_str()) {
                    return concrete.clone();
                }
            }
        }
        walk_rust_type(self, ty)
    }
}

/// `Substitute` folder を使ってノードを置換するヘルパー。
fn fold_with<'a, T, F: FnOnce(&mut Substitute<'a>) -> T>(
    bindings: &'a HashMap<String, RustType>,
    f: F,
) -> T {
    let mut folder = Substitute { bindings };
    f(&mut folder)
}

impl TypeParam {
    /// 型パラメータの制約内の型パラメータ参照を具体型で置換した新しい `TypeParam` を返す。
    ///
    /// `name` は型パラメータの識別子であり置換対象ではない。`constraint` 内の
    /// `RustType` のみ置換する。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> TypeParam {
        fold_with(bindings, |f| f.fold_type_param(self.clone()))
    }
}

impl RustType {
    /// 型パラメータ名を具体型に置換する。
    ///
    /// `bindings` は型パラメータ名 → 具体型のマッピング。
    /// `Named { name: "T" }` が `bindings` に存在すれば具体型に置換し、
    /// それ以外のバリアントは再帰的に処理する。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> RustType {
        fold_with(bindings, |f| f.fold_rust_type(self.clone()))
    }
}

impl StructField {
    /// 型パラメータを具体型で置換した新しい `StructField` を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> StructField {
        fold_with(bindings, |f| f.fold_struct_field(self.clone()))
    }
}

impl Param {
    /// 型パラメータを具体型で置換した新しい `Param` を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Param {
        fold_with(bindings, |f| f.fold_param(self.clone()))
    }
}

impl Method {
    /// 型パラメータを具体型で置換した新しい `Method` を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Method {
        fold_with(bindings, |f| f.fold_method(self.clone()))
    }
}

impl Stmt {
    /// 型パラメータを具体型で置換した新しい `Stmt` を返す。
    ///
    /// `RustType` を含むバリアント（`Let::ty` など）を置換する。パターンに含まれる
    /// `Expr`（`Pattern::Literal(Expr::Cast { target: T })` 等）も `walk_*` 経由で
    /// 正しく置換される。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Stmt {
        fold_with(bindings, |f| f.fold_stmt(self.clone()))
    }
}

impl Expr {
    /// 型パラメータを具体型で置換した新しい `Expr` を返す。
    ///
    /// `RustType` を含むバリアント（`Closure::return_type`, `Cast::target`）を置換
    /// する。`CallTarget` の `segments` / `type_ref` はプレーン識別子であり置換対象
    /// ではない（`walk_expr` 内の `fold_call_target` で恒等変換される）。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Expr {
        fold_with(bindings, |f| f.fold_expr(self.clone()))
    }
}

impl Item {
    /// 型パラメータを具体型で置換した新しい `Item` を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> Item {
        fold_with(bindings, |f| f.fold_item(self.clone()))
    }
}

impl MatchArm {
    /// 型パラメータを具体型で置換した新しい `MatchArm` を返す。
    pub fn substitute(&self, bindings: &HashMap<String, RustType>) -> MatchArm {
        fold_with(bindings, |f| f.fold_match_arm(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::CallTarget;

    /// `Expr::substitute` must preserve the structured `CallTarget` untouched:
    /// the `segments` and `type_ref` fields are plain identifiers and are not
    /// type-parameter substitution targets. Only the arguments should be walked.
    #[test]
    fn test_substitute_fn_call_preserves_call_target_and_substitutes_args() {
        let bindings: HashMap<String, RustType> = [(
            "T".to_string(),
            RustType::Named {
                name: "Concrete".to_string(),
                type_args: vec![],
            },
        )]
        .into_iter()
        .collect();

        // FnCall with a 2-segment assoc path and an arg that references `T`
        // inside a `Cast { target: T }`. The cast's type should be substituted
        // but the call target must remain intact.
        let expr = Expr::FnCall {
            target: CallTarget::assoc("Wrapper", "new"),
            args: vec![Expr::Cast {
                expr: Box::new(Expr::Ident("x".to_string())),
                target: RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            }],
        };

        let substituted = expr.substitute(&bindings);
        match substituted {
            Expr::FnCall { target, args } => {
                // target must be exactly the same — identifiers are not types
                assert_eq!(target, CallTarget::assoc("Wrapper", "new"));
                // args[0] should have its `T` replaced with `Concrete`
                match &args[0] {
                    Expr::Cast { target: ty, .. } => {
                        assert_eq!(
                            ty,
                            &RustType::Named {
                                name: "Concrete".to_string(),
                                type_args: vec![]
                            }
                        );
                    }
                    other => panic!("expected Cast, got {other:?}"),
                }
            }
            other => panic!("expected FnCall, got {other:?}"),
        }
    }

    /// `CallTarget::Path { type_ref: Some("T"), .. }` is an identifier string,
    /// not a `RustType`, so it MUST remain as `"T"` even when `bindings` maps
    /// `T` to a concrete type. The substitution only applies to `RustType::Named`.
    #[test]
    fn test_substitute_preserves_call_target_type_ref_string() {
        let bindings: HashMap<String, RustType> = [(
            "T".to_string(),
            RustType::Named {
                name: "Concrete".to_string(),
                type_args: vec![],
            },
        )]
        .into_iter()
        .collect();

        let expr = Expr::FnCall {
            target: CallTarget::Path {
                segments: vec!["T".to_string(), "new".to_string()],
                type_ref: Some("T".to_string()),
            },
            args: vec![],
        };

        let substituted = expr.substitute(&bindings);
        match substituted {
            Expr::FnCall { target, .. } => {
                // `T` stays as the identifier — it's not a RustType in this context.
                assert_eq!(
                    target,
                    CallTarget::Path {
                        segments: vec!["T".to_string(), "new".to_string()],
                        type_ref: Some("T".to_string()),
                    }
                );
            }
            _ => panic!("expected FnCall"),
        }
    }

    /// `CallTarget::Super` must round-trip through `substitute` unchanged.
    #[test]
    fn test_substitute_preserves_super_call_target() {
        let bindings: HashMap<String, RustType> = HashMap::new();
        let expr = Expr::FnCall {
            target: CallTarget::Super,
            args: vec![Expr::Ident("x".to_string())],
        };
        let substituted = expr.substitute(&bindings);
        assert!(
            matches!(
                &substituted,
                Expr::FnCall {
                    target: CallTarget::Super,
                    ..
                }
            ),
            "expected Super target preserved, got {substituted:?}"
        );
    }

    /// 型パラメータ置換が `MatchArm` 内の `Pattern::Literal(Expr::Cast { target: T })`
    /// にも正しく伝播することを確認する。I-377 以前の手書き substitute では pattern
    /// 内の Expr は走査されていなかった。本 IrFolder ベース実装では `walk_pattern` →
    /// `fold_expr` → `walk_expr` の経路で substitute が届く。
    #[test]
    fn test_substitute_walks_into_pattern_literal_expr() {
        let bindings: HashMap<String, RustType> = [(
            "T".to_string(),
            RustType::Named {
                name: "Concrete".to_string(),
                type_args: vec![],
            },
        )]
        .into_iter()
        .collect();

        let arm = MatchArm {
            patterns: vec![Pattern::Literal(Expr::Cast {
                expr: Box::new(Expr::IntLit(1)),
                target: RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            })],
            guard: None,
            body: vec![],
        };

        let substituted = arm.substitute(&bindings);
        match &substituted.patterns[0] {
            Pattern::Literal(Expr::Cast { target, .. }) => {
                assert_eq!(
                    target,
                    &RustType::Named {
                        name: "Concrete".to_string(),
                        type_args: vec![]
                    }
                );
            }
            other => panic!("expected Literal(Cast), got {other:?}"),
        }
    }

    /// 型パラメータ置換が `Stmt::IfLet` 内の `Pattern::TupleStruct` → `Pattern::Literal(Expr::Cast)`
    /// のようにネストした箇所にも伝播することを確認する。
    #[test]
    fn test_substitute_walks_into_nested_iflet_pattern() {
        let bindings: HashMap<String, RustType> = [(
            "T".to_string(),
            RustType::Named {
                name: "Concrete".to_string(),
                type_args: vec![],
            },
        )]
        .into_iter()
        .collect();

        let stmt = Stmt::IfLet {
            pattern: Pattern::TupleStruct {
                path: vec!["Some".to_string()],
                fields: vec![Pattern::Literal(Expr::Cast {
                    expr: Box::new(Expr::IntLit(1)),
                    target: RustType::Named {
                        name: "T".to_string(),
                        type_args: vec![],
                    },
                })],
            },
            expr: Expr::Ident("x".to_string()),
            then_body: vec![],
            else_body: None,
        };

        let substituted = stmt.substitute(&bindings);
        if let Stmt::IfLet {
            pattern: Pattern::TupleStruct { fields, .. },
            ..
        } = substituted
        {
            if let Pattern::Literal(Expr::Cast { target, .. }) = &fields[0] {
                assert_eq!(
                    target,
                    &RustType::Named {
                        name: "Concrete".to_string(),
                        type_args: vec![]
                    }
                );
            } else {
                panic!("expected nested Literal(Cast)");
            }
        } else {
            panic!("expected IfLet with TupleStruct pattern");
        }
    }
}
