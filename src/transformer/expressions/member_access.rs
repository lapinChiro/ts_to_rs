//! Member access expression conversion (property access, optional chaining, discriminated unions).
//!
//! Class member dispatch logic (Static / Instance / Fallback receiver classification +
//! Read/Write context dispatch arms) lives in the sibling [`super::member_dispatch`] module
//! to centralize the cross-cutting receiver-detection knowledge shared by Read context
//! ([`Transformer::resolve_member_access`]) and Write context
//! ([`Transformer::dispatch_member_write`]) (and subsequent T7-T9 compound dispatch).

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{ClosureBody, Expr, MatchArm, Param, Pattern, RustType, Stmt};
use crate::pipeline::type_resolution::Span;
use crate::registry::{FieldDef, TypeDef};

use super::member_dispatch::{
    dispatch_instance_member_read, dispatch_static_member_read, MemberReceiverClassification,
};
use super::methods::map_method_call;
use crate::transformer::Transformer;

/// Converts an index expression to a usize-compatible form for `Vec::get()`.
///
/// Integer-valued `NumberLit` → `IntLit` (renders as `0`, not `0.0`).
/// Other expressions → `Cast { target: usize }`.
pub(crate) fn convert_index_to_usize(index: Expr) -> Expr {
    match &index {
        Expr::NumberLit(n) if n.fract() == 0.0 => Expr::IntLit(*n as i128),
        _ => Expr::Cast {
            expr: Box::new(index),
            // I-387: `usize` は `Primitive` variant で構造化
            target: RustType::Primitive(crate::ir::PrimitiveIntKind::Usize),
        },
    }
}

/// Builds a safe index expression: `object.get(index).cloned()`.
///
/// The `index` argument must already be converted via [`convert_index_to_usize`].
/// Returns `Option<T>` by value (bounded out-of-bounds). Used in Option<T>
/// expected contexts (return, assignment, call arg, ternary branch, optional
/// chaining) where the caller unifies with `Option<T>` directly.
///
/// The emitted pattern is recognized by `produces_option_result` so that the
/// surrounding `convert_expr_with_expected` skips wrapping in `Some(...)`.
pub(crate) fn build_safe_index_expr(object: Expr, index: Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(Expr::MethodCall {
            object: Box::new(object),
            method: "get".to_string(),
            args: vec![index],
        }),
        method: "cloned".to_string(),
        args: vec![],
    }
}

/// Builds a safe index expression with unwrap: `object.get(index).cloned().unwrap()`.
///
/// Used in read contexts where `T` (not `Option<T>`) is expected. The
/// `.get().cloned()` prefix retains bounded indexing semantics; `.unwrap()`
/// then panics on out-of-bounds access, matching the TS `arr[i]` runtime
/// TypeError when used as a non-nullable `T`.
///
/// Call sites are selected by [`Transformer::convert_member_expr`] based on the
/// span's expected type (Option<T> → [`build_safe_index_expr`], otherwise this
/// helper).
pub(crate) fn build_safe_index_expr_unwrapped(object: Expr, index: Expr) -> Expr {
    Expr::MethodCall {
        object: Box::new(build_safe_index_expr(object, index)),
        method: "unwrap".to_string(),
        args: vec![],
    }
}

