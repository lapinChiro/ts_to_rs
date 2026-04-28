//! Object literal type resolution (`Object` arm of `resolve_expr_inner`).
//!
//! Resolves object literal expressions by walking `KeyValue` / `Shorthand` /
//! `Method` / `Getter` / `Setter` / `Assign` properties + `Spread` sources, then
//! either returns a pre-set expected type (annotation / return type / call arg),
//! reuses a common spread Named type, or registers an inline struct via
//! [`crate::pipeline::synthetic_registry::SyntheticTypeRegistry::register_inline_struct`].

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::super::*;
use crate::pipeline::type_resolution::Span;

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_object_expr(&mut self, obj: &ast::ObjectLit) -> ResolvedType {
        // Walk property values to resolve their types and collect field info.
        // For spread sources, resolve their types and extract fields from
        // TypeRegistry to build a complete field list.
        let mut explicit_fields: Vec<(String, RustType)> = Vec::new();
        let mut spread_types: Vec<RustType> = Vec::new();
        // Track the total number of non-spread properties to detect partial
        // resolution (some fields resolved, some didn't). We must not generate
        // an anonymous struct with missing fields — that would silently drop them.
        let mut total_explicit_props = 0u32;

        for prop in &obj.props {
            match prop {
                ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                    ast::Prop::KeyValue(kv) => {
                        total_explicit_props += 1;
                        let span = Span::from_swc(kv.value.span());
                        let ty = self.resolve_expr(&kv.value);
                        self.result.expr_types.insert(span, ty.clone());

                        let key = extract_prop_name(&kv.key);
                        if let (Some(key), ResolvedType::Known(rust_ty)) = (key, ty) {
                            explicit_fields.push((key, rust_ty));
                        }
                    }
                    ast::Prop::Shorthand(ident) => {
                        total_explicit_props += 1;
                        let span = Span::from_swc(ident.span);
                        let name = ident.sym.to_string();
                        let ty = self.lookup_var(&name);
                        self.result.expr_types.insert(span, ty.clone());

                        if let ResolvedType::Known(rust_ty) = ty {
                            explicit_fields.push((name, rust_ty));
                        }
                    }
                    ast::Prop::Method(method_prop) => {
                        // PRD 2.7 (I-200, cell 12): visit_method_function 同等処理で
                        // method body を walk、typeof/instanceof narrow event を push。
                        // Transformer 側の完全 emission (Tier 1 化) は I-202 で別 PRD。
                        total_explicit_props += 1;
                        let span = Span::from_swc(method_prop.function.span);
                        self.visit_prop_method_function(&method_prop.function);
                        self.result.expr_types.insert(span, ResolvedType::Unknown);
                    }
                    ast::Prop::Getter(getter_prop) => {
                        // PRD 2.7 (I-200, cell 13): getter body を visit_block_stmt
                        // 経由で walk。Transformer 完全 emission は I-202。
                        total_explicit_props += 1;
                        let span = Span::from_swc(getter_prop.span);
                        if let Some(body) = &getter_prop.body {
                            self.enter_scope();
                            let param_pats: Vec<&ast::Pat> = Vec::new();
                            self.collect_emission_hints(body, &param_pats);
                            self.visit_block_stmt(body);
                            self.leave_scope();
                        }
                        self.result.expr_types.insert(span, ResolvedType::Unknown);
                    }
                    ast::Prop::Setter(setter_prop) => {
                        // PRD 2.7 (I-200, cell 14): setter body を param_pat visit +
                        // visit_block_stmt 経由で walk。Transformer 完全 emission は I-202。
                        total_explicit_props += 1;
                        let span = Span::from_swc(setter_prop.span);
                        if let Some(body) = &setter_prop.body {
                            self.enter_scope();
                            self.visit_param_pat(&setter_prop.param);
                            let param_pats: Vec<&ast::Pat> = vec![&setter_prop.param];
                            self.collect_emission_hints(body, &param_pats);
                            self.visit_block_stmt(body);
                            self.leave_scope();
                        }
                        self.result.expr_types.insert(span, ResolvedType::Unknown);
                    }
                    ast::Prop::Assign(_) => {
                        // PRD 2.7 Implementation Revision 2 (2026-04-27、critical
                        // Spec gap fix): 当初 NA 認識だったが SWC parser empirical
                        // observation で `{ x = expr }` を `Prop::Assign` として
                        // accept することを確認。TS spec では parse error だが SWC
                        // parser は accept、ts_to_rs では Tier 2 honest error
                        // (UnsupportedSyntaxError) として処理。TypeResolver は no-op。
                        total_explicit_props += 1;
                    } // No `_ => ...` arm — PRD 2.7 Rule 11 (d-1) compliance.
                      // 新 Prop variant 追加時に compile error で全 dispatch fix を強制。
                },
                ast::PropOrSpread::Spread(spread) => {
                    let span = Span::from_swc(spread.expr.span());
                    let ty = self.resolve_expr(&spread.expr);
                    self.result.expr_types.insert(span, ty.clone());
                    if let ResolvedType::Known(rust_ty) = ty {
                        spread_types.push(rust_ty);
                    }
                }
            }
        }

        let obj_span = Span::from_swc(obj.span);

        // Store resolved spread fields for Transformer's spread expansion.
        // Must be done before the early return for pre-set expected types,
        // because the Transformer needs field names/types to convert `...spread`
        // into individual `field: spread.field` accesses regardless of how the
        // expected type was determined.
        if !spread_types.is_empty() {
            if let Some(fields) = self.merge_object_fields(&spread_types, &explicit_fields) {
                self.result.spread_fields.insert(obj_span, fields);
            }
        }

        if self.result.expected_types.contains_key(&obj_span) {
            // Expected type already set (from annotation, return type, etc.)
            // — skip anonymous struct generation
            return ResolvedType::Unknown;
        }

        // Abort if any explicit field's type couldn't be resolved.
        // Generating an anonymous struct with missing fields would cause confusing
        // Rust compile errors (unknown field) rather than the clear "requires type
        // annotation" error from the Transformer.
        if explicit_fields.len() != total_explicit_props as usize {
            return ResolvedType::Unknown;
        }

        // Build merged field list: spread source fields + explicit fields.
        // When spreads exist, use the pre-stored spread_fields (computed above).
        // When no spreads, merge from explicit fields only.
        let merged = if !spread_types.is_empty() {
            match self.result.spread_fields.get(&obj_span).cloned() {
                Some(fields) if !fields.is_empty() => fields,
                _ => return ResolvedType::Unknown,
            }
        } else {
            match self.merge_object_fields(&[], &explicit_fields) {
                Some(fields) if !fields.is_empty() => fields,
                _ => return ResolvedType::Unknown,
            }
        };

        // Determine the expected type:
        // - If all spread sources are the same Named type (including type_args)
        //   and no extra explicit fields, use that type directly.
        // - Otherwise, generate an anonymous struct from the merged fields.
        let expected_ty = if explicit_fields.is_empty() && !spread_types.is_empty() {
            if let Some(common_type) = common_named_type(&spread_types) {
                common_type
            } else {
                let name = self.synthetic.register_inline_struct(&merged);
                RustType::Named {
                    name,
                    type_args: vec![],
                }
            }
        } else {
            let name = self.synthetic.register_inline_struct(&merged);
            RustType::Named {
                name,
                type_args: vec![],
            }
        };
        self.result
            .expected_types
            .insert(obj_span, expected_ty.clone());
        ResolvedType::Known(expected_ty)
    }
}
