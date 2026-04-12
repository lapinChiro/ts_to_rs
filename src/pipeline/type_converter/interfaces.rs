use super::*;

/// Converts an interface declaration into one or more IR items.
///
/// - Properties only → `[Struct]`
/// - Methods only → `[Trait]`
/// - Call signatures only → `[TypeAlias]` (fn type)
/// - Properties + Methods mixed → `[Struct, Trait, Impl]`
pub fn convert_interface_items(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Vec<Item>> {
    let name = sanitize_rust_type_name(&decl.id.sym);
    let tp_names: Vec<String> = decl
        .type_params
        .as_ref()
        .map(|tp| tp.params.iter().map(|p| p.name.sym.to_string()).collect())
        .unwrap_or_default();
    let prev_scope = synthetic.push_type_param_scope(tp_names);
    let (type_params, mono_subs) = extract_type_params(decl.type_params.as_deref(), synthetic, reg);

    let has_methods = decl
        .body
        .body
        .iter()
        .any(|m| matches!(m, TsTypeElement::TsMethodSignature(_)));
    let has_properties = decl
        .body
        .body
        .iter()
        .any(|m| matches!(m, TsTypeElement::TsPropertySignature(_)));

    let result = (|| -> Result<Vec<Item>> {
        if crate::registry::interfaces::is_callable_only(&decl.body.body) {
            let item =
                convert_callable_interface_as_trait(decl, vis, &name, type_params, synthetic, reg)?;
            return Ok(vec![item]);
        }

        if has_methods && has_properties {
            return convert_interface_as_struct_and_trait(
                decl,
                vis,
                &name,
                type_params,
                synthetic,
                reg,
            );
        }

        if has_methods {
            let item = convert_interface_as_trait(decl, vis, &name, type_params, synthetic, reg)?;
            return Ok(vec![item]);
        }

        let item = convert_interface_as_struct(decl, vis, &name, type_params, synthetic, reg)?;
        Ok(vec![item])
    })();

    synthetic.restore_type_param_scope(prev_scope);

    let items = result?;
    Ok(apply_mono_subs_to_items(items, &mono_subs))
}

/// Converts an interface into a single IR item (legacy API, delegates to `convert_interface_items`).
pub fn convert_interface(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Item> {
    let items = convert_interface_items(decl, vis, synthetic, reg)?;
    Ok(items.into_iter().next().unwrap())
}

/// Converts an interface with only property signatures into an IR [`Item::Struct`].
///
/// If the interface extends other interfaces, parent fields are included
/// (flattened) before the child's own fields.
fn convert_interface_as_struct(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<TypeParam>,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Item> {
    let mut fields = Vec::new();

    // Flatten parent fields from extends chain
    for parent_name in collect_extends_names(decl) {
        if let Some(TypeDef::Struct {
            fields: parent_fields,
            ..
        }) = reg.get(&parent_name)
        {
            for field in parent_fields {
                let sanitized = sanitize_field_name(&field.name);
                if !fields.iter().any(|f: &StructField| f.name == sanitized) {
                    fields.push(StructField {
                        vis: Some(Visibility::Public),
                        name: sanitized,
                        ty: field.ty.clone(),
                    });
                }
            }
        }
    }

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                let field = convert_property_signature(prop, synthetic, reg)?;
                fields.push(field);
            }
            _ => {
                return Err(anyhow!(
                    "unsupported interface member (only property signatures are supported)"
                ));
            }
        }
    }

    Ok(Item::Struct {
        vis,
        name: name.to_string(),
        type_params,
        fields,
        is_unit_struct: false,
    })
}