/// Extracts the Rust-side field name from a non-computed [`ast::MemberProp`].
///
/// Returns the field name string used to look up class members in `TypeRegistry` and to
/// emit Rust identifiers in IR (`Expr::FieldAccess { field }`、`Expr::MethodCall { method }`、
/// etc.):
///
/// - [`ast::MemberProp::Ident`]`("foo")` → `"foo"` (TS regular identifier)
/// - [`ast::MemberProp::PrivateName`]`("foo")` → `"_foo"` (`#foo` ECMA private name → `_foo`
///   Rust convention、collect_class_info で `_` prefix-strip された field 名と同期)
/// - [`ast::MemberProp::Computed`]`(_)` → `None` (= runtime-evaluated key、class member
///   dispatch / static field name resolution の対象外、caller side で context-specific
///   handling が必要)
///
/// I-205 T6 Iteration v10 third-review (DRY refactor、`design-integrity.md` "DRY"): pre-extract
/// は `convert_member_expr_inner` (Read path、Computed 早期 return 後の `match` block) と
/// `dispatch_member_write` (Write path、`unreachable!()` macro 経由) で同 logic が重複していた
/// ため共通 helper に集約。Caller は `Option::ok_or_else` (Read = Err return) / `unwrap_or_else`
/// (Write = `unreachable!()` macro) で context-specific fallback を実行。
pub(super) fn extract_non_computed_field_name(prop: &ast::MemberProp) -> Option<String> {
    match prop {
        ast::MemberProp::Ident(ident) => Some(ident.sym.to_string()),
        ast::MemberProp::PrivateName(private) => Some(format!("_{}", private.name)),
        ast::MemberProp::Computed(_) => None,
    }
}

impl<'a> Transformer<'a> {
    /// Resolves a member access expression, applying special conversions for known fields.
    ///
    /// Read context dispatch order (I-205 T5、Iteration v9):
    /// 1. Enum variant access → `EnumName::Variant`
    /// 2. `Math.PI`, `Math.E` → `std::f64::consts::*`
    /// 3. `.length` → `.len() as f64`
    /// 4. **Class member dispatch (T5 拡張)**:
    ///    - Static (B8): receiver = `Ident(class_name)` with class registration →
    ///      `Class::field()` (Getter dispatch) / Tier 2 honest error (read-only / method)
    ///    - Instance (B1-B4, B6-B7, B9): `get_expr_type(ts_obj)` = `RustType::Named { name, .. }` →
    ///      `lookup_method_sigs_in_inheritance_chain` で getter/setter/method/inherited を判別、
    ///      Tier 1 dispatch (Getter → MethodCall) または Tier 2 honest error (read-only / method
    ///      / inherited) を emit、それ以外 (B1 field, B9 unknown) → 5. fallback FieldAccess
    /// 5. Fallback: `object.field` (FieldAccess、B1 / B9 / non-class receiver)
    pub(crate) fn resolve_member_access(
        &self,
        object: &Expr,
        field: &str,
        ts_obj: &ast::Expr,
    ) -> Result<Expr> {
        // 1. Enum variant access
        if let ast::Expr::Ident(ident) = ts_obj {
            let name = ident.sym.as_ref();
            if let Some(TypeDef::Enum { .. }) = self.reg().get(name) {
                return Ok(Expr::EnumVariant {
                    enum_ty: crate::ir::UserTypeRef::new(name),
                    variant: field.to_string(),
                });
            }
        }

        // 2. Math.PI, Math.E etc.
        if let ast::Expr::Ident(ident) = ts_obj {
            if ident.sym.as_ref() == "Math" {
                if let Some(c) = crate::ir::StdConst::from_math_member(field) {
                    return Ok(Expr::StdConst(c));
                }
            }
        }

        // 3. .length → .len() as f64
        if field == "length" {
            let len_call = Expr::MethodCall {
                object: Box::new(object.clone()),
                method: "len".to_string(),
                args: vec![],
            };
            return Ok(Expr::Cast {
                expr: Box::new(len_call),
                target: RustType::F64,
            });
        }

        // 4. Class member dispatch (I-205 T5、Iteration v10 で `classify_member_receiver`
        //    shared helper 経由に refactor、T6 dispatch_member_write と DRY 解消、
        //    Iteration v10 third-review で member_dispatch.rs に file split)
        match self.classify_member_receiver(ts_obj, field) {
            MemberReceiverClassification::Static {
                class_name,
                sigs,
                is_inherited,
            } => {
                return dispatch_static_member_read(
                    &class_name,
                    field,
                    &sigs,
                    is_inherited,
                    ts_obj,
                );
            }
            MemberReceiverClassification::Instance { sigs, is_inherited } => {
                return dispatch_instance_member_read(object, field, &sigs, is_inherited, ts_obj);
            }
            MemberReceiverClassification::Fallback => {
                // 5. Fallback: direct field access (B1 / B9 / non-class receiver / static field)
            }
        }

        Ok(Expr::FieldAccess {
            object: Box::new(object.clone()),
            field: field.to_string(),
        })
    }

