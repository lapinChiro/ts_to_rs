//! 2-pass 型収集ロジック。
//!
//! Pass 1 で型名をプレースホルダー登録し、Pass 2 でフィールド型を完全解決する。
//!
//! Collection 関数は SWC AST → TsTypeInfo の変換を先行し、TsTypeInfo を起点として
//! resolve 関数を使用する。SWC AST を直接解析するアドホックなロジックは持たない。
//! TypeDef<TsTypeInfo> として自然に表現できる型は `resolve_struct_for_registry` に委譲し、
//! できない型（intersection, utility type ref 等）は resolve 関数を直接使用して
//! TypeDef<RustType> を構築する。

use std::collections::HashMap;

use swc_ecma_ast as ast;

use super::{ConstElement, ConstField, FieldDef, MethodSignature, ParamDef, TypeDef, TypeRegistry};
use crate::ir::{RustType, TypeParam};
use crate::pipeline::SyntheticTypeRegistry;
use crate::ts_type_info::resolve::{resolve_ts_type, resolve_type_params, resolve_typedef};
use crate::ts_type_info::{
    convert_to_ts_type_info, TsFnSigInfo, TsMethodInfo, TsTypeInfo, TsTypeLiteralInfo,
};

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
/// 大部分のブランチ（interface, DU, function 等）は `TypeDef<TsTypeInfo>` を構築し、
/// `resolve_typedef` で `TypeDef<RustType>` に変換してから registry に登録する。
///
/// type alias の else ブランチは `convert_to_ts_type_info` で TsTypeInfo に変換後、
/// 型の形態に応じた 3 パスで処理する:
/// - パス A (TypeLiteral): `build_struct_from_type_literal` → `resolve_struct_for_registry`
/// - パス B (Intersection): `resolve_intersection_for_registry`
/// - パス C (TypeRef / その他): `resolve_type_ref_for_registry`
///
/// `lookup` には Pass 1 で登録された全型名が含まれており、型解決に使用される。
/// `reg` は Pass 2 で蓄積中の registry で、intersection/typeref パスが参照する。
pub(super) fn collect_decl(
    reg: &mut TypeRegistry,
    decl: &ast::Decl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) {
    match decl {
        ast::Decl::TsInterface(iface) => {
            if let Ok(fields) = super::interfaces::collect_interface_fields(iface) {
                let sigs = super::interfaces::collect_interface_signatures(iface);
                let type_params = collect_type_params(iface.type_params.as_deref());
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
                let ts_def: TypeDef<TsTypeInfo> = TypeDef::Struct {
                    type_params,
                    fields,
                    methods: sigs.methods,
                    constructor: sigs.constructor,
                    call_signatures: sigs.call_signatures,
                    extends,
                    is_interface: true,
                };
                if let Ok(resolved) = resolve_typedef(ts_def, lookup, synthetic) {
                    reg.register(name, resolved);
                }
            }
        }
        ast::Decl::TsTypeAlias(alias) => {
            if let Some(enum_def) = super::unions::try_collect_string_literal_union(alias) {
                // TypeDef<TsTypeInfo>::Enum → resolve で PascalCase 適用
                if let Ok(resolved) = resolve_typedef(enum_def, lookup, synthetic) {
                    reg.register(alias.id.sym.to_string(), resolved);
                }
            } else if let Some(mut enum_def) = super::unions::try_collect_discriminated_union(alias)
            {
                // DU enum に型パラメータを設定
                let type_params = collect_type_params(alias.type_params.as_deref());
                if let TypeDef::Enum {
                    type_params: ref mut tp,
                    ..
                } = enum_def
                {
                    *tp = type_params;
                }
                if let Ok(resolved) = resolve_typedef(enum_def, lookup, synthetic) {
                    reg.register(alias.id.sym.to_string(), resolved);
                }
            } else if let Some(func_def) = super::functions::try_collect_fn_type_alias(alias) {
                if let Ok(resolved) = resolve_typedef(func_def, lookup, synthetic) {
                    reg.register(alias.id.sym.to_string(), resolved);
                }
            } else {
                // TsTypeInfo を中間表現として統一し、型形態に応じた resolve パスを選択。
                // SWC AST のアドホック解析は行わない。
                let name = alias.id.sym.to_string();
                let type_params = collect_type_params(alias.type_params.as_deref());

                match convert_to_ts_type_info(alias.type_ann.as_ref()) {
                    // パス A: TypeLiteral → TsTypeInfo 変換 → 個別 resolve
                    // resolve_typedef は monomorphization を行うため使用しない。
                    // registry は generic 定義を TypeVar のまま保持する。
                    Ok(TsTypeInfo::TypeLiteral(ref lit)) => {
                        let ts_def = build_struct_from_type_literal(lit, type_params);
                        if let Ok(resolved) = resolve_struct_for_registry(ts_def, lookup, synthetic)
                        {
                            reg.register(name, resolved);
                        }
                    }
                    // パス B: Intersection → resolve 関数で全 variant を処理
                    Ok(TsTypeInfo::Intersection(ref members)) => {
                        if let Some(resolved) =
                            resolve_intersection_for_registry(members, &type_params, reg, synthetic)
                        {
                            reg.register(name, resolved);
                        }
                    }
                    // パス C: TypeRef / その他 → resolve_ts_type で解決
                    Ok(ref info) => {
                        if let Some(resolved) =
                            resolve_type_ref_for_registry(info, &type_params, reg, synthetic)
                        {
                            reg.register(name, resolved);
                        }
                    }
                    // convert_to_ts_type_info 失敗 → 未対応型。登録スキップ
                    Err(_) => {}
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
            if let Ok(func_def) = super::functions::collect_fn_def_with_extras(&fn_decl.function) {
                if let Ok(resolved) = resolve_typedef(func_def, lookup, synthetic) {
                    let fn_name = fn_decl.ident.sym.to_string();
                    reg.register(fn_name, resolved);
                }
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
                        if let Ok(func_def) = super::functions::collect_arrow_def_with_extras(arrow)
                        {
                            if let Ok(resolved) = resolve_typedef(func_def, lookup, synthetic) {
                                reg.register(name, resolved);
                            }
                        }
                        continue;
                    }
                }

                // `as const` or type-annotated const: collect as ConstValue
                if let Some(const_value) = collect_const_value_def(d) {
                    if let Ok(resolved) = resolve_typedef(const_value, lookup, synthetic) {
                        reg.register(name, resolved);
                    }
                }
            }
        }
        ast::Decl::Class(class) => {
            let ts_def = collect_class_info(class);
            if let TypeDef::Struct {
                ref fields,
                ref methods,
                ref constructor,
                ..
            } = ts_def
            {
                if !fields.is_empty() || !methods.is_empty() || constructor.is_some() {
                    if let Ok(resolved) = resolve_typedef(ts_def, lookup, synthetic) {
                        reg.register(class.ident.sym.to_string(), resolved);
                    }
                }
            }
        }
        _ => {}
    }
}

