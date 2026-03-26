//! 合成 enum 登録 + any-narrowing enum の生成。

use std::collections::HashMap;

use swc_ecma_ast as ast;

use super::{TypeDef, TypeRegistry};
use crate::ir::{EnumVariant, Item, RustType};
use crate::pipeline::SyntheticTypeRegistry;

/// Registers enum items generated during type conversion into the TypeRegistry.
pub(super) fn register_extra_enums(reg: &mut TypeRegistry, synthetic: &SyntheticTypeRegistry) {
    for item in synthetic.all_items() {
        register_single_enum(reg, item);
    }
}

/// Registers a single enum item in the TypeRegistry.
fn register_single_enum(reg: &mut TypeRegistry, item: &Item) {
    if let Item::Enum { name, variants, .. } = item {
        let variant_names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
        register_enum_typedef(reg, name, &variant_names);
    }
}

/// Registers an enum TypeDef by name and variant names.
fn register_single_enum_by_name(reg: &mut TypeRegistry, name: &str, variants: Vec<EnumVariant>) {
    let variant_names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
    register_enum_typedef(reg, name, &variant_names);
}

/// Internal: creates and registers an enum TypeDef.
fn register_enum_typedef(reg: &mut TypeRegistry, name: &str, variant_names: &[String]) {
    reg.register(
        name.to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: variant_names.to_vec(),
            string_values: HashMap::new(),
            tag_field: None,
            variant_fields: HashMap::new(),
        },
    );
}

/// Registers any-narrowing enum types for `any`-typed function parameters.
///
/// Scans the function body for typeof checks on `any`-typed parameters and registers
/// the generated enum types in the TypeRegistry so that `resolve_typeof_to_enum_variant`
/// can find them during statement conversion.
pub(super) fn register_any_narrowing_enums(
    reg: &mut TypeRegistry,
    fn_name: &str,
    func_def: &TypeDef,
    body: &ast::BlockStmt,
) {
    use crate::transformer::any_narrowing::{
        build_any_enum_variants, collect_any_constraints, collect_any_local_var_names,
    };

    let TypeDef::Function { params, .. } = func_def else {
        return;
    };

    // Collect any-typed parameter names
    let mut any_names: Vec<String> = params
        .iter()
        .filter(|(_, ty)| matches!(ty, RustType::Any))
        .map(|(name, _)| name.clone())
        .collect();

    // Also collect any-typed local variable names
    any_names.extend(collect_any_local_var_names(body));

    if any_names.is_empty() {
        return;
    }

    let constraints = collect_any_constraints(body, &any_names);
    for (param_name, constraint) in &constraints {
        if constraint.is_empty() {
            continue;
        }
        let variants = build_any_enum_variants(constraint);
        let enum_name = format!(
            "{}{}Type",
            crate::transformer::any_narrowing::to_pascal_case(fn_name),
            crate::transformer::any_narrowing::to_pascal_case(param_name)
        );
        register_single_enum_by_name(reg, &enum_name, variants);
    }
}

/// Expression-body variant of `register_any_narrowing_enums`.
pub(super) fn register_any_narrowing_enums_from_expr(
    reg: &mut TypeRegistry,
    fn_name: &str,
    func_def: &TypeDef,
    expr: &ast::Expr,
) {
    use crate::transformer::any_narrowing::{
        build_any_enum_variants, collect_any_constraints_from_expr,
    };

    let TypeDef::Function { params, .. } = func_def else {
        return;
    };

    let any_names: Vec<String> = params
        .iter()
        .filter(|(_, ty)| matches!(ty, RustType::Any))
        .map(|(name, _)| name.clone())
        .collect();

    if any_names.is_empty() {
        return;
    }

    let constraints = collect_any_constraints_from_expr(expr, &any_names);
    for (param_name, constraint) in &constraints {
        if constraint.is_empty() {
            continue;
        }
        let variants = build_any_enum_variants(constraint);
        let enum_name = format!(
            "{}{}Type",
            crate::transformer::any_narrowing::to_pascal_case(fn_name),
            crate::transformer::any_narrowing::to_pascal_case(param_name)
        );
        register_single_enum_by_name(reg, &enum_name, variants);
    }
}