    /// Converts an optional chaining expression (`x?.y`) to `x.as_ref().map(|_v| _v.y)`.
    ///
    /// Supports property access, method calls, and computed access.
    /// Chained optional chaining (`x?.y?.z`) is handled recursively.
    pub(crate) fn convert_opt_chain_expr(&mut self, opt_chain: &ast::OptChainExpr) -> Result<Expr> {
        match opt_chain.base.as_ref() {
            ast::OptChainBase::Member(member) => {
                let obj_type = self.get_expr_type(&member.obj);
                let is_option = obj_type.is_some_and(|ty| matches!(ty, RustType::Option(_)));

                // Non-Option type with known type: plain member access
                if !is_option && obj_type.is_some() {
                    return self.convert_member_expr(member);
                }

                // Cat A: receiver object for optional chaining
                let object = self.convert_expr(&member.obj)?;
                let body_expr = match &member.prop {
                    ast::MemberProp::Ident(ident) => {
                        let field = ident.sym.to_string();
                        self.resolve_member_access(
                            &Expr::Ident("_v".to_string()),
                            &field,
                            &member.obj,
                        )?
                    }
                    ast::MemberProp::Computed(computed) => {
                        // Use .get() for safe bounds-checked access (I-316).
                        // Direct indexing (_v[i]) panics on out-of-bounds;
                        // TS returns undefined, which maps to None.
                        //
                        // I-138: `Option<Vec<Option<T>>>` via `obj?.[i]` produces
                        // `Option<Option<T>>` at the `and_then` output level, which
                        // mismatches an `Option<T>` expected context. Append
                        // `.flatten()` to the closure body when the Vec's element
                        // type is itself `Option<X>` matching the outer expected
                        // inner type — the same invariant as `convert_member_expr_inner`'s
                        // T4 branch, applied at the optional-chaining emission site.
                        let index = self.convert_expr(&computed.expr)?;
                        let safe_index = convert_index_to_usize(index);
                        let base = build_safe_index_expr(Expr::Ident("_v".to_string()), safe_index);
                        let opt_chain_expected = self
                            .tctx
                            .type_resolution
                            .expected_type(Span::from_swc(opt_chain.span));
                        let elem_ty = obj_type.and_then(|t| match t {
                            RustType::Option(inner) => match inner.as_ref() {
                                RustType::Vec(v) => Some(v.as_ref()),
                                _ => None,
                            },
                            _ => None,
                        });
                        let needs_flatten = matches!(
                            (opt_chain_expected, elem_ty),
                            (
                                Some(RustType::Option(exp_inner)),
                                Some(RustType::Option(elem_inner)),
                            ) if elem_inner.as_ref() == exp_inner.as_ref()
                        );
                        if needs_flatten {
                            Expr::MethodCall {
                                object: Box::new(base),
                                method: "flatten".to_string(),
                                args: vec![],
                            }
                        } else {
                            base
                        }
                    }
                    _ => return Err(anyhow!("unsupported optional chaining property")),
                };

                // Use and_then when the body returns Option (to avoid Option<Option<T>>):
                // - Computed index: .get() returns Option<&T>
                // - Option field type: field is already Option<T>
                let is_computed = matches!(&member.prop, ast::MemberProp::Computed(_));
                let field_type =
                    self.resolve_field_type(obj_type.unwrap_or(&RustType::Any), &member.prop);
                let method_name = if is_computed
                    || field_type.is_some_and(|ty| matches!(ty, RustType::Option(_)))
                {
                    "and_then"
                } else {
                    "map"
                };

                Ok(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(object),
                        method: "as_ref".to_string(),
                        args: vec![],
                    }),
                    method: method_name.to_string(),
                    args: vec![Expr::Closure {
                        params: vec![Param {
                            name: "_v".to_string(),
                            ty: None,
                        }],
                        return_type: None,
                        body: ClosureBody::Expr(Box::new(body_expr)),
                    }],
                })
            }
            ast::OptChainBase::Call(opt_call) => {
                // Check if the callee object is a non-Option type
                let callee_obj_type = match opt_call.callee.as_ref() {
                    ast::Expr::Member(m) => self.get_expr_type(&m.obj),
                    ast::Expr::OptChain(oc) => match oc.base.as_ref() {
                        ast::OptChainBase::Member(m) => self.get_expr_type(&m.obj),
                        _ => None,
                    },
                    _ => None,
                };
                let is_option = callee_obj_type.is_some_and(|ty| matches!(ty, RustType::Option(_)));

                let (object, method) = self.extract_method_from_callee(&opt_call.callee)?;

                let args: Vec<Expr> = opt_call
                    .args
                    .iter()
                    .map(|arg| self.convert_expr(&arg.expr))
                    .collect::<Result<_>>()?;

                // Non-Option type: plain method call
                if !is_option && callee_obj_type.is_some() {
                    return Ok(Expr::MethodCall {
                        object: Box::new(object),
                        method,
                        args,
                    });
                }

                let body_expr = map_method_call(Expr::Ident("_v".to_string()), &method, args);
                Ok(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(object),
                        method: "as_ref".to_string(),
                        args: vec![],
                    }),
                    method: "map".to_string(),
                    args: vec![Expr::Closure {
                        params: vec![Param {
                            name: "_v".to_string(),
                            ty: None,
                        }],
                        return_type: None,
                        body: ClosureBody::Expr(Box::new(body_expr)),
                    }],
                })
            }
        }
    }

    /// Converts a member expression (`obj.field`) for read access.
    ///
    /// Vec index reads use `.get(idx).cloned()` for safe bounds checking (I-319).
    /// `this.x` becomes `self.x`.
    pub(crate) fn convert_member_expr(&mut self, member: &ast::MemberExpr) -> Result<Expr> {
        self.convert_member_expr_inner(member, false)
    }

    /// Converts a member expression for write access (assignment target).
    ///
    /// Always uses direct indexing (`arr[idx]`) for assignment LHS.
    pub(crate) fn convert_member_expr_for_write(
        &mut self,
        member: &ast::MemberExpr,
    ) -> Result<Expr> {
        self.convert_member_expr_inner(member, true)
    }

    /// Inner implementation for member expression conversion.
    ///
    /// When `for_write` is false (read), Vec index access uses `.get(idx).cloned()`.
    /// When `for_write` is true (assignment target), direct indexing is used.
    fn convert_member_expr_inner(
        &mut self,
        member: &ast::MemberExpr,
        for_write: bool,
    ) -> Result<Expr> {
        // Computed property: arr[0], arr[i] → safe get or Expr::Index
        if let ast::MemberProp::Computed(computed) = &member.prop {
            // Cat A: receiver object
            let object = self.convert_expr(&member.obj)?;

            // Tuple index access: pair[0] → pair.0 (Rust uses dot notation for tuples)
            if let Some(RustType::Tuple(_)) = self.get_expr_type(&member.obj) {
                if let ast::Expr::Lit(ast::Lit::Num(num)) = &*computed.expr {
                    let idx = num.value as usize;
                    return Ok(Expr::FieldAccess {
                        object: Box::new(object),
                        field: idx.to_string(),
                    });
                }
            }

            // Cat A: computed index
            let index = self.convert_expr(&computed.expr)?;

            // Range index (slice): always direct access
            if matches!(index, Expr::Range { .. }) || for_write {
                return Ok(Expr::Index {
                    object: Box::new(object),
                    index: Box::new(index),
                });
            }

            // Read access: safe bounds-checked indexing (I-319, I-138, I-022).
            //
            // When the enclosing context expects `Option<T>` (return, assignment,
            // call arg, ternary branch — all propagated by TypeResolver), emit
            // `arr.get(i).cloned()` directly. Otherwise emit the `.unwrap()` form
            // so the output type matches the `T` expected by the caller. This
            // single expected-type query is the sole context-aware branch in
            // Vec index emission; outer `Some(...)` wrapping is skipped by
            // `produces_option_result` detecting the `.get().cloned()` pattern.
            //
            // `Vec<Option<T>>` + expected `Option<T>` edge case: `.get(i).cloned()`
            // would yield `Option<Option<T>>`. Append `.flatten()` only when the
            // produced type is exactly one `Option` level deeper than expected —
            // i.e., `elem_ty == Option<expected_inner>`. This preserves valid
            // `Option<Option<T>>`-expected cases (no flatten) while collapsing the
            // common TS `(T | undefined)[]` → `T | undefined` flattening pattern.
            //
            // Nullish coalescing (`arr[i] ?? default`) is covered via
            // `resolve_bin_expr`'s NC arm (I-022): it propagates `Option<T>` to
            // the LHS span so this member access reads `Some(Option(_))` as the
            // expected type and emits the `.get().cloned()` (no-unwrap) form.
            let safe_index = convert_index_to_usize(index);
            let expected = self
                .tctx
                .type_resolution
                .expected_type(Span::from_swc(member.span));
            if let Some(RustType::Option(expected_inner)) = expected {
                let base = build_safe_index_expr(object, safe_index);
                let needs_flatten = matches!(
                    self.get_expr_type(&member.obj),
                    Some(RustType::Vec(v)) if matches!(
                        v.as_ref(),
                        RustType::Option(vi) if vi.as_ref() == expected_inner.as_ref()
                    )
                );
                if needs_flatten {
                    return Ok(Expr::MethodCall {
                        object: Box::new(base),
                        method: "flatten".to_string(),
                        args: vec![],
                    });
                }
                return Ok(base);
            }
            return Ok(build_safe_index_expr_unwrapped(object, safe_index));
        }

        // Iteration v10 third-review (DRY refactor): pre-refactor の `match member.prop` で
        // `Ident → sym / PrivateName → _{name}` を local extract していた logic を
        // `extract_non_computed_field_name` shared helper 経由に統合
        // (= `dispatch_member_write` と同 logic 共有、`design-integrity.md` "DRY")。
        // 本 path に到達する時点で `MemberProp::Computed` は冒頭の早期 return (line 87 +)
        // で handle 済 = `extract_non_computed_field_name` の `None` return は構造的 unreachable、
        // `unreachable!()` macro で structural invariant codify。
        let field = extract_non_computed_field_name(&member.prop).unwrap_or_else(|| {
            unreachable!(
                "convert_member_expr_inner: MemberProp::Computed is handled by the early \
                 `if let MemberProp::Computed` block (Vec index / tuple field access / Range / \
                 safe-bounds check), so this match's None arm is structurally unreachable"
            )
        });

        // process.env.VAR → std::env::var("VAR").unwrap()
        if let ast::Expr::Member(inner) = member.obj.as_ref() {
            if let (ast::Expr::Ident(obj), ast::MemberProp::Ident(prop)) =
                (inner.obj.as_ref(), &inner.prop)
            {
                if obj.sym.as_ref() == "process" && prop.sym.as_ref() == "env" {
                    return Ok(Expr::MethodCall {
                        object: Box::new(Expr::FnCall {
                            target: crate::ir::CallTarget::ExternalPath(vec![
                                "std".to_string(),
                                "env".to_string(),
                                "var".to_string(),
                            ]),
                            args: vec![Expr::StringLit(field)],
                        }),
                        method: "unwrap".to_string(),
                        args: vec![],
                    });
                }
            }
        }

        // Check if accessing a field of a discriminated union enum
        if let Some(RustType::Named { name, .. }) = self.get_expr_type(&member.obj) {
            if let Some(TypeDef::Enum {
                tag_field: Some(tag),
                variant_fields,
                ..
            }) = self.reg().get(name)
            {
                if field == *tag {
                    // Tag field → method call (e.g., s.kind() )
                    // Cat A: receiver object
                    let object = self.convert_expr(&member.obj)?;
                    return Ok(Expr::MethodCall {
                        object: Box::new(object),
                        method: tag.clone(),
                        args: vec![],
                    });
                }
                // Non-tag field: if bound in match arm destructuring,
                // clone the reference (match on &obj binds fields by reference)
                if self
                    .tctx
                    .type_resolution
                    .is_du_field_binding(&field, member.span.lo.0)
                {
                    return Ok(Expr::MethodCall {
                        object: Box::new(Expr::Ident(field)),
                        method: "clone".to_string(),
                        args: vec![],
                    });
                }
                // Standalone field access → inline match expression
                let variant_fields = variant_fields.clone();
                return self.convert_du_standalone_field_access(
                    &member.obj,
                    name,
                    &field,
                    &variant_fields,
                );
            }
        }

        // Cat A: receiver object
        let object = self.convert_expr(&member.obj)?;
        // I-205 T5 Iteration v9 deep deep review fix: Write context (assignment LHS) では
        // 本 T5 で導入した Read context dispatch logic (`resolve_member_access` の class
        // member dispatch、getter/setter/method の Tier 1/2 dispatch arm) を **skip**
        // し、既存 FieldAccess fallback path を維持する。Write context の setter dispatch
        // は subsequent T6 (Write context dispatch、`dispatch_member_write` helper) で別途
        // 実装。本 fix なしだと `f.x = 5;` (TS setter call) の LHS で getter dispatch が
        // apply されて `f.x() = 5.0;` (invalid Rust MethodCall LHS、compile error) を emit
        // する silent regression が発生する (Iteration v9 deep deep empirical probe で発覚)。
        if for_write {
            return Ok(Expr::FieldAccess {
                object: Box::new(object),
                field,
            });
        }
        self.resolve_member_access(&object, &field, &member.obj)
    }

    /// Discriminated union の standalone フィールドアクセスを inline match 式に変換する。
    ///
    /// `s.radius` → `match &s { Shape::Circle { radius, .. } => radius.clone(), _ => panic!("...") }`
    pub(crate) fn convert_du_standalone_field_access(
        &mut self,
        obj_expr: &ast::Expr,
        enum_name: &str,
        field: &str,
        variant_fields: &std::collections::HashMap<String, Vec<FieldDef>>,
    ) -> Result<Expr> {
        // Cat A: receiver object
        let object = self.convert_expr(obj_expr)?;
        let match_expr = Expr::Ref(Box::new(object));

        let mut arms: Vec<MatchArm> = Vec::new();

        // Create arms for variants that have this field
        for (variant_name, fields) in variant_fields {
            if fields.iter().any(|f| f.name == field) {
                arms.push(MatchArm {
                    patterns: vec![Pattern::Struct {
                        ctor: crate::ir::PatternCtor::UserEnumVariant {
                            enum_ty: crate::ir::UserTypeRef::new(enum_name.to_string()),
                            variant: variant_name.clone(),
                        },
                        fields: vec![(field.to_string(), Pattern::binding(field))],
                        rest: true,
                    }],
                    guard: None,
                    body: vec![Stmt::TailExpr(Expr::MethodCall {
                        object: Box::new(Expr::Ident(field.to_string())),
                        method: "clone".to_string(),
                        args: vec![],
                    })],
                });
            }
        }

        // Add wildcard arm with panic
        arms.push(MatchArm {
            patterns: vec![Pattern::Wildcard],
            guard: None,
            body: vec![Stmt::TailExpr(Expr::MacroCall {
                name: "panic".to_string(),
                args: vec![Expr::StringLit(format!(
                    "variant does not have field '{field}'"
                ))],
                use_debug: vec![false],
            })],
        });

        Ok(Expr::Match {
            expr: Box::new(match_expr),
            arms,
        })
    }
}