/// クラス宣言からフィールドとメソッドシグネチャを収集し、`TypeDef::Struct<TsTypeInfo>` を返す。
fn collect_class_info(class: &ast::ClassDecl) -> TypeDef<TsTypeInfo> {
    let mut fields = Vec::new();
    let mut methods: HashMap<String, Vec<MethodSignature<TsTypeInfo>>> = HashMap::new();
    let mut constructor_sigs: Vec<MethodSignature<TsTypeInfo>> = Vec::new();

    for member in &class.class.body {
        match member {
            ast::ClassMember::ClassProp(prop) => {
                let name = match &prop.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_to_ts_type_info(&ann.type_ann) {
                        fields.push(FieldDef {
                            name,
                            ty,
                            optional: prop.is_optional,
                        });
                    }
                }
            }
            ast::ClassMember::PrivateProp(prop) => {
                let name = prop.key.name.to_string();
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_to_ts_type_info(&ann.type_ann) {
                        fields.push(FieldDef {
                            name,
                            ty,
                            optional: prop.is_optional,
                        });
                    }
                }
            }
            ast::ClassMember::Constructor(ctor) => {
                let params: Vec<ParamDef<TsTypeInfo>> = ctor
                    .params
                    .iter()
                    .filter_map(|p| match p {
                        ast::ParamOrTsParamProp::Param(param) => {
                            let ident = match &param.pat {
                                ast::Pat::Ident(ident) => ident,
                                _ => return None,
                            };
                            let ty = ident
                                .type_ann
                                .as_ref()
                                .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok())?;
                            Some(ParamDef {
                                name: ident.id.sym.to_string(),
                                ty,
                                optional: ident.id.optional,
                                has_default: false,
                            })
                        }
                        ast::ParamOrTsParamProp::TsParamProp(param_prop) => {
                            let ident = match &param_prop.param {
                                ast::TsParamPropParam::Ident(ident) => ident,
                                _ => return None,
                            };
                            let ty = ident
                                .type_ann
                                .as_ref()
                                .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok())?;
                            Some(ParamDef {
                                name: ident.id.sym.to_string(),
                                ty,
                                optional: ident.id.optional,
                                has_default: false,
                            })
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
                    // I-383 T8': constructor は通常 generic を持たないが、TS の `class C<T> {
                    // constructor<U>(...) }` のような構文があれば ctor.function.type_params から
                    // 抽出する。現状の SWC AST の Constructor では type_params は直接持てないため
                    // 空 vec で OK。
                    type_params: vec![],
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
                let params: Vec<ParamDef<TsTypeInfo>> = method
                    .function
                    .params
                    .iter()
                    .filter_map(|param| super::functions::extract_pat_param(&param.pat))
                    .collect();
                let return_type = method
                    .function
                    .return_type
                    .as_ref()
                    .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok());
                let has_rest = method
                    .function
                    .params
                    .iter()
                    .any(|param| matches!(&param.pat, ast::Pat::Rest(_)));
                // I-383 T8': メソッド自身の generic 型パラメータを抽出する。
                // 例: `class C<S> { foo<M extends string>(x: M | M[]) }` の `<M>`。
                // 抽出した type_params は `resolve_method_sig` で scope に push され、
                // 戻り値型・パラメータ型解決中の anonymous union の generic 化に使われる。
                let method_type_params =
                    collect_type_params(method.function.type_params.as_deref());
                methods.entry(name).or_default().push(MethodSignature {
                    params,
                    return_type,
                    has_rest,
                    type_params: method_type_params,
                });
            }
            _ => {}
        }
    }

    let type_params = collect_type_params(class.class.type_params.as_deref());
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

