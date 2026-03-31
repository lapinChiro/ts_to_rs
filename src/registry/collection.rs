//! 2-pass 型収集ロジック。
//!
//! Pass 1 で型名をプレースホルダー登録し、Pass 2 でフィールド型を完全解決する。

use std::collections::HashMap;

use swc_ecma_ast as ast;

use super::{ConstElement, ConstField, MethodSignature, TypeDef, TypeRegistry};
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
                    type_params: vec![],
                    params: vec![],
                    return_type: None,
                    has_rest: false,
                },
            );
        }
        ast::Decl::Var(var_decl) => {
            for d in &var_decl.decls {
                let name = match &d.name {
                    ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                    _ => continue,
                };
                if let Some(init) = &d.init {
                    if let ast::Expr::Arrow(_) = init.as_ref() {
                        reg.register(
                            name,
                            TypeDef::Function {
                                type_params: vec![],
                                params: vec![],
                                return_type: None,
                                has_rest: false,
                            },
                        );
                        continue;
                    }
                }
                // `as const` or type-annotated const: register placeholder
                if is_registrable_const_decl(d) {
                    reg.register(
                        name,
                        TypeDef::ConstValue {
                            fields: vec![],
                            elements: vec![],
                            type_ref_name: None,
                        },
                    );
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

/// `const` 宣言が TypeRegistry に登録すべきかどうか判定する。
///
/// 以下のいずれかに該当する場合に true:
/// - `as const` アサーション付き（`const X = [...] as const`）
/// - 明示的な型注釈付き（`const X: Type = ...`）
fn is_registrable_const_decl(d: &ast::VarDeclarator) -> bool {
    // Check for type annotation
    if let ast::Pat::Ident(ident) = &d.name {
        if ident.type_ann.is_some() {
            return true;
        }
    }
    // Check for `as const` assertion
    if let Some(init) = &d.init {
        if matches!(init.as_ref(), ast::Expr::TsConstAssertion(_)) {
            return true;
        }
    }
    false
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
                let sigs =
                    super::interfaces::collect_interface_signatures(iface, lookup, synthetic);
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
                        methods: sigs.methods,
                        constructor: sigs.constructor,
                        call_signatures: sigs.call_signatures,
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
                            constructor: None,
                            call_signatures: vec![],
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
                reg.register(fn_name, func_def);
            }
        }
        ast::Decl::Var(var_decl) => {
            for d in &var_decl.decls {
                let name = match &d.name {
                    ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                    _ => continue,
                };

                // Arrow function: collect as Function
                if let Some(init) = &d.init {
                    if let ast::Expr::Arrow(arrow) = init.as_ref() {
                        if let Ok(func_def) = super::functions::collect_arrow_def_with_extras(
                            arrow, lookup, synthetic,
                        ) {
                            reg.register(name, func_def);
                        }
                        continue;
                    }
                }

                // `as const` or type-annotated const: collect as ConstValue
                if let Some(const_value) = collect_const_value_def(d, lookup, synthetic) {
                    reg.register(name, const_value);
                }
            }
        }
        ast::Decl::Class(class) => {
            let def = collect_class_info(class, lookup, synthetic);
            if let TypeDef::Struct {
                ref fields,
                ref methods,
                ref constructor,
                ..
            } = def
            {
                if !fields.is_empty() || !methods.is_empty() || constructor.is_some() {
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
    let mut constructor_sigs: Vec<MethodSignature> = Vec::new();

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
            ast::ClassMember::PrivateProp(prop) => {
                let name = prop.key.name.to_string();
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_ts_type(&ann.type_ann, synthetic, lookup) {
                        fields.push((name, ty));
                    }
                }
            }
            ast::ClassMember::Constructor(ctor) => {
                let params: Vec<(String, RustType)> = ctor
                    .params
                    .iter()
                    .filter_map(|p| match p {
                        ast::ParamOrTsParamProp::Param(param) => {
                            let ident = match &param.pat {
                                ast::Pat::Ident(ident) => ident,
                                _ => return None,
                            };
                            let ty = ident.type_ann.as_ref().and_then(|ann| {
                                convert_ts_type(&ann.type_ann, synthetic, lookup).ok()
                            })?;
                            Some((ident.id.sym.to_string(), ty))
                        }
                        ast::ParamOrTsParamProp::TsParamProp(param_prop) => {
                            let ident = match &param_prop.param {
                                ast::TsParamPropParam::Ident(ident) => ident,
                                _ => return None,
                            };
                            let ty = ident.type_ann.as_ref().and_then(|ann| {
                                convert_ts_type(&ann.type_ann, synthetic, lookup).ok()
                            })?;
                            Some((ident.id.sym.to_string(), ty))
                        }
                    })
                    .collect();
                let has_rest = ctor.params.iter().any(|p| {
                    matches!(
                        p,
                        ast::ParamOrTsParamProp::Param(param) if matches!(&param.pat, ast::Pat::Rest(_))
                    )
                });
                constructor_sigs.push(MethodSignature {
                    params,
                    return_type: None,
                    has_rest,
                });
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
                        super::functions::extract_pat_param(&param.pat, lookup, synthetic)
                    })
                    .collect();
                let return_type = method
                    .function
                    .return_type
                    .as_ref()
                    .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());
                let has_rest = method
                    .function
                    .params
                    .iter()
                    .any(|param| matches!(&param.pat, ast::Pat::Rest(_)));
                methods.entry(name).or_default().push(MethodSignature {
                    params,
                    return_type,
                    has_rest,
                });
            }
            _ => {}
        }
    }

    let type_params = collect_type_params(class.class.type_params.as_deref(), lookup, synthetic);
    let constructor = if constructor_sigs.is_empty() {
        None
    } else {
        Some(constructor_sigs)
    };
    TypeDef::Struct {
        type_params,
        fields,
        methods,
        constructor,
        call_signatures: vec![],
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

/// TsTypeRef からフィールドを解決する。
///
/// 型引数付きの参照（`Partial<Body>` 等）の場合は、ジェネリクスを具体型でインスタンス化してから
/// フィールドを取得する。SyntheticTypeRegistry のインライン構造体も解決対象。
fn resolve_type_ref_fields(
    type_ref: &ast::TsTypeRef,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<Vec<(String, RustType)>> {
    let name = match &type_ref.type_name {
        ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };

    // Convert type arguments if present
    let type_args: Vec<RustType> = type_ref
        .type_params
        .as_ref()
        .map(|params| {
            params
                .params
                .iter()
                .filter_map(|p| {
                    crate::pipeline::type_converter::convert_ts_type(p, synthetic, reg).ok()
                })
                .collect()
        })
        .unwrap_or_default();

    // Resolve from registry (with instantiation for generics)
    if let Some(TypeDef::Struct { fields, .. }) = reg.get(&name) {
        if !type_args.is_empty() {
            if let Some(TypeDef::Struct { fields, .. }) = reg.instantiate(&name, &type_args) {
                return Some(fields);
            }
        }
        return Some(fields.clone());
    }

    None
}

/// TsTypeLit のプロパティシグネチャからフィールドを収集する。
fn collect_type_lit_fields(
    lit: &ast::TsTypeLit,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Vec<(String, RustType)> {
    lit.members
        .iter()
        .filter_map(|member| {
            if let ast::TsTypeElement::TsPropertySignature(prop) = member {
                super::interfaces::collect_property_signature(prop, reg, synthetic)
            } else {
                None
            }
        })
        .collect()
}

/// type alias (オブジェクト型・intersection 型・型参照) のフィールドを収集する。
///
/// 対応する `TsType` バリアント:
/// - `TsTypeLit`: `type X = { a: number; b: string }`
/// - `TsIntersectionType`: `type X = A & B`（各メンバーから TsTypeLit / TsTypeRef のフィールドをマージ）
/// - `TsTypeRef`: `type X = Partial<Body>`（registry からフィールドを解決）
fn collect_type_alias_fields(
    alias: &ast::TsTypeAliasDecl,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<Vec<(String, RustType)>> {
    match alias.type_ann.as_ref() {
        ast::TsType::TsTypeLit(lit) => Some(collect_type_lit_fields(lit, reg, synthetic)),
        // Intersection type: `type Person = Named & Aged` → merge fields from all members
        ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(intersection),
        ) => {
            let mut fields = Vec::new();
            for ty in &intersection.types {
                match ty.as_ref() {
                    ast::TsType::TsTypeLit(lit) => {
                        fields.extend(collect_type_lit_fields(lit, reg, synthetic));
                    }
                    ast::TsType::TsTypeRef(type_ref) => {
                        if let Some(ref_fields) = resolve_type_ref_fields(type_ref, reg, synthetic)
                        {
                            fields.extend(ref_fields);
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
        // Type reference: `type X = Partial<Body>` → resolve fields from registry
        ast::TsType::TsTypeRef(type_ref) => resolve_type_ref_fields(type_ref, reg, synthetic),
        _ => None,
    }
}

/// `as const` 宣言または型注釈付き const 宣言から `TypeDef::ConstValue` を構築する。
///
/// 対象パターン:
/// - `const X = ['a', 'b'] as const` → 文字列リテラル配列
/// - `const X = { key: 'value' } as const` → オブジェクトリテラル
/// - `const X: Type = expr` → 型注釈から構築
fn collect_const_value_def(
    d: &ast::VarDeclarator,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<TypeDef> {
    // Case 1: Type annotation — resolve fields from the annotated type
    if let ast::Pat::Ident(ident) = &d.name {
        if let Some(type_ann) = &ident.type_ann {
            return collect_const_value_from_type_annotation(&type_ann.type_ann, lookup, synthetic);
        }
    }

    // Case 2: `as const` assertion
    let init = d.init.as_ref()?;
    if let ast::Expr::TsConstAssertion(assertion) = init.as_ref() {
        return collect_const_value_from_as_const(&assertion.expr);
    }

    None
}

/// 型注釈から `ConstValue` を構築する。
///
/// 型注釈がオブジェクト型リテラルの場合、フィールドを直接変換する。
/// 型参照（`const x: MyType = ...`）の場合は、参照名を保持した ConstValue を生成し、
/// 後続の型解決（TsTypeQuery ハンドラ）で参照先から間接的にフィールドを取得する。
fn collect_const_value_from_type_annotation(
    type_ann: &ast::TsType,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<TypeDef> {
    // Inline object type → convert fields directly
    if let ast::TsType::TsTypeLit(lit) = type_ann {
        let mut fields = Vec::new();
        for member in &lit.members {
            if let ast::TsTypeElement::TsPropertySignature(prop) = member {
                let field_name = match &*prop.key {
                    ast::Expr::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                let field_type = prop
                    .type_ann
                    .as_ref()
                    .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok())
                    .unwrap_or(RustType::Any);
                fields.push(ConstField {
                    name: field_name,
                    ty: field_type,
                    string_literal_value: None,
                });
            }
        }
        if !fields.is_empty() {
            return Some(TypeDef::ConstValue {
                fields,
                elements: vec![],
                type_ref_name: None,
            });
        }
    }

    // Type reference → store the referenced type name for redirect at typeof resolution time
    if let ast::TsType::TsTypeRef(type_ref) = type_ann {
        let ref_name = match &type_ref.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => Some(ident.sym.to_string()),
            _ => None,
        };
        return Some(TypeDef::ConstValue {
            fields: vec![],
            elements: vec![],
            type_ref_name: ref_name,
        });
    }

    None
}

/// `as const` アサーション内の式から `ConstValue` を構築する。
fn collect_const_value_from_as_const(expr: &ast::Expr) -> Option<TypeDef> {
    match expr {
        ast::Expr::Array(array_lit) => {
            let elements = extract_const_array_elements(array_lit);
            if elements.is_empty() {
                return None;
            }
            Some(TypeDef::ConstValue {
                fields: vec![],
                elements,
                type_ref_name: None,
            })
        }
        ast::Expr::Object(obj_lit) => {
            let fields = extract_const_object_fields(obj_lit);
            if fields.is_empty() {
                return None;
            }
            Some(TypeDef::ConstValue {
                fields,
                elements: vec![],
                type_ref_name: None,
            })
        }
        _ => None,
    }
}

/// 配列リテラルからリテラル要素を抽出する。
///
/// 各要素のリテラル型と、文字列リテラルの場合はその値を保持する。
/// すべての要素がリテラルの場合のみ返す。
fn extract_const_array_elements(array_lit: &ast::ArrayLit) -> Vec<ConstElement> {
    let mut elements = Vec::new();
    for elem in &array_lit.elems {
        match elem {
            Some(ast::ExprOrSpread { expr, .. }) => match expr.as_ref() {
                ast::Expr::Lit(ast::Lit::Str(s)) => {
                    elements.push(ConstElement {
                        ty: RustType::String,
                        string_literal_value: Some(s.value.to_string_lossy().into_owned()),
                    });
                }
                ast::Expr::Lit(ast::Lit::Num(_)) => {
                    elements.push(ConstElement {
                        ty: RustType::F64,
                        string_literal_value: None,
                    });
                }
                ast::Expr::Lit(ast::Lit::Bool(_)) => {
                    elements.push(ConstElement {
                        ty: RustType::Bool,
                        string_literal_value: None,
                    });
                }
                _ => return vec![],
            },
            None => return vec![],
        }
    }
    elements
}

/// オブジェクトリテラルからフィールド情報を抽出する。
fn extract_const_object_fields(obj_lit: &ast::ObjectLit) -> Vec<ConstField> {
    let mut fields = Vec::new();
    for prop in &obj_lit.props {
        if let ast::PropOrSpread::Prop(prop) = prop {
            if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                let field_name = match &kv.key {
                    ast::PropName::Ident(id) => id.sym.to_string(),
                    ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                    _ => continue,
                };
                let (field_type, string_value) = match kv.value.as_ref() {
                    ast::Expr::Lit(ast::Lit::Str(s)) => (
                        RustType::String,
                        Some(s.value.to_string_lossy().into_owned()),
                    ),
                    ast::Expr::Lit(ast::Lit::Num(_)) => (RustType::F64, None),
                    ast::Expr::Lit(ast::Lit::Bool(_)) => (RustType::Bool, None),
                    _ => (RustType::Any, None),
                };
                fields.push(ConstField {
                    name: field_name,
                    ty: field_type,
                    string_literal_value: string_value,
                });
            }
        }
    }
    fields
}