/// Converts a call-signature-only interface into a fn type alias.
///
/// `interface Foo { (x: number): string }` → `trait Foo { fn call_0(&self, x: f64) -> String; }`
///
/// Each call signature becomes a separate `call_N` method in the trait.
fn convert_callable_interface_as_trait(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<TypeParam>,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Item> {
    let call_sigs: Vec<&swc_ecma_ast::TsCallSignatureDecl> = decl
        .body
        .body
        .iter()
        .filter_map(|m| match m {
            TsTypeElement::TsCallSignatureDecl(sig) => Some(sig),
            _ => None,
        })
        .collect();

    if call_sigs.is_empty() {
        return Err(anyhow!("no call signatures found"));
    }

    let mut methods = Vec::new();
    let mut merged_tp = type_params;

    for (i, sig) in call_sigs.iter().enumerate() {
        // Push call signature's own type params to scope for this sig
        let sig_tp_names: Vec<String> = sig
            .type_params
            .as_ref()
            .map(|tpd| tpd.params.iter().map(|p| p.name.sym.to_string()).collect())
            .unwrap_or_default();
        let prev_scope = synthetic.push_type_param_scope(sig_tp_names);

        let method = (|| -> Result<Method> {
            let mut params = Vec::new();
            for param in &sig.params {
                match param {
                    swc_ecma_ast::TsFnParam::Ident(ident) => {
                        let param_name = ident.id.sym.to_string();
                        let ty = ident
                            .type_ann
                            .as_ref()
                            .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                            .transpose()?
                            .unwrap_or(RustType::Any);
                        params.push(Param {
                            name: param_name,
                            ty: Some(ty),
                        });
                    }
                    swc_ecma_ast::TsFnParam::Rest(rest) => {
                        let param_name = if let swc_ecma_ast::Pat::Ident(ident) = rest.arg.as_ref()
                        {
                            ident.id.sym.to_string()
                        } else {
                            format!("args{i}")
                        };
                        let type_ann = rest.type_ann.as_ref().or_else(|| {
                            if let swc_ecma_ast::Pat::Ident(ident) = rest.arg.as_ref() {
                                ident.type_ann.as_ref()
                            } else {
                                None
                            }
                        });
                        let ty = type_ann
                            .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                            .transpose()?
                            .unwrap_or(RustType::Vec(Box::new(RustType::Any)));
                        params.push(Param {
                            name: param_name,
                            ty: Some(ty),
                        });
                    }
                    _ => return Err(anyhow!("unsupported call signature parameter pattern")),
                }
            }

            let raw_return_type = sig
                .type_ann
                .as_ref()
                .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                .transpose()?;

            // Detect async (Promise<T> return) and unwrap
            let is_async = raw_return_type.as_ref().is_some_and(|ty| ty.is_promise());
            let return_type = raw_return_type
                .map(|ty| ty.unwrap_promise())
                .and_then(|ty| {
                    if matches!(ty, RustType::Unit) {
                        None
                    } else {
                        Some(ty)
                    }
                });

            // Merge call signature type params into the trait's type params
            if let Some(tpd) = sig.type_params.as_ref() {
                for p in &tpd.params {
                    let tp_name = p.name.sym.to_string();
                    if !merged_tp.iter().any(|tp| tp.name == tp_name) {
                        let constraint = p
                            .constraint
                            .as_ref()
                            .and_then(|c| convert_ts_type(c, synthetic, reg).ok());
                        merged_tp.push(TypeParam {
                            name: tp_name,
                            constraint,
                        });
                    }
                }
            }

            Ok(Method {
                vis: Visibility::Public,
                name: format!("call_{i}"),
                is_async,
                has_self: true,
                has_mut_self: false,
                params,
                return_type,
                body: None,
            })
        })();

        synthetic.restore_type_param_scope(prev_scope);
        methods.push(method?);
    }

    Ok(Item::Trait {
        vis,
        name: name.to_string(),
        type_params: merged_tp,
        supertraits: vec![],
        methods,
        associated_types: vec![],
    })
}

/// Converts a mixed interface (properties + methods) into struct + trait + impl.
///
/// - Properties → `Item::Struct` (named `{Name}Data`)
/// - Methods → `Item::Trait` (named `{Name}` — the interface name)
/// - Impl block → `Item::Impl` (implements `{Name}` for `{Name}Data`)
fn convert_interface_as_struct_and_trait(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<TypeParam>,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Vec<Item>> {
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    // Flatten parent fields from extends chain
    for parent_name in collect_extends_names(decl) {
        if let Some(TypeDef::Struct {
            fields: parent_fields,
            ..
        }) = reg.get(&parent_name)
        {
            for field in parent_fields {
                let sanitized = sanitize_field_name(&field.name);
                if !fields.iter().any(|f: &StructField| f.name == sanitized) {
                    fields.push(StructField {
                        vis: Some(Visibility::Public),
                        name: sanitized,
                        ty: field.ty.clone(),
                    });
                }
            }
        }
    }

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                fields.push(convert_property_signature(prop, synthetic, reg)?);
            }
            TsTypeElement::TsMethodSignature(method_sig) => {
                methods.push(convert_method_signature(method_sig, synthetic, reg)?);
            }
            _ => {
                // Skip unsupported members in mixed interfaces
            }
        }
    }

    let struct_name = format!("{name}Data");
    let supertraits = collect_extends_refs(decl, synthetic, reg);

    // trait 自身の型パラメータを型引数として TraitRef に変換
    // （例: interface Foo<T> → impl<T> Foo<T> for FooData<T>）
    let trait_type_args: Vec<RustType> = type_params
        .iter()
        .map(|p| RustType::Named {
            name: p.name.clone(),
            type_args: vec![],
        })
        .collect();

    let struct_item = Item::Struct {
        vis,
        name: struct_name.clone(),
        type_params: type_params.clone(),
        fields,
        is_unit_struct: false,
    };

    let trait_item = Item::Trait {
        vis,
        name: name.to_string(),
        type_params: type_params.clone(),
        supertraits,
        methods: methods.clone(),
        associated_types: vec![],
    };

    let impl_item = Item::Impl {
        struct_name,
        type_params: type_params.clone(),
        for_trait: Some(TraitRef {
            name: name.to_string(),
            type_args: trait_type_args,
        }),
        consts: vec![],
        methods,
    };

    Ok(vec![struct_item, trait_item, impl_item])
}