/// TS の型パラメータ宣言から `TypeParam<TsTypeInfo>` を収集する。
///
/// 制約は `convert_to_ts_type_info` で TsTypeInfo に変換する（TypeRegistry 不要）。
pub(crate) fn collect_type_params(
    decl: Option<&ast::TsTypeParamDecl>,
) -> Vec<TypeParam<TsTypeInfo>> {
    decl.map(|d| {
        d.params
            .iter()
            .map(|p| TypeParam {
                name: p.name.sym.to_string(),
                constraint: p
                    .constraint
                    .as_ref()
                    .and_then(|c| convert_to_ts_type_info(c).ok()),
            })
            .collect()
    })
    .unwrap_or_default()
}

/// `TypeDef<TsTypeInfo>::Struct` を `TypeDef<RustType>::Struct` に解決する（registry 用）。
///
/// `resolve_struct_members` で全メンバーを解決し、monomorphization はスキップする。
/// registry は generic 定義を TypeVar のまま保持するため。
/// monomorphization は TypeConverter (IR 生成) 側の責務。
fn resolve_struct_for_registry(
    def: TypeDef<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<TypeDef<RustType>> {
    use crate::ts_type_info::resolve::typedef::resolve_struct_members;

    if let TypeDef::Struct {
        type_params,
        fields,
        methods,
        constructor,
        call_signatures,
        extends,
        is_interface,
    } = def
    {
        // 型パラメータスコープを push（TypeVar 認識用）
        let tp_names: Vec<String> = type_params.iter().map(|tp| tp.name.clone()).collect();
        let prev_scope = synthetic.push_type_param_scope(tp_names);

        let result = resolve_struct_members(
            type_params,
            fields,
            methods,
            constructor,
            call_signatures,
            reg,
            synthetic,
        )
        .map(|members| TypeDef::Struct {
            type_params: members.type_params,
            fields: members.fields,
            methods: members.methods,
            constructor: members.constructor,
            call_signatures: members.call_signatures,
            extends,
            is_interface,
        });

        synthetic.restore_type_param_scope(prev_scope);
        result
    } else {
        anyhow::bail!("expected TypeDef::Struct")
    }
}

/// TypeRef / その他の型を registry 用に解決する。
///
/// `resolve_ts_type` で RustType に変換し、Named 型の場合は registry または
/// SyntheticTypeRegistry からフィールドを取得して TypeDef::Struct を構築する。
/// utility type (Partial, Pick 等) も正しく解決される。
fn resolve_type_ref_for_registry(
    info: &TsTypeInfo,
    type_params: &[TypeParam<TsTypeInfo>],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<TypeDef<RustType>> {
    // 型パラメータ scope を push
    let tp_names: Vec<String> = type_params.iter().map(|tp| tp.name.clone()).collect();
    let prev_scope = synthetic.push_type_param_scope(tp_names);

    let result = (|| -> Option<TypeDef<RustType>> {
        let rt = resolve_ts_type(info, reg, synthetic).ok()?;

        let source_def = match &rt {
            RustType::Named { name, type_args } => {
                // registry から取得（generic type ref のインスタンス化含む）
                if let Some(def @ TypeDef::Struct { .. }) = reg.get(name) {
                    if !type_args.is_empty() {
                        reg.instantiate(name, type_args)
                            .or_else(|| Some(def.clone()))
                    } else {
                        Some(def.clone())
                    }
                }
                // SyntheticTypeRegistry から取得（Partial, Pick 等の utility type）
                else if let Some(syn_def) = synthetic.get(name) {
                    if let crate::ir::Item::Struct { fields, .. } = &syn_def.item {
                        Some(TypeDef::Struct {
                            type_params: vec![],
                            fields: fields
                                .iter()
                                .map(|sf| FieldDef {
                                    name: sf.name.clone(),
                                    ty: sf.ty.clone(),
                                    optional: matches!(sf.ty, RustType::Option(_)),
                                })
                                .collect(),
                            methods: HashMap::new(),
                            constructor: None,
                            call_signatures: vec![],
                            extends: vec![],
                            is_interface: false,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        let source = source_def?;
        if let TypeDef::Struct {
            fields,
            methods,
            constructor,
            call_signatures,
            extends,
            ..
        } = source
        {
            let resolved_params =
                resolve_type_params(type_params.to_vec(), reg, synthetic).unwrap_or_default();
            Some(TypeDef::Struct {
                type_params: resolved_params,
                fields,
                methods,
                constructor,
                call_signatures,
                extends,
                // type alias は TS でも interface ではない。`type X = SomeInterface` は
                // struct として扱い、trait 生成には interface 宣言自体が使われる。
                is_interface: false,
            })
        } else {
            None
        }
    })();

    synthetic.restore_type_param_scope(prev_scope);
    result
}

/// Intersection 型を registry 用に解決する。
///
/// 各 member を TsTypeInfo variant に応じて resolve 関数で処理し、
/// フィールドとメソッドをマージして TypeDef<RustType>::Struct を構築する。
/// registry 用のため monomorphization は行わない。
fn resolve_intersection_for_registry(
    members: &[TsTypeInfo],
    type_params: &[TypeParam<TsTypeInfo>],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<TypeDef<RustType>> {
    use crate::ir::sanitize_field_name;
    use crate::ts_type_info::resolve::intersection::resolve_type_literal_fields;

    // 型パラメータ scope を push
    let tp_names: Vec<String> = type_params.iter().map(|tp| tp.name.clone()).collect();
    let prev_scope = synthetic.push_type_param_scope(tp_names);

    let result = (|| -> Option<TypeDef<RustType>> {
        // Union member 検出 → resolve_intersection に委譲
        let has_union = members.iter().any(|m| matches!(m, TsTypeInfo::Union(_)));
        if has_union {
            if let Ok(rt) = crate::ts_type_info::resolve::intersection::resolve_intersection(
                members, reg, synthetic,
            ) {
                // synthetic enum/struct が作られる。TypeDef::Alias がないため
                // _0 field として埋め込む。
                let resolved_params =
                    resolve_type_params(type_params.to_vec(), reg, synthetic).unwrap_or_default();
                return Some(TypeDef::Struct {
                    type_params: resolved_params,
                    fields: vec![FieldDef {
                        name: "_0".to_string(),
                        ty: rt,
                        optional: false,
                    }],
                    methods: HashMap::new(),
                    constructor: None,
                    call_signatures: vec![],
                    extends: vec![],
                    is_interface: false,
                });
            }
            return None;
        }

        let mut merged_fields: Vec<FieldDef<RustType>> = Vec::new();
        let mut merged_methods: HashMap<String, Vec<MethodSignature<RustType>>> = HashMap::new();

        for member in members {
            match member {
                TsTypeInfo::TypeLiteral(lit) => {
                    // フィールドを resolve
                    if let Ok(fields) = resolve_type_literal_fields(lit, reg, synthetic) {
                        for sf in fields {
                            merged_fields.push(FieldDef {
                                name: sf.name,
                                optional: matches!(sf.ty, RustType::Option(_)),
                                ty: sf.ty,
                            });
                        }
                    }
                    // メソッドを resolve（TsMethodInfo → MethodSignature<TsTypeInfo> → resolve_method_sig）
                    // resolve_method_info → ir::Method の lossy 変換を回避し、
                    // optional/has_rest を保持する。
                    for method_info in &lit.methods {
                        let ts_sig = convert_method_info_to_sig(method_info);
                        if let Ok(resolved_sig) =
                            crate::ts_type_info::resolve::typedef::resolve_method_sig(
                                ts_sig, reg, synthetic,
                            )
                        {
                            merged_methods
                                .entry(method_info.name.clone())
                                .or_default()
                                .push(resolved_sig);
                        }
                    }
                }
                TsTypeInfo::TypeRef {
                    name, type_args, ..
                } => {
                    // Registry から解決済みフィールドを取得（型引数付きは instantiate）
                    if let Some(TypeDef::Struct { fields, .. }) = reg.get(name) {
                        let source_fields = if !type_args.is_empty() {
                            let resolved_args: Vec<RustType> = type_args
                                .iter()
                                .filter_map(|a| resolve_ts_type(a, reg, synthetic).ok())
                                .collect();
                            if let Some(TypeDef::Struct { fields, .. }) =
                                reg.instantiate(name, &resolved_args)
                            {
                                fields
                            } else {
                                fields.clone()
                            }
                        } else {
                            fields.clone()
                        };
                        for f in &source_fields {
                            merged_fields.push(FieldDef {
                                name: sanitize_field_name(&f.name),
                                ty: f.ty.clone(),
                                optional: f.optional,
                            });
                        }
                    } else {
                        // Struct 以外（Enum, Function 等）→ resolve_ts_type で _N field 埋め込み
                        // resolve_intersection (intersection.rs) と同じ振る舞い
                        if let Ok(rt) = resolve_ts_type(member, reg, synthetic) {
                            let field_name = format!("_{}", merged_fields.len());
                            merged_fields.push(FieldDef {
                                name: field_name,
                                ty: rt,
                                optional: false,
                            });
                        }
                    }
                }
                _ => {
                    // Mapped / その他 → resolve_ts_type で解決し _N field 埋め込み
                    if let Ok(rt) = resolve_ts_type(member, reg, synthetic) {
                        let field_name = format!("_{}", merged_fields.len());
                        merged_fields.push(FieldDef {
                            name: field_name,
                            ty: rt,
                            optional: false,
                        });
                    }
                }
            }
        }

        if merged_fields.is_empty() && merged_methods.is_empty() {
            return None;
        }

        let resolved_params =
            resolve_type_params(type_params.to_vec(), reg, synthetic).unwrap_or_default();
        Some(TypeDef::Struct {
            type_params: resolved_params,
            fields: merged_fields,
            methods: merged_methods,
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        })
    })();

    synthetic.restore_type_param_scope(prev_scope);
    result
}

/// `TsTypeLiteralInfo` から `TypeDef<TsTypeInfo>::Struct` を構築する。
///
/// TsTypeInfo の各メンバーを `FieldDef<TsTypeInfo>` / `MethodSignature<TsTypeInfo>` に変換し、
/// `resolve_typedef` に渡せる形式を返す。index signature は TypeDef では表現できないため、
/// 呼び出し元で別途処理する。
fn build_struct_from_type_literal(
    lit: &TsTypeLiteralInfo,
    type_params: Vec<TypeParam<TsTypeInfo>>,
) -> TypeDef<TsTypeInfo> {
    // TsFieldInfo → FieldDef<TsTypeInfo>
    let fields: Vec<FieldDef<TsTypeInfo>> = lit
        .fields
        .iter()
        .map(|f| FieldDef {
            name: f.name.clone(),
            ty: f.ty.clone(),
            optional: f.optional,
        })
        .collect();

    // TsMethodInfo → MethodSignature<TsTypeInfo> (grouped by name)
    let mut methods: HashMap<String, Vec<MethodSignature<TsTypeInfo>>> = HashMap::new();
    for m in &lit.methods {
        let sig = convert_method_info_to_sig(m);
        methods.entry(m.name.clone()).or_default().push(sig);
    }

    // TsFnSigInfo (call) → call_signatures
    let call_signatures: Vec<MethodSignature<TsTypeInfo>> = lit
        .call_signatures
        .iter()
        .map(convert_fn_sig_to_method_sig)
        .collect();

    // TsFnSigInfo (construct) → constructor
    let constructor = if lit.construct_signatures.is_empty() {
        None
    } else {
        Some(
            lit.construct_signatures
                .iter()
                .map(convert_fn_sig_to_method_sig)
                .collect(),
        )
    };

    TypeDef::Struct {
        type_params,
        fields,
        methods,
        constructor,
        call_signatures,
        extends: vec![],
        is_interface: false,
    }
}

/// `TsMethodInfo` → `MethodSignature<TsTypeInfo>` 変換。
fn convert_method_info_to_sig(m: &TsMethodInfo) -> MethodSignature<TsTypeInfo> {
    let params = m
        .params
        .iter()
        .map(|p| ParamDef {
            name: p.name.clone(),
            ty: p.ty.clone(),
            optional: p.optional,
            has_default: false,
        })
        .collect();
    let type_params = m
        .type_params
        .iter()
        .map(|name| TypeParam {
            name: name.clone(),
            constraint: None,
        })
        .collect();
    MethodSignature {
        params,
        return_type: m.return_type.clone(),
        has_rest: m.has_rest,
        type_params,
    }
}

/// `TsFnSigInfo` → `MethodSignature<TsTypeInfo>` 変換。
fn convert_fn_sig_to_method_sig(sig: &TsFnSigInfo) -> MethodSignature<TsTypeInfo> {
    let params = sig
        .params
        .iter()
        .map(|p| ParamDef {
            name: p.name.clone(),
            ty: p.ty.clone(),
            optional: p.optional,
            has_default: false,
        })
        .collect();
    MethodSignature {
        params,
        return_type: sig.return_type.clone(),
        has_rest: sig.has_rest,
        type_params: vec![],
    }
}

/// `as const` 宣言または型注釈付き const 宣言から `TypeDef::ConstValue<TsTypeInfo>` を構築する。
///
/// 対象パターン:
/// - `const X = ['a', 'b'] as const` → 文字列リテラル配列
/// - `const X = { key: 'value' } as const` → オブジェクトリテラル
/// - `const X: Type = expr` → 型注釈から構築
fn collect_const_value_def(d: &ast::VarDeclarator) -> Option<TypeDef<TsTypeInfo>> {
    // Case 1: Type annotation — resolve fields from the annotated type
    if let ast::Pat::Ident(ident) = &d.name {
        if let Some(type_ann) = &ident.type_ann {
            return collect_const_value_from_type_annotation(&type_ann.type_ann);
        }
    }

    // Case 2: `as const` assertion
    let init = d.init.as_ref()?;
    if let ast::Expr::TsConstAssertion(assertion) = init.as_ref() {
        return collect_const_value_from_as_const(&assertion.expr);
    }

    None
}

/// 型注釈から `ConstValue<TsTypeInfo>` を構築する。
///
/// 型注釈がオブジェクト型リテラルの場合、フィールドを直接変換する。
/// 型参照（`const x: MyType = ...`）の場合は、参照名を保持した ConstValue を生成し、
/// 後続の型解決（TsTypeQuery ハンドラ）で参照先から間接的にフィールドを取得する。
fn collect_const_value_from_type_annotation(type_ann: &ast::TsType) -> Option<TypeDef<TsTypeInfo>> {
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
                    .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok())
                    .unwrap_or(TsTypeInfo::Any);
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

/// `as const` アサーション内の式から `ConstValue<TsTypeInfo>` を構築する。
fn collect_const_value_from_as_const(expr: &ast::Expr) -> Option<TypeDef<TsTypeInfo>> {
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

/// 配列リテラルからリテラル要素を抽出する（TsTypeInfo 版）。
///
/// 各要素のリテラル型と、文字列リテラルの場合はその値を保持する。
/// すべての要素がリテラルの場合のみ返す。
fn extract_const_array_elements(array_lit: &ast::ArrayLit) -> Vec<ConstElement<TsTypeInfo>> {
    let mut elements = Vec::new();
    for elem in &array_lit.elems {
        match elem {
            Some(ast::ExprOrSpread { expr, .. }) => match expr.as_ref() {
                ast::Expr::Lit(ast::Lit::Str(s)) => {
                    elements.push(ConstElement {
                        ty: TsTypeInfo::String,
                        string_literal_value: Some(s.value.to_string_lossy().into_owned()),
                    });
                }
                ast::Expr::Lit(ast::Lit::Num(_)) => {
                    elements.push(ConstElement {
                        ty: TsTypeInfo::Number,
                        string_literal_value: None,
                    });
                }
                ast::Expr::Lit(ast::Lit::Bool(_)) => {
                    elements.push(ConstElement {
                        ty: TsTypeInfo::Boolean,
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

/// オブジェクトリテラルからフィールド情報を抽出する（TsTypeInfo 版）。
fn extract_const_object_fields(obj_lit: &ast::ObjectLit) -> Vec<ConstField<TsTypeInfo>> {
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
                        TsTypeInfo::String,
                        Some(s.value.to_string_lossy().into_owned()),
                    ),
                    ast::Expr::Lit(ast::Lit::Num(_)) => (TsTypeInfo::Number, None),
                    ast::Expr::Lit(ast::Lit::Bool(_)) => (TsTypeInfo::Boolean, None),
                    _ => (TsTypeInfo::Any, None),
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

#[cfg(test)]
mod build_struct_from_type_literal_tests {
    use super::*;
    use crate::ts_type_info::*;

    fn empty_lit() -> TsTypeLiteralInfo {
        TsTypeLiteralInfo {
            fields: vec![],
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![],
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_empty_returns_empty_struct() {
        let lit = empty_lit();
        let result = build_struct_from_type_literal(&lit, vec![]);
        match result {
            TypeDef::Struct {
                fields,
                methods,
                call_signatures,
                constructor,
                ..
            } => {
                assert!(fields.is_empty());
                assert!(methods.is_empty());
                assert!(call_signatures.is_empty());
                assert!(constructor.is_none());
            }
            other => panic!("expected Struct, got {other:?}"),
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_property_only() {
        let lit = TsTypeLiteralInfo {
            fields: vec![
                TsFieldInfo {
                    name: "x".to_string(),
                    ty: TsTypeInfo::Number,
                    optional: false,
                },
                TsFieldInfo {
                    name: "name".to_string(),
                    ty: TsTypeInfo::String,
                    optional: true,
                },
            ],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct { fields, .. } = result {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, TsTypeInfo::Number);
            assert!(!fields[0].optional);
            assert_eq!(fields[1].name, "name");
            assert_eq!(fields[1].ty, TsTypeInfo::String);
            assert!(fields[1].optional);
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_method_only() {
        let lit = TsTypeLiteralInfo {
            methods: vec![TsMethodInfo {
                name: "handle".to_string(),
                params: vec![TsParamInfo {
                    name: "x".to_string(),
                    ty: TsTypeInfo::String,
                    optional: false,
                }],
                return_type: Some(TsTypeInfo::Void),
                type_params: vec![],
                optional: false,
                has_rest: false,
            }],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct { methods, .. } = result {
            let sigs = methods.get("handle").expect("handle method");
            assert_eq!(sigs.len(), 1);
            assert_eq!(sigs[0].params.len(), 1);
            assert_eq!(sigs[0].params[0].name, "x");
            assert_eq!(sigs[0].params[0].ty, TsTypeInfo::String);
            assert_eq!(sigs[0].return_type, Some(TsTypeInfo::Void));
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_call_signature() {
        let lit = TsTypeLiteralInfo {
            call_signatures: vec![TsFnSigInfo {
                params: vec![TsParamInfo {
                    name: "input".to_string(),
                    ty: TsTypeInfo::String,
                    optional: false,
                }],
                return_type: Some(TsTypeInfo::Number),
                has_rest: false,
            }],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct {
            call_signatures, ..
        } = result
        {
            assert_eq!(call_signatures.len(), 1);
            assert_eq!(call_signatures[0].params.len(), 1);
            assert_eq!(call_signatures[0].params[0].name, "input");
            assert_eq!(call_signatures[0].return_type, Some(TsTypeInfo::Number));
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_construct_signature() {
        let lit = TsTypeLiteralInfo {
            construct_signatures: vec![TsFnSigInfo {
                params: vec![TsParamInfo {
                    name: "config".to_string(),
                    ty: TsTypeInfo::String,
                    optional: false,
                }],
                return_type: Some(TsTypeInfo::TypeRef {
                    name: "Foo".to_string(),
                    type_args: vec![],
                }),
                has_rest: false,
            }],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct { constructor, .. } = result {
            let ctors = constructor.expect("constructor should be Some");
            assert_eq!(ctors.len(), 1);
            assert_eq!(ctors[0].params[0].name, "config");
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_build_struct_from_type_literal_mixed_fields_and_methods() {
        let lit = TsTypeLiteralInfo {
            fields: vec![TsFieldInfo {
                name: "count".to_string(),
                ty: TsTypeInfo::Number,
                optional: false,
            }],
            methods: vec![TsMethodInfo {
                name: "increment".to_string(),
                params: vec![],
                return_type: Some(TsTypeInfo::Void),
                type_params: vec![],
                optional: false,
                has_rest: false,
            }],
            ..empty_lit()
        };
        let result = build_struct_from_type_literal(&lit, vec![]);
        if let TypeDef::Struct {
            fields, methods, ..
        } = result
        {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "count");
            assert!(methods.contains_key("increment"));
        } else {
            panic!("expected Struct");
        }
    }
}
