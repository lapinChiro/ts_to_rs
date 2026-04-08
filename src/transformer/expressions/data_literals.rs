//! Data literal conversions: object literals, array literals, and spread arrays.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{CallTarget, Expr, RustType, Stmt};
use crate::registry::TypeDef;

use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Converts a discriminated union object literal to an enum variant construction.
    ///
    /// Identifies the discriminant field value to determine the correct variant,
    /// then builds the variant with remaining fields (excluding the discriminant).
    pub(crate) fn convert_discriminated_union_object_lit(
        &mut self,
        obj_lit: &ast::ObjectLit,
        enum_name: &str,
        tag_field: &str,
        string_values: &std::collections::HashMap<String, String>,
    ) -> Result<Expr> {
        // Find the discriminant field value
        let mut disc_value = None;
        for prop in &obj_lit.props {
            if let ast::PropOrSpread::Prop(prop) = prop {
                if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                    let key = match &kv.key {
                        ast::PropName::Ident(ident) => ident.sym.to_string(),
                        ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => continue,
                    };
                    if key == tag_field {
                        if let ast::Expr::Lit(ast::Lit::Str(s)) = kv.value.as_ref() {
                            disc_value = Some(s.value.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }

        let disc_value = disc_value.ok_or_else(|| {
            anyhow!("discriminated union object literal missing discriminant field '{tag_field}'")
        })?;

        let variant_name = string_values.get(&disc_value).ok_or_else(|| {
            anyhow!("unknown discriminant value '{disc_value}' for enum '{enum_name}'")
        })?;

        // Build fields (excluding the discriminant field)
        let mut fields = Vec::new();
        for prop in &obj_lit.props {
            if let ast::PropOrSpread::Prop(prop) = prop {
                match prop.as_ref() {
                    ast::Prop::KeyValue(kv) => {
                        let key = match &kv.key {
                            ast::PropName::Ident(ident) => ident.sym.to_string(),
                            ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                            _ => continue,
                        };
                        if key == tag_field {
                            continue; // Skip discriminant field
                        }
                        let value = self.convert_expr(&kv.value)?;
                        fields.push((key, value));
                    }
                    ast::Prop::Shorthand(ident) => {
                        let key = ident.sym.to_string();
                        if key == tag_field {
                            continue;
                        }
                        let value = self.convert_expr(&ast::Expr::Ident(ident.clone()))?;
                        fields.push((key, value));
                    }
                    _ => {}
                }
            }
        }

        // Unit variant (no fields) → structural EnumVariant reference
        // (I-378 PRD-DEVIATION D-2: this site was missed in the original PRD's
        // 7-site enumeration; discovered during Phase 2 T10 test fixup as a
        // surviving `Expr::Ident("Status::Active")` form. Fixed for completeness.)
        if fields.is_empty() {
            return Ok(Expr::EnumVariant {
                enum_ty: crate::ir::UserTypeRef::new(enum_name.to_string()),
                variant: variant_name.to_string(),
            });
        }

        // Struct variant: `name` is `"Enum::Variant"` for the generator's
        // StructInit rendering (Rust enum struct-variant syntax). The display-
        // formatted `::` here is part of `Item::StructInit::name: String` which
        // is out of I-378 scope (separate broken window for `StructInit` IR
        // restructuring; tracked in TODO).
        Ok(Expr::StructInit {
            name: format!("{enum_name}::{variant_name}"),
            fields,
            base: None,
        })
    }

    /// Tries to convert an object literal with all computed keys to a `HashMap::from(...)`.
    ///
    /// Returns `Ok(Some(expr))` if all properties use computed keys, `Ok(None)` if not
    /// (mixed or no computed keys), or `Err` if a computed key value fails to convert.
    pub(crate) fn try_convert_as_hashmap(
        &mut self,
        obj_lit: &ast::ObjectLit,
    ) -> Result<Option<Expr>> {
        if obj_lit.props.is_empty() {
            return Ok(None);
        }

        // Check that ALL properties are key-value pairs with computed keys
        let mut entries = Vec::new();
        for prop in &obj_lit.props {
            match prop {
                ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                    ast::Prop::KeyValue(kv) => {
                        let computed_expr = match &kv.key {
                            ast::PropName::Computed(c) => &c.expr,
                            _ => return Ok(None), // non-computed key → not a HashMap
                        };
                        // Cat A: HashMap computed key — arbitrary expression
                        let key = self.convert_expr(computed_expr)?;
                        let value = self.convert_expr(&kv.value)?;
                        entries.push(Expr::Tuple {
                            elements: vec![key, value],
                        });
                    }
                    _ => return Ok(None),
                },
                ast::PropOrSpread::Spread(_) => return Ok(None),
            }
        }

        Ok(Some(Expr::FnCall {
            // `HashMap` is a Rust std type, not a user type.
            target: CallTarget::ExternalPath(vec!["HashMap".to_string(), "from".to_string()]),
            args: vec![Expr::Vec { elements: entries }],
        }))
    }

    /// Converts an SWC object literal to an IR `Expr::StructInit`.
    ///
    /// Follows TypeScript's rightmost-wins semantics: in `{ x: 1, ...a, y: 2 }`, the spread
    /// overrides `x` (because it appears after `x`), but `y: 2` overrides the spread's `y`
    /// (because the explicit field appears after the spread).
    ///
    /// Requires an expected type (`RustType::Named`) from the enclosing context (e.g., a variable
    /// declaration's type annotation). Without a named type, returns an error because Rust requires
    /// a named struct.
    pub(crate) fn convert_object_lit(
        &mut self,
        obj_lit: &ast::ObjectLit,
        expected: Option<&RustType>,
    ) -> Result<Expr> {
        // Check if all properties use computed keys → HashMap::from(vec![(k, v), ...])
        if let Some(hashmap) = self.try_convert_as_hashmap(obj_lit)? {
            return Ok(hashmap);
        }

        // Empty object literal with HashMap expected type → HashMap::new()
        if obj_lit.props.is_empty() {
            let is_hashmap_expected = matches!(
                expected,
                Some(RustType::Named { name, .. }) if name == "HashMap"
            ) || matches!(
                expected,
                Some(RustType::StdCollection {
                    kind: crate::ir::StdCollectionKind::HashMap,
                    ..
                })
            );
            if is_hashmap_expected {
                return Ok(Expr::FnCall {
                    target: CallTarget::ExternalPath(vec![
                        "HashMap".to_string(),
                        "new".to_string(),
                    ]),
                    args: vec![],
                });
            }
        }

        let struct_name = match expected {
            Some(RustType::Named { name, .. }) => name.as_str(),
            _ => {
                return Err(anyhow!(
                    "object literal requires a type annotation to determine struct name"
                ))
            }
        };

        // Check if this is a discriminated union enum
        if let Some(TypeDef::Enum {
            tag_field: Some(tag),
            string_values,
            ..
        }) = self.reg().get(struct_name)
        {
            let tag = tag.clone();
            let string_values = string_values.clone();
            return self.convert_discriminated_union_object_lit(
                obj_lit,
                struct_name,
                &tag,
                &string_values,
            );
        }

        // Look up field types for spread expansion and optional None completion.
        // Priority: pre-resolved spread_fields from TypeResolver (handles type param
        // constraints, Option unwrap, type_args instantiation) → TypeRegistry fallback.
        let obj_span = crate::pipeline::type_resolution::Span::from_swc(obj_lit.span);
        let struct_fields = self
            .tctx
            .type_resolution
            .spread_fields
            .get(&obj_span)
            .cloned()
            .or_else(|| {
                self.reg().get(struct_name).and_then(|def| match def {
                    TypeDef::Struct { fields, .. } => Some(
                        fields
                            .iter()
                            .map(|f| (f.name.clone(), f.ty.clone()))
                            .collect(),
                    ),
                    _ => None,
                })
            });

        // Build position-ordered event list preserving AST source order
        enum PropEvent {
            Explicit { key: String, value: Expr },
            Spread { expr: Expr },
        }

        let mut events = Vec::new();
        for prop in &obj_lit.props {
            match prop {
                ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                    ast::Prop::KeyValue(kv) => {
                        let key = match &kv.key {
                            ast::PropName::Ident(ident) => ident.sym.to_string(),
                            ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                            _ => return Err(anyhow!("unsupported object literal key")),
                        };
                        let value = self.convert_expr(&kv.value)?;
                        events.push(PropEvent::Explicit { key, value });
                    }
                    ast::Prop::Shorthand(ident) => {
                        let key = ident.sym.to_string();
                        let value = self.convert_expr(&ast::Expr::Ident(ident.clone()))?;
                        events.push(PropEvent::Explicit { key, value });
                    }
                    _ => {
                        return Err(anyhow!(
                        "unsupported object literal property (only key-value pairs and shorthand)"
                    ))
                    }
                },
                ast::PropOrSpread::Spread(spread_elem) => {
                    let spread_expr = self.convert_expr(&spread_elem.expr)?;
                    events.push(PropEvent::Spread { expr: spread_expr });
                }
            }
        }

        let has_spread = events.iter().any(|e| matches!(e, PropEvent::Spread { .. }));

        if let Some(all_fields) = &struct_fields {
            // Pre-bind non-pure spread sources to temp variables.
            // TypeScript evaluates spread sources exactly once; without this, the spread
            // expression would be cloned into each field's FieldAccess, causing multi-evaluation.
            let mut spread_bindings: std::collections::HashMap<usize, Stmt> =
                std::collections::HashMap::new();
            let mut spread_counter = 0usize;
            for (idx, event) in events.iter_mut().enumerate() {
                if let PropEvent::Spread { expr } = event {
                    if !expr.is_trivially_pure() {
                        let var_name = format!("__spread_obj_{spread_counter}");
                        spread_counter += 1;
                        let original = std::mem::replace(expr, Expr::Ident(var_name.clone()));
                        spread_bindings.insert(
                            idx,
                            Stmt::Let {
                                mutable: false,
                                name: var_name,
                                ty: None,
                                init: Some(original),
                            },
                        );
                    }
                }
            }

            // Registered type: resolve each field using rightmost-wins scan.
            // After pre-binding, spread expressions are Ident (trivially pure),
            // so cloning into each FieldAccess is safe.
            let mut fields = Vec::new();
            let mut used_indices = std::collections::HashSet::new();
            for (field_name, _) in all_fields {
                // Scan events right-to-left; first match wins (rightmost in source)
                let mut resolved = None;
                for (idx, event) in events.iter().enumerate().rev() {
                    match event {
                        PropEvent::Explicit { key, value } if key == field_name => {
                            resolved = Some(value.clone());
                            used_indices.insert(idx);
                            break;
                        }
                        PropEvent::Spread { expr } => {
                            resolved = Some(Expr::FieldAccess {
                                object: Box::new(expr.clone()),
                                field: field_name.clone(),
                            });
                            break;
                        }
                        _ => {}
                    }
                }
                if let Some(value) = resolved {
                    fields.push((field_name.clone(), value));
                }
            }

            // Auto-fill omitted Option<T> fields with None (only when no spread present)
            if !has_spread {
                let explicit_keys: std::collections::HashSet<String> =
                    fields.iter().map(|(k, _)| k.clone()).collect();
                for (field_name, field_ty) in all_fields {
                    if !explicit_keys.contains(field_name)
                        && matches!(field_ty, RustType::Option(_))
                    {
                        fields.push((
                            field_name.clone(),
                            Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None),
                        ));
                    }
                }
            }

            // Collect all source-ordered pre-evaluations:
            // - Overridden non-pure explicits → let _ = expr;
            // - Pre-bound spread sources → let __spread_obj = expr;
            // TypeScript evaluates all expressions left-to-right.
            let mut side_effects: Vec<Stmt> = Vec::new();
            for (idx, event) in events.iter().enumerate() {
                if let Some(binding) = spread_bindings.remove(&idx) {
                    side_effects.push(binding);
                } else if let PropEvent::Explicit { value, .. } = event {
                    if !used_indices.contains(&idx) && !value.is_trivially_pure() {
                        side_effects.push(Stmt::Let {
                            mutable: false,
                            name: "_".to_string(),
                            ty: None,
                            init: Some(value.clone()),
                        });
                    }
                }
            }

            let struct_init = Expr::StructInit {
                name: struct_name.to_string(),
                fields,
                base: None,
            };
            if side_effects.is_empty() {
                Ok(struct_init)
            } else {
                side_effects.push(Stmt::TailExpr(struct_init));
                Ok(Expr::Block(side_effects))
            }
        } else {
            // Unregistered type: use struct update syntax
            let spread_count = events
                .iter()
                .filter(|e| matches!(e, PropEvent::Spread { .. }))
                .count();

            if spread_count > 1 {
                return Err(anyhow!(
                    "multiple spreads with unregistered type '{}' — TypeRegistry required for field expansion",
                    struct_name
                ));
            }

            if spread_count == 0 {
                // No spreads — collect all explicit fields
                let fields: Vec<(String, Expr)> = events
                    .into_iter()
                    .filter_map(|e| match e {
                        PropEvent::Explicit { key, value } => Some((key, value)),
                        PropEvent::Spread { .. } => None,
                    })
                    .collect();
                return Ok(Expr::StructInit {
                    name: struct_name.to_string(),
                    fields,
                    base: None,
                });
            }

            // Single spread: only explicit fields AFTER the spread are kept;
            // fields before the spread are overridden by it.
            let spread_idx = events
                .iter()
                .position(|e| matches!(e, PropEvent::Spread { .. }))
                .unwrap();

            // Collect side effects from dropped explicits BEFORE the spread.
            // Uses `let _ = expr;` to suppress unused-value warnings.
            let side_effects: Vec<Stmt> = events
                .iter()
                .take(spread_idx)
                .filter_map(|event| {
                    if let PropEvent::Explicit { value, .. } = event {
                        if !value.is_trivially_pure() {
                            return Some(Stmt::Let {
                                mutable: false,
                                name: "_".to_string(),
                                ty: None,
                                init: Some(value.clone()),
                            });
                        }
                    }
                    None
                })
                .collect();

            let spread_expr = match events.remove(spread_idx) {
                PropEvent::Spread { expr } => expr,
                _ => unreachable!(),
            };

            // Only fields after the spread position (accounting for the removal)
            let fields: Vec<(String, Expr)> = events
                .into_iter()
                .skip(spread_idx)
                .filter_map(|e| match e {
                    PropEvent::Explicit { key, value } => Some((key, value)),
                    PropEvent::Spread { .. } => None,
                })
                .collect();

            let struct_init = Expr::StructInit {
                name: struct_name.to_string(),
                fields,
                base: Some(Box::new(spread_expr)),
            };
            if side_effects.is_empty() {
                Ok(struct_init)
            } else {
                let mut stmts = side_effects;
                stmts.push(Stmt::TailExpr(struct_init));
                Ok(Expr::Block(stmts))
            }
        }
    }

    /// Converts an SWC array literal to an IR `Expr::Vec` or `Expr::VecSpread`.
    ///
    /// When `expected` is `RustType::Vec(inner)`, the inner type is propagated to each element.
    ///
    /// Spread arrays (`[...arr, 1]`) are handled at the statement level by `try_expand_spread_*`
    /// in `convert_stmt`, so only non-spread arrays should reach here. If a spread array reaches
    /// here (e.g., nested in a function call argument), an error is returned.
    pub(crate) fn convert_array_lit(
        &mut self,
        array_lit: &ast::ArrayLit,
        expected: Option<&RustType>,
    ) -> Result<Expr> {
        let has_spread = array_lit
            .elems
            .iter()
            .filter_map(|e| e.as_ref())
            .any(|e| e.spread.is_some());

        // When expected is a Tuple type, convert to Expr::Tuple
        if let Some(RustType::Tuple(_)) = expected {
            let elements = array_lit
                .elems
                .iter()
                .filter_map(|elem| elem.as_ref())
                .map(|elem| self.convert_expr(&elem.expr))
                .collect::<Result<Vec<_>>>()?;
            return Ok(Expr::Tuple { elements });
        }

        if has_spread {
            return self.convert_spread_array_to_block(array_lit);
        }

        let elements = array_lit
            .elems
            .iter()
            .filter_map(|elem| elem.as_ref())
            .map(|elem| self.convert_expr(&elem.expr))
            .collect::<Result<Vec<_>>>()?;
        Ok(Expr::Vec { elements })
    }

    /// Converts a spread array literal to an `Expr::Block` that builds the vec at runtime.
    ///
    /// `[1, ...arr, 2]` becomes:
    /// ```text
    /// {
    ///     let mut _v = vec![1.0];
    ///     _v.extend(arr.iter().cloned());
    ///     _v.push(2.0);
    ///     _v
    /// }
    /// ```
    pub(crate) fn convert_spread_array_to_block(
        &mut self,
        array_lit: &ast::ArrayLit,
    ) -> Result<Expr> {
        let mut stmts: Vec<Stmt> = Vec::new();

        // Collect initial non-spread elements for vec![...] initialization
        let mut init_elements: Vec<Expr> = Vec::new();
        let mut initialized = false;

        for elem_opt in &array_lit.elems {
            let elem = match elem_opt {
                Some(e) => e,
                None => continue,
            };

            if elem.spread.is_some() {
                // Emit initialization if not yet done
                if !initialized {
                    stmts.push(Stmt::Let {
                        mutable: true,
                        name: "_v".to_string(),
                        ty: None,
                        init: Some(Expr::Vec {
                            elements: std::mem::take(&mut init_elements),
                        }),
                    });
                    initialized = true;
                }
                // Cat A: spread source — type is the array itself
                let spread_expr = self.convert_expr(&elem.expr)?;
                stmts.push(Stmt::Expr(Expr::MethodCall {
                    object: Box::new(Expr::Ident("_v".to_string())),
                    method: "extend".to_string(),
                    args: vec![Expr::MethodCall {
                        object: Box::new(Expr::MethodCall {
                            object: Box::new(spread_expr),
                            method: "iter".to_string(),
                            args: vec![],
                        }),
                        method: "cloned".to_string(),
                        args: vec![],
                    }],
                }));
            } else {
                let value = self.convert_expr(&elem.expr)?;
                if initialized {
                    // _v.push(value)
                    stmts.push(Stmt::Expr(Expr::MethodCall {
                        object: Box::new(Expr::Ident("_v".to_string())),
                        method: "push".to_string(),
                        args: vec![value],
                    }));
                } else {
                    init_elements.push(value);
                }
            }
        }

        // If no spread was encountered (shouldn't happen), fall back
        if !initialized {
            return Ok(Expr::Vec {
                elements: init_elements,
            });
        }

        stmts.push(Stmt::TailExpr(Expr::Ident("_v".to_string())));
        Ok(Expr::Block(stmts))
    }
}