/// Converts an interface with method signatures into an IR [`Item::Trait`].
fn convert_interface_as_trait(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<TypeParam>,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Item> {
    let mut methods = Vec::new();

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsMethodSignature(method_sig) => {
                let method = convert_method_signature(method_sig, synthetic, reg)?;
                methods.push(method);
            }
            TsTypeElement::TsPropertySignature(_) => {
                // Properties in a trait interface are skipped for now.
                // Trait cannot have fields in Rust.
            }
            _ => {
                return Err(anyhow!(
                    "unsupported interface member (only property and method signatures are supported)"
                ));
            }
        }
    }

    let supertraits = collect_extends_refs(decl, synthetic, reg);

    Ok(Item::Trait {
        vis,
        name: name.to_string(),
        type_params,
        supertraits,
        methods,
        associated_types: vec![],
    })
}

/// Collects parent interface names from the `extends` clause of an interface declaration.
fn collect_extends_refs(
    decl: &TsInterfaceDecl,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Vec<TraitRef> {
    decl.extends
        .iter()
        .filter_map(|e| {
            if let swc_ecma_ast::Expr::Ident(ident) = e.expr.as_ref() {
                let type_args = e
                    .type_args
                    .as_ref()
                    .map(|ta| {
                        ta.params
                            .iter()
                            .filter_map(|t| convert_ts_type(t, synthetic, reg).ok())
                            .collect()
                    })
                    .unwrap_or_default();
                Some(TraitRef {
                    name: ident.sym.to_string(),
                    type_args,
                })
            } else {
                None
            }
        })
        .collect()
}

/// `collect_extends_refs` から名前のみを抽出する（TypeRegistry のフィールド展開用）。
fn collect_extends_names(decl: &TsInterfaceDecl) -> Vec<String> {
    decl.extends
        .iter()
        .filter_map(|e| {
            if let swc_ecma_ast::Expr::Ident(ident) = e.expr.as_ref() {
                Some(ident.sym.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Converts a [`TsMethodSignature`] into an IR [`Method`] (signature only, no body).
pub(super) fn convert_method_signature(
    sig: &TsMethodSignature,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Method> {
    let name = match sig.key.as_ref() {
        swc_ecma_ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => {
            return Err(anyhow!(
                "unsupported method signature key (only identifiers)"
            ))
        }
    };

    // I-383 T9: interface method 自身の generic 型パラメータを scope に append する。
    // 外部 (interface 自身) の type_params は `convert_interface_items` で既に push 済。
    // append-merge 意味論で両方アクティブになる。restore 漏れを防ぐため inner closure で
    // 本体処理を囲み、戻り値取得後に必ず restore する。
    let method_tp_names: Vec<String> = sig
        .type_params
        .as_ref()
        .map(|tpd| tpd.params.iter().map(|p| p.name.sym.to_string()).collect())
        .unwrap_or_default();
    let prev_scope = synthetic.push_type_param_scope(method_tp_names);

    let result = (|| -> Result<Method> {
        let mut params = Vec::new();
        for param in &sig.params {
            match param {
                swc_ecma_ast::TsFnParam::Ident(ident) => {
                    let param_name = ident.id.sym.to_string();
                    let ty = ident
                        .type_ann
                        .as_ref()
                        .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                        .transpose()?;
                    params.push(Param {
                        name: param_name,
                        ty,
                    });
                }
                _ => return Err(anyhow!("unsupported method parameter pattern")),
            }
        }

        let return_type = sig
            .type_ann
            .as_ref()
            .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
            .transpose()?;

        Ok(Method {
            vis: Visibility::Public,
            name,
            is_async: false,
            has_self: true,
            has_mut_self: false,
            params,
            return_type,
            body: None,
        })
    })();

    synthetic.restore_type_param_scope(prev_scope);
    result
}
