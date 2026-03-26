//! 2-pass 型収集ロジック。
//!
//! Pass 1 で型名をプレースホルダー登録し、Pass 2 でフィールド型を完全解決する。

use std::collections::HashMap;

use swc_ecma_ast as ast;

use super::{MethodSignature, TypeDef, TypeRegistry};
use crate::ir::{RustType, TypeParam};
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::SyntheticTypeRegistry;

/// Pass 1: 宣言から型名だけをプレースホルダーとして登録する。
///
/// フィールド型の解決は行わず、型名の存在だけを記録する。
/// これにより Pass 2 で前方参照を解決できる。
pub(super) fn collect_type_name(reg: &mut TypeRegistry, decl: &ast::Decl) {
    match decl {
        ast::Decl::TsInterface(iface) => {
            reg.register(
                iface.id.sym.to_string(),
                TypeDef::new_interface(vec![], vec![], HashMap::new(), vec![]),
            );
        }
        ast::Decl::TsTypeAlias(alias) => {
            reg.register(
                alias.id.sym.to_string(),
                TypeDef::new_struct(vec![], HashMap::new(), vec![]),
            );
        }
        ast::Decl::TsEnum(ts_enum) => {
            reg.register(
                ts_enum.id.sym.to_string(),
                TypeDef::Enum {
                    type_params: vec![],
                    variants: vec![],
                    string_values: HashMap::new(),
                    tag_field: None,
                    variant_fields: HashMap::new(),
                },
            );
        }
        ast::Decl::Fn(fn_decl) => {
            reg.register(
                fn_decl.ident.sym.to_string(),
                TypeDef::Function {
                    params: vec![],
                    return_type: None,
                    has_rest: false,
                },
            );
        }
        ast::Decl::Var(var_decl) => {
            for d in &var_decl.decls {
                if let Some(init) = &d.init {
                    if let ast::Expr::Arrow(_) = init.as_ref() {
                        let name = match &d.name {
                            ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                            _ => continue,
                        };
                        reg.register(
                            name,
                            TypeDef::Function {
                                params: vec![],
                                return_type: None,
                                has_rest: false,
                            },
                        );
                    }
                }
            }
        }
        ast::Decl::Class(class) => {
            reg.register(
                class.ident.sym.to_string(),
                TypeDef::new_struct(vec![], HashMap::new(), vec![]),
            );
        }
        _ => {}
    }
}

