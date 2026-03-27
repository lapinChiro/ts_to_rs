//! Expected type propagation for TypeResolver.
//!
//! Propagates expected types from parent contexts (variable annotations, return types,
//! function parameters) into child expressions (object literal fields, array elements,
//! ternary branches).

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::type_resolution::Span;

impl<'a> TypeResolver<'a> {
    /// Resolves the field list for an object literal based on the expected type name.
    ///
    /// For `TypeDef::Struct`, returns its fields directly.
    /// For `TypeDef::Enum` (discriminated union), identifies the variant from the
    /// tag field value in the object literal, then returns the variant's fields.
    pub(super) fn resolve_object_lit_fields(
        &self,
        type_name: &str,
        obj: &ast::ObjectLit,
    ) -> Option<Vec<(String, RustType)>> {
        match self.registry.get(type_name) {
            Some(TypeDef::Struct { fields, .. }) => Some(fields.clone()),
            Some(TypeDef::Enum {
                tag_field: Some(tag),
                variant_fields,
                string_values,
                ..
            }) => {
                let tag_value = find_string_prop_value(obj, tag)?;
                let variant_name = string_values.get(&tag_value)?;
                variant_fields.get(variant_name).cloned()
            }
            _ => None,
        }
    }

    /// Resolves the field list from a spread source type.
    ///
    /// Handles three resolution strategies in order:
    /// 1. `Option<T>` → unwrap and resolve inner type's fields
    /// 2. `Named` type → registry lookup (with type_args instantiation if present)
    /// 3. `Named` type not in registry → type parameter constraint lookup
    fn resolve_spread_source_fields(
        &self,
        spread_ty: &RustType,
    ) -> Option<Vec<(String, RustType)>> {
        match spread_ty {
            // 1. Option<T> → unwrap and resolve inner
            RustType::Option(inner) => self.resolve_spread_source_fields(inner),
            // 2. Named type
            RustType::Named { name, type_args } => {
                // 2a. Instantiate with type_args if present
                let type_def = if type_args.is_empty() {
                    self.registry.get(name).cloned()
                } else {
                    self.registry.instantiate(name, type_args)
                };
                if let Some(TypeDef::Struct { fields, .. }) = type_def {
                    return Some(fields);
                }
                // 2b. Type parameter → resolve via constraint.
                // Only applies when type_args is empty (bare type parameter like `E`).
                // A name with type_args (like `Foo<Bar>`) is a concrete type, not a
                // type parameter, so constraint lookup would be semantically wrong.
                if type_args.is_empty() {
                    if let Some(constraint) = self.type_param_constraints.get(name) {
                        return self.resolve_spread_source_fields(constraint);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Merges fields from spread sources and explicit properties into a unified field list.
    ///
    /// Spread source types are resolved through TypeRegistry to extract their fields.
    /// Explicit fields override spread fields with the same name (TS semantics).
    ///
    /// Returns `None` if any spread source's fields cannot be resolved (the type is not
    /// a Named type with a Struct definition in the registry). This prevents generating
    /// anonymous structs with incomplete field information, which would silently drop
    /// spread fields — a semantic change.
    pub(super) fn merge_object_fields(
        &self,
        spread_types: &[RustType],
        explicit_fields: &[(String, RustType)],
    ) -> Option<Vec<(String, RustType)>> {
        let mut merged: Vec<(String, RustType)> = Vec::new();

        // Collect fields from spread sources (in order)
        for spread_ty in spread_types {
            let fields = self.resolve_spread_source_fields(spread_ty)?;
            for (field_name, field_ty) in &fields {
                if let Some(pos) = merged.iter().position(|(n, _)| n == field_name) {
                    merged[pos] = (field_name.clone(), field_ty.clone());
                } else {
                    merged.push((field_name.clone(), field_ty.clone()));
                }
            }
        }

        // Explicit fields override spread fields
        for (name, ty) in explicit_fields {
            if let Some(pos) = merged.iter().position(|(n, _)| n == name) {
                merged[pos] = (name.clone(), ty.clone());
            } else {
                merged.push((name.clone(), ty.clone()));
            }
        }

        Some(merged)
    }

    /// Propagates an expected type into compound expressions recursively.
    ///
    /// When a parent context provides an expected type (e.g., variable annotation,
    /// return type, function parameter), this method sets expected types on child
    /// expressions (object literal fields, array elements, ternary branches, etc.).
    pub(super) fn propagate_expected(&mut self, expr: &ast::Expr, expected: &RustType) {
        match expr {
            // Object literal: propagate field types from struct/enum or HashMap value type
            ast::Expr::Object(obj) => {
                match expected {
                    RustType::Named { name, type_args }
                        if name == "HashMap" && type_args.len() == 2 =>
                    {
                        // HashMap<K, V> — set value type V for each computed property
                        let value_type = &type_args[1];
                        for prop in &obj.props {
                            if let ast::PropOrSpread::Prop(prop) = prop {
                                if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                                    let span = Span::from_swc(kv.value.span());
                                    self.result.expected_types.insert(span, value_type.clone());
                                    self.propagate_expected(&kv.value, value_type);
                                }
                            }
                        }
                    }
                    RustType::Named { name, .. } => {
                        // Struct or DU — set field types from type registry
                        let fields = self.resolve_object_lit_fields(name, obj);
                        if let Some(fields) = fields {
                            for prop in &obj.props {
                                if let ast::PropOrSpread::Prop(prop) = prop {
                                    match prop.as_ref() {
                                        ast::Prop::KeyValue(kv) => {
                                            let key = extract_prop_name(&kv.key);
                                            if let Some(field_ty) = key
                                                .and_then(|k| fields.iter().find(|(n, _)| n == &k))
                                            {
                                                let span = Span::from_swc(kv.value.span());
                                                self.result
                                                    .expected_types
                                                    .insert(span, field_ty.1.clone());
                                                self.propagate_expected(&kv.value, &field_ty.1);
                                            }
                                        }
                                        ast::Prop::Shorthand(ident) => {
                                            let key = ident.sym.to_string();
                                            if let Some(field_ty) =
                                                fields.iter().find(|(n, _)| n == &key)
                                            {
                                                let span = Span::from_swc(ident.span);
                                                self.result
                                                    .expected_types
                                                    .insert(span, field_ty.1.clone());
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            // Array literal: propagate Vec element type or Tuple positional types
            ast::Expr::Array(arr) => match expected {
                RustType::Vec(inner) => {
                    for elem in arr.elems.iter().flatten() {
                        let span = Span::from_swc(elem.expr.span());
                        self.result
                            .expected_types
                            .insert(span, inner.as_ref().clone());
                        self.propagate_expected(&elem.expr, inner);
                    }
                }
                RustType::Tuple(types) => {
                    for (elem, ty) in arr.elems.iter().flatten().zip(types.iter()) {
                        let span = Span::from_swc(elem.expr.span());
                        self.result.expected_types.insert(span, ty.clone());
                        self.propagate_expected(&elem.expr, ty);
                    }
                }
                _ => {}
            },
            // Parenthesized expr → propagate to inner expression
            ast::Expr::Paren(paren) => {
                let span = Span::from_swc(paren.expr.span());
                self.result.expected_types.insert(span, expected.clone());
                self.propagate_expected(&paren.expr, expected);
            }
            // Ternary conditional → propagate to both branches
            ast::Expr::Cond(cond) => {
                let cons_span = Span::from_swc(cond.cons.span());
                self.result
                    .expected_types
                    .insert(cons_span, expected.clone());
                self.propagate_expected(&cond.cons, expected);
                let alt_span = Span::from_swc(cond.alt.span());
                self.result
                    .expected_types
                    .insert(alt_span, expected.clone());
                self.propagate_expected(&cond.alt, expected);
            }
            _ => {}
        }
    }

    /// Sets expected types for function/constructor arguments from resolved parameter types.
    ///
    /// Zips arguments with parameter types and propagates each expected type
    /// into the argument expression. Extra arguments (beyond param count) are ignored.
    pub(super) fn propagate_arg_expected_types(
        &mut self,
        args: &[ast::ExprOrSpread],
        param_types: &[RustType],
    ) {
        for (arg, param_ty) in args.iter().zip(param_types.iter()) {
            let arg_span = Span::from_swc(arg.expr.span());
            self.result
                .expected_types
                .insert(arg_span, param_ty.clone());
            self.propagate_expected(&arg.expr, param_ty);
        }
    }
}