impl<'a> Transformer<'a> {
    /// Extracts the object and method name from an optional call's callee.
    ///
    /// Handles both `x.method` (`Member`) and `x?.method` (`OptChain(Member)`) patterns.
    fn extract_method_from_callee(&mut self, callee: &ast::Expr) -> Result<(Expr, String)> {
        let member = match callee {
            ast::Expr::Member(member) => member,
            ast::Expr::OptChain(opt) => match opt.base.as_ref() {
                ast::OptChainBase::Member(member) => member,
                _ => return Err(anyhow!("unsupported optional call callee")),
            },
            _ => return Err(anyhow!("unsupported optional call callee: {:?}", callee)),
        };
        let object = self.convert_expr(&member.obj)?;
        let method = match &member.prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            _ => return Err(anyhow!("unsupported optional call property")),
        };
        Ok((object, method))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_index_to_usize_integer_number_lit() {
        let result = convert_index_to_usize(Expr::NumberLit(0.0));
        assert_eq!(result, Expr::IntLit(0));
    }

    #[test]
    fn test_convert_index_to_usize_large_integer() {
        let result = convert_index_to_usize(Expr::NumberLit(42.0));
        assert_eq!(result, Expr::IntLit(42));
    }

    #[test]
    fn test_convert_index_to_usize_fractional_gets_cast() {
        let result = convert_index_to_usize(Expr::NumberLit(1.5));
        assert!(matches!(result, Expr::Cast { .. }));
    }