/// Pass 2: 個々の宣言から型情報を完全に収集する。
///
/// `lookup` には Pass 1 で登録された全型名が含まれており、
/// `convert_ts_type` での型解決に使用される。
pub(super) fn collect_decl(
    reg: &mut TypeRegistry,
    decl: &ast::Decl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) {
    match decl {
        ast::Decl::TsInterface(iface) => {
            if let Ok(fields) =
                super::interfaces::collect_interface_fields(iface, lookup, synthetic)
            {
                let methods =
                    super::interfaces::collect_interface_methods(iface, lookup, synthetic);
                let type_params =
                    collect_type_params(iface.type_params.as_deref(), lookup, synthetic);
                let extends: Vec<String> = iface
                    .extends
                    .iter()
                    .filter_map(|e| {
                        if let ast::Expr::Ident(ident) = e.expr.as_ref() {
                            Some(ident.sym.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                let name = iface.id.sym.to_string();
                reg.register(
                    name,
                    TypeDef::Struct {
                        type_params,
                        fields,
                        methods,
                        extends,
                        is_interface: true,
                    },
                );
            }
        }
        ast::Decl::TsTypeAlias(alias) => {
            if let Some(enum_def) = super::unions::try_collect_string_literal_union(alias) {
                reg.register(alias.id.sym.to_string(), enum_def);
            } else if let Some(mut enum_def) =
                super::unions::try_collect_discriminated_union(alias, lookup, synthetic)
            {
                // DU enum に型パラメータを設定（try_collect_discriminated_union は
                // 型パラメータの概念に無関係なため、呼び出し元で上書きする）
                let type_params =
                    collect_type_params(alias.type_params.as_deref(), lookup, synthetic);
                if let TypeDef::Enum {
                    type_params: ref mut tp,
                    ..
                } = enum_def
                {
                    *tp = type_params;
                }
                reg.register(alias.id.sym.to_string(), enum_def);
            } else if let Some(func_def) =
                super::functions::try_collect_fn_type_alias(alias, lookup, synthetic)
            {
                reg.register(alias.id.sym.to_string(), func_def);
            } else {
                // Intersection types need pass-2 resolved types (e.g., `type Person = Named & Aged`
                // requires Named and Aged to have their fields already resolved).
                // Use `reg` which accumulates resolved types during pass 2.
                let fields = collect_type_alias_fields(alias, reg, synthetic);
                if let Some(fields) = fields {
                    let type_params =
                        collect_type_params(alias.type_params.as_deref(), lookup, synthetic);
                    reg.register(
                        alias.id.sym.to_string(),
                        TypeDef::Struct {
                            type_params,
                            fields,
                            methods: HashMap::new(),
                            extends: vec![],
                            is_interface: false,
                        },
                    );
                }
            }
        }
        ast::Decl::TsEnum(ts_enum) => {
            let variants = ts_enum
                .members
                .iter()
                .map(|m| match &m.id {
                    ast::TsEnumMemberId::Ident(ident) => ident.sym.to_string(),
                    ast::TsEnumMemberId::Str(s) => s.value.to_string_lossy().into_owned(),
                })
                .collect();
            reg.register(
                ts_enum.id.sym.to_string(),
                TypeDef::Enum {
                    type_params: vec![],
                    variants,
                    string_values: HashMap::new(),
                    tag_field: None,
                    variant_fields: HashMap::new(),
                },
            );
        }
        ast::Decl::Fn(fn_decl) => {
            if let Ok(func_def) =
                super::functions::collect_fn_def_with_extras(&fn_decl.function, lookup, synthetic)
            {
                let fn_name = fn_decl.ident.sym.to_string();
                // Register any-narrowing enums for `any`-typed parameters with typeof checks
                if let Some(body) = &fn_decl.function.body {
                    super::enums::register_any_narrowing_enums(reg, &fn_name, &func_def, body);
                }
                reg.register(fn_name, func_def);
            }
        }
        ast::Decl::Var(var_decl) => {
            // const f = (x: number): string => ...
            for d in &var_decl.decls {
                if let Some(init) = &d.init {
                    if let ast::Expr::Arrow(arrow) = init.as_ref() {
                        let name = match &d.name {
                            ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                            _ => continue,
                        };
                        if let Ok(func_def) = super::functions::collect_arrow_def_with_extras(
                            arrow, lookup, synthetic,
                        ) {
                            // Register any-narrowing enums for arrow function any-typed params
                            match arrow.body.as_ref() {
                                ast::BlockStmtOrExpr::BlockStmt(body) => {
                                    super::enums::register_any_narrowing_enums(
                                        reg, &name, &func_def, body,
                                    );
                                }
                                ast::BlockStmtOrExpr::Expr(expr) => {
                                    super::enums::register_any_narrowing_enums_from_expr(
                                        reg, &name, &func_def, expr,
                                    );
                                }
                            }
                            reg.register(name, func_def);
                        }
                    }
                }
            }
        }
        ast::Decl::Class(class) => {
            let def = collect_class_info(class, lookup, synthetic);
            if let TypeDef::Struct {
                ref fields,
                ref methods,
                ..
            } = def
            {
                if !fields.is_empty() || !methods.is_empty() {
                    reg.register(class.ident.sym.to_string(), def);
                }
            }
        }
        _ => {}
    }
}

/// クラス宣言からフィールドとメソッドシグネチャを収集し、`TypeDef::Struct` を返す。
fn collect_class_info(
    class: &ast::ClassDecl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> TypeDef {
    let mut fields = Vec::new();
    let mut methods: HashMap<String, Vec<MethodSignature>> = HashMap::new();

    for member in &class.class.body {
        match member {
            ast::ClassMember::ClassProp(prop) => {
                let name = match &prop.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_ts_type(&ann.type_ann, synthetic, lookup) {
                        fields.push((name, ty));
                    }
                }
            }
            ast::ClassMember::Method(method) => {
                let name = match &method.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                if let Some(func) = &method.function.body {
                    let _ = func; // body exists, collect params
                }
                let params: Vec<(String, RustType)> = method
                    .function
                    .params
                    .iter()
                    .filter_map(|param| {
                        let ident = match &param.pat {
                            ast::Pat::Ident(ident) => ident,
                            _ => return None,
                        };
                        let ty = ident.type_ann.as_ref().and_then(|ann| {
                            convert_ts_type(&ann.type_ann, synthetic, lookup).ok()
                        })?;
                        Some((ident.id.sym.to_string(), ty))
                    })
                    .collect();
                let return_type = method
                    .function
                    .return_type
                    .as_ref()
                    .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());
                methods.entry(name).or_default().push(MethodSignature {
                    params,
                    return_type,
                });
            }
            _ => {}
        }
    }

    let type_params = collect_type_params(class.class.type_params.as_deref(), lookup, synthetic);
    TypeDef::Struct {
        type_params,
        fields,
        methods,
        extends: vec![],
        is_interface: false,
    }
}

/// TS の型パラメータ宣言から TypeParam を収集する。
pub fn collect_type_params(
    decl: Option<&ast::TsTypeParamDecl>,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Vec<TypeParam> {
    decl.map(|d| {
        d.params
            .iter()
            .map(|p| TypeParam {
                name: p.name.sym.to_string(),
                constraint: p
                    .constraint
                    .as_ref()
                    .and_then(|c| convert_ts_type(c, synthetic, lookup).ok()),
            })
            .collect()
    })
    .unwrap_or_default()
}

/// type alias (オブジェクト型・intersection 型) のフィールドを収集する。
fn collect_type_alias_fields(
    alias: &ast::TsTypeAliasDecl,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<Vec<(String, RustType)>> {
    match alias.type_ann.as_ref() {
        ast::TsType::TsTypeLit(lit) => {
            let mut fields = Vec::new();
            for member in &lit.members {
                if let ast::TsTypeElement::TsPropertySignature(prop) = member {
                    if let Some((name, ty)) =
                        super::interfaces::collect_property_signature(prop, reg, synthetic)
                    {
                        fields.push((name, ty));
                    }
                }
            }
            Some(fields)
        }
        // Intersection type: `type Person = Named & Aged` → merge fields from all members
        ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(intersection),
        ) => {
            let mut fields = Vec::new();
            for ty in &intersection.types {
                match ty.as_ref() {
                    ast::TsType::TsTypeLit(lit) => {
                        for member in &lit.members {
                            if let ast::TsTypeElement::TsPropertySignature(prop) = member {
                                if let Some(field) = super::interfaces::collect_property_signature(
                                    prop, reg, synthetic,
                                ) {
                                    fields.push(field);
                                }
                            }
                        }
                    }
                    ast::TsType::TsTypeRef(type_ref) => {
                        if let ast::TsEntityName::Ident(ident) = &type_ref.type_name {
                            if let Some(TypeDef::Struct {
                                fields: ref_fields, ..
                            }) = reg.get(ident.sym.as_ref())
                            {
                                fields.extend(ref_fields.iter().cloned());
                            }
                        }
                    }
                    _ => {}
                }
            }
            if fields.is_empty() {
                None
            } else {
                Some(fields)
            }
        }
        _ => None,
    }
}
