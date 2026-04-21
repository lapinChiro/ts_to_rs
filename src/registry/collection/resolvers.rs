//! `TypeDef<TsTypeInfo>` → `TypeDef<RustType>` resolvers used by Pass
//! 2 ([`super::decl::collect_decl`]) when the TS-level `type` alias
//! cannot be stored as-is and requires TsTypeInfo-level shape analysis:
//!
//! - [`resolve_struct_for_registry`] — object/type-literal → `Struct`
//! - [`resolve_type_ref_for_registry`] — `T`, `Partial<T>`, utility
//!   types, etc.
//! - [`resolve_intersection_for_registry`] — `T & U & ...`
//!
//! These three resolvers are kept in one file because they recurse
//! into each other (resolve_struct → resolve_type_ref →
//! resolve_intersection → …). Splitting by file would force mutual
//! `pub(super)` visibility without structural benefit.

use std::collections::HashMap;

use crate::ir::{RustType, TypeParam};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{FieldDef, MethodSignature, TypeDef, TypeRegistry};
use crate::ts_type_info::resolve::{resolve_ts_type, resolve_type_params};
use crate::ts_type_info::TsTypeInfo;

use super::type_literals::convert_method_info_to_sig;

/// `TypeDef<TsTypeInfo>::Struct` を `TypeDef<RustType>::Struct` に解決する（registry 用）。
///
/// `resolve_struct_members` で全メンバーを解決し、monomorphization はスキップする。
/// registry は generic 定義を TypeVar のまま保持するため。
/// monomorphization は TypeConverter (IR 生成) 側の責務。
pub(super) fn resolve_struct_for_registry(
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
pub(super) fn resolve_type_ref_for_registry(
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
pub(super) fn resolve_intersection_for_registry(
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