    #[test]
    fn test_convert_index_to_usize_negative_becomes_int_lit() {
        // -1.0 has fract() == 0.0, so it becomes IntLit(-1)
        // When used as usize, this wraps to a large number, but .get() safely returns None
        let result = convert_index_to_usize(Expr::NumberLit(-1.0));
        assert_eq!(result, Expr::IntLit(-1));
    }

    #[test]
    fn test_build_safe_index_expr_unwrapped_wraps_with_unwrap() {
        let result =
            build_safe_index_expr_unwrapped(Expr::Ident("arr".to_string()), Expr::IntLit(0));
        // Should be: arr.get(0).cloned().unwrap()
        match &result {
            Expr::MethodCall {
                object,
                method,
                args,
            } => {
                assert_eq!(method, "unwrap");
                assert!(args.is_empty());
                // Inner should be build_safe_index_expr result: arr.get(0).cloned()
                assert_eq!(
                    *object.as_ref(),
                    build_safe_index_expr(Expr::Ident("arr".to_string()), Expr::IntLit(0))
                );
            }
            other => panic!("expected MethodCall(unwrap), got: {other:?}"),
        }
    }

    #[test]
    fn test_convert_index_to_usize_variable_gets_cast() {
        let result = convert_index_to_usize(Expr::Ident("i".to_string()));
        match result {
            Expr::Cast { expr, target } => {
                assert_eq!(*expr, Expr::Ident("i".to_string()));
                // I-387: `usize` は `Primitive` variant で構造化
                assert_eq!(
                    target,
                    RustType::Primitive(crate::ir::PrimitiveIntKind::Usize)
                );
            }
            other => panic!("expected Cast, got: {other:?}"),
        }
    }
}
