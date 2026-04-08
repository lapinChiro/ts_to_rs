//! Expected type propagation for TypeResolver.
//!
//! Propagates expected types from parent contexts (variable annotations, return types,
//! function parameters) into child expressions (object literal fields, array elements,
//! ternary branches).

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::ir::Item;
use crate::pipeline::type_resolution::Span;

impl<'a> TypeResolver<'a> {
    /// Resolves type parameter names to their constraint types recursively.
    ///
    /// When a `RustType::TypeVar { name: "T" }` (I-387) or legacy
    /// `RustType::Named { name: "T", type_args: [] }` is encountered and `T`
    /// is in `type_param_constraints`, replaces it with the constraint type.
    /// Also resolves type parameters within `type_args` of Named types.
    ///
    /// This ensures expected types contain concrete type names that the
    /// Transformer can use for struct initialization, rather than type
    /// parameter names that don't exist in the TypeRegistry.
    pub(super) fn resolve_type_params_in_type(&self, ty: &RustType) -> RustType {
        self.resolve_type_params_impl(ty, 0)
    }

    fn resolve_type_params_impl(&self, ty: &RustType, depth: usize) -> RustType {
        // Guard against circular references in type_param_constraints
        // (e.g., "T" → Named("T") when T has no constraint but appears in the map)
        if depth > 10 {
            return ty.clone();
        }
        match ty {
            // I-387: TypeVar は型パラメータ参照の一級表現。constraint lookup で解決。
            RustType::TypeVar { name } => {
                if let Some(constraint) = self.type_param_constraints.get(name) {
                    if constraint == ty {
                        return ty.clone();
                    }
                    return self.resolve_type_params_impl(constraint, depth + 1);
                }
                ty.clone()
            }
            RustType::Named { name, type_args } => {
                // If name itself is a type parameter, resolve to constraint
                if type_args.is_empty() {
                    if let Some(constraint) = self.type_param_constraints.get(name) {
                        // Skip self-referential constraints to prevent infinite recursion
                        if constraint == ty {
                            return ty.clone();
                        }
                        return self.resolve_type_params_impl(constraint, depth + 1);
                    }
                    // Handle "::" compound names like "E::Bindings" from indexed access types.
                    // Split into base ("E") and field ("Bindings"), resolve the base via
                    // type parameter constraints, then look up the field type from the
                    // resolved struct.
                    if name.contains("::") {
                        if let Some((base, field)) = name.split_once("::") {
                            if let Some(constraint) = self.type_param_constraints.get(base) {
                                let resolved_base =
                                    self.resolve_type_params_impl(constraint, depth + 1);
                                if let RustType::Named {
                                    name: ref resolved_name,
                                    ref type_args,
                                } = resolved_base
                                {
                                    if let Some(fields) =
                                        self.resolve_struct_fields_by_name(resolved_name, type_args)
                                    {
                                        if let Some((_, field_ty)) =
                                            fields.iter().find(|(n, _)| n == field)
                                        {
                                            return self
                                                .resolve_type_params_impl(field_ty, depth + 1);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // Resolve type params within type_args
                let resolved_args: Vec<RustType> = type_args
                    .iter()
                    .map(|a| self.resolve_type_params_impl(a, depth + 1))
                    .collect();
                if &resolved_args == type_args {
                    ty.clone()
                } else {
                    RustType::Named {
                        name: name.clone(),
                        type_args: resolved_args,
                    }
                }
            }
            RustType::Option(inner) => {
                let resolved = self.resolve_type_params_impl(inner, depth + 1);
                if &resolved == inner.as_ref() {
                    ty.clone()
                } else {
                    RustType::Option(Box::new(resolved))
                }
            }
            RustType::Vec(inner) => {
                let resolved = self.resolve_type_params_impl(inner, depth + 1);
                if &resolved == inner.as_ref() {
                    ty.clone()
                } else {
                    RustType::Vec(Box::new(resolved))
                }
            }
            RustType::Fn {
                params,
                return_type,
            } => {
                let resolved_params: Vec<RustType> = params
                    .iter()
                    .map(|p| self.resolve_type_params_impl(p, depth + 1))
                    .collect();
                let resolved_ret = self.resolve_type_params_impl(return_type, depth + 1);
                if &resolved_params == params && &resolved_ret == return_type.as_ref() {
                    ty.clone()
                } else {
                    RustType::Fn {
                        params: resolved_params,
                        return_type: Box::new(resolved_ret),
                    }
                }
            }
            RustType::Tuple(types) => {
                let resolved: Vec<RustType> = types
                    .iter()
                    .map(|t| self.resolve_type_params_impl(t, depth + 1))
                    .collect();
                if &resolved == types {
                    ty.clone()
                } else {
                    RustType::Tuple(resolved)
                }
            }
            _ => ty.clone(),
        }
    }

    /// Resolves struct fields by type name from TypeRegistry, SyntheticTypeRegistry,
    /// and type parameter constraints.
    ///
    /// This is the single source of truth for "given a type name, get its struct fields".
    /// All field resolution in the TypeResolver (member access, spread sources, object
    /// literal propagation) delegates to this method.
    ///
    /// Resolution order:
    /// 1. TypeRegistry (with type_args instantiation for generics)
    /// 2. SyntheticTypeRegistry (inline structs like `_TypeLit0`)
    /// 3. Type parameter constraint fallback (bare type params only, recursive)
    pub(super) fn resolve_struct_fields_by_name(
        &self,
        name: &str,
        type_args: &[RustType],
    ) -> Option<Vec<(String, RustType)>> {
        // 1. TypeRegistry
        let type_def = if type_args.is_empty() {
            self.registry.get(name).cloned()
        } else {
            self.registry.instantiate(name, type_args)
        };
        if let Some(TypeDef::Struct { fields, .. }) = type_def {
            return Some(
                fields
                    .iter()
                    .map(|f| (f.name.clone(), f.ty.clone()))
                    .collect(),
            );
        }

        // 2. SyntheticTypeRegistry (inline object types → _TypeLitN)
        if let Some(def) = self.synthetic.get(name) {
            if let Item::Struct { fields, .. } = &def.item {
                return Some(
                    fields
                        .iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect(),
                );
            }
        }

        // 3. Type parameter constraint (bare type params only).
        // A name with type_args (like `Foo<Bar>`) is a concrete type, not a
        // type parameter, so constraint lookup would be semantically wrong.
        if type_args.is_empty() {
            if let Some(RustType::Named {
                name: cn,
                type_args: ca,
            }) = self.type_param_constraints.get(name)
            {
                return self.resolve_struct_fields_by_name(cn, ca);
            }
        }

        None
    }

    /// Resolves the field list for an object literal based on the expected type name.
    ///
    /// For `TypeDef::Enum` (discriminated union), identifies the variant from the
    /// tag field value in the object literal, then returns the variant's fields.
    /// For structs, delegates to [`resolve_struct_fields_by_name`].
    pub(super) fn resolve_object_lit_fields(
        &self,
        type_name: &str,
        type_args: &[RustType],
        obj: &ast::ObjectLit,
    ) -> Option<Vec<(String, RustType)>> {
        // Enum (discriminated union) — TypeRegistry only
        if let Some(TypeDef::Enum {
            tag_field: Some(tag),
            variant_fields,
            string_values,
            ..
        }) = self.registry.get(type_name)
        {
            let tag_value = find_string_prop_value(obj, tag)?;
            let variant_name = string_values.get(&tag_value)?;
            let fields = variant_fields.get(variant_name)?;
            return Some(
                fields
                    .iter()
                    .map(|f| (f.name.clone(), f.ty.clone()))
                    .collect(),
            );
        }

        // Struct resolution (TypeRegistry + SyntheticTypeRegistry + constraints)
        self.resolve_struct_fields_by_name(type_name, type_args)
    }

    /// Resolves the field list from a spread source type.
    ///
    /// Handles `Option<T>` unwrapping, then delegates to [`resolve_struct_fields_by_name`]
    /// for the actual field resolution.
    fn resolve_spread_source_fields(
        &self,
        spread_ty: &RustType,
    ) -> Option<Vec<(String, RustType)>> {
        match spread_ty {
            RustType::Option(inner) => self.resolve_spread_source_fields(inner),
            RustType::Named { name, type_args } => {
                self.resolve_struct_fields_by_name(name, type_args)
            }
            // I-387: TypeVar は型パラメータ参照。type_param_constraints 経由で
            // 元の Named 名に解決してから struct field を lookup する。
            RustType::TypeVar { name } => self.resolve_struct_fields_by_name(name, &[]),
            _ => None,
        }
    }

    /// Merges fields from spread sources and explicit properties into a unified field list.
    ///
    /// Spread source types are resolved through [`resolve_struct_fields_by_name`] to extract
    /// their fields (TypeRegistry, SyntheticTypeRegistry, and type parameter constraints).
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
        // I-387: TypeVar を上流で Named に解決してから propagate する。
        // これにより下流の Named リテラルチェックが型変数ケースもカバーする。
        if let RustType::TypeVar { .. } = expected {
            let resolved = self.resolve_type_params_in_type(expected);
            if &resolved != expected {
                return self.propagate_expected(expr, &resolved);
            }
        }
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
                    RustType::Named { name, type_args } => {
                        // Struct or DU — set field types from type registry.
                        // Resolve type params in type_args so generic structs
                        // get properly instantiated fields.
                        let resolved_args: Vec<RustType> = type_args
                            .iter()
                            .map(|a| self.resolve_type_params_in_type(a))
                            .collect();
                        let fields = self.resolve_object_lit_fields(name, &resolved_args, obj);
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

    /// Propagates expected types to default expressions within destructuring patterns.
    ///
    /// For `const { color = "black" } = opts` where `opts: Options` and
    /// `color?: string` (i.e., `Option<String>` in Rust), sets `String` as the
    /// expected type for `"black"`. For `Option<T>` fields, unwraps to `T`.
    pub(super) fn propagate_destructuring_defaults(
        &mut self,
        pat: &ast::Pat,
        source_type: &ResolvedType,
    ) {
        let source_rust_type = match source_type {
            ResolvedType::Known(ty) => ty,
            _ => return,
        };
        match pat {
            ast::Pat::Object(obj_pat) => {
                for prop in &obj_pat.props {
                    if let ast::ObjectPatProp::Assign(assign) = prop {
                        if let Some(default_expr) = &assign.value {
                            let field_name = assign.key.sym.to_string();
                            if let Some(field_type) =
                                self.lookup_struct_field(source_rust_type, &field_name)
                            {
                                let expected =
                                    super::helpers::unwrap_option_for_default(field_type);
                                let span = Span::from_swc(default_expr.span());
                                self.result.expected_types.insert(span, expected.clone());
                                self.propagate_expected(default_expr, &expected);
                            }
                        }
                    }
                }
            }
            ast::Pat::Array(arr_pat) => {
                for (i, elem) in arr_pat.elems.iter().enumerate() {
                    if let Some(ast::Pat::Assign(assign)) = elem {
                        let elem_type =
                            super::helpers::lookup_array_element_type(source_rust_type, i);
                        if let Some(ty) = elem_type {
                            // Unwrap Option<T> → T for the default expression
                            let expected = super::helpers::unwrap_option_for_default(ty);
                            let span = Span::from_swc(assign.right.span());
                            self.result.expected_types.insert(span, expected.clone());
                            self.propagate_expected(&assign.right, &expected);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Looks up a field type from a struct definition in the registry.
    ///
    /// Handles `Option<Named>` unwrapping before delegating to `TypeRegistry::lookup_field_type`.
    pub(super) fn lookup_struct_field(
        &self,
        source_type: &RustType,
        field_name: &str,
    ) -> Option<RustType> {
        // Unwrap Option<T> to get the inner Named type for lookup
        let inner_type = match source_type {
            RustType::Option(inner) => inner.as_ref(),
            other => other,
        };
        self.registry.lookup_field_type(inner_type, field_name)
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
            let resolved = self.resolve_type_params_in_type(param_ty);
            let arg_span = Span::from_swc(arg.expr.span());
            self.result
                .expected_types
                .insert(arg_span, resolved.clone());
            self.propagate_expected(&arg.expr, &resolved);
        }
    }
}
