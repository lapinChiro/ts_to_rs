//! TypeDef<TsTypeInfo> → TypeDef<RustType> 変換。
//!
//! registry フェーズで構築された TS 型ベースの TypeDef を、
//! Rust 型ベースに変換する。Optional ラップ、PascalCase 命名もここで行う。

use std::collections::HashMap;

use crate::ir::{RustType, TypeParam};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{
    ConstElement, ConstField, FieldDef, MethodSignature, ParamDef, TypeDef, TypeRegistry,
};
use crate::ts_type_info::TsTypeInfo;

use super::resolve_ts_type;

/// Vec<TypeParam<TsTypeInfo>> → Vec<TypeParam<RustType>> 変換。
///
/// 制約の型解決に失敗した場合はエラーを伝播する。
pub(crate) fn resolve_type_params(
    params: Vec<crate::ir::TypeParam<TsTypeInfo>>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<Vec<crate::ir::TypeParam<RustType>>> {
    params
        .into_iter()
        .map(|tp| {
            let constraint = tp
                .constraint
                .map(|c| resolve_ts_type(&c, reg, synthetic))
                .transpose()?;
            let default = tp
                .default
                .map(|d| resolve_ts_type(&d, reg, synthetic))
                .transpose()?;
            Ok(crate::ir::TypeParam {
                name: tp.name,
                constraint,
                default,
            })
        })
        .collect()
}

/// TypeDef<TsTypeInfo> → TypeDef<RustType> 変換。
///
/// registry フェーズで構築された TS 型ベースの TypeDef を、
/// Rust 型ベースに変換する。Optional ラップ、PascalCase 命名もここで行う。
///
/// # エラーハンドリング
///
/// 各要素（フィールド、パラメータ、メソッドシグネチャ等）の型解決失敗は
/// TypeDef 全体のエラーとして伝播する。部分的に成功した TypeDef は生成しない。
/// 呼び出し元（`collection.rs`）は `if let Ok(resolved)` でハンドリングし、
/// TypeDef 全体の失敗は「未解決の型」として安全に処理される。
pub fn resolve_typedef(
    def: TypeDef<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<TypeDef<RustType>> {
    // 型パラメータスコープを設定（合成 union enum に型パラメータを伝播するため）。
    // push/restore で early return 時にもスコープが確実に復元される。
    let tp_names: Vec<String> = match &def {
        TypeDef::Struct { type_params, .. }
        | TypeDef::Enum { type_params, .. }
        | TypeDef::Function { type_params, .. } => {
            type_params.iter().map(|tp| tp.name.clone()).collect()
        }
        TypeDef::ConstValue { .. } => vec![],
    };
    let prev_scope = synthetic.push_type_param_scope(tp_names);

    let result = resolve_typedef_inner(def, reg, synthetic);

    // Apply monomorphization substitutions across all items currently in the
    // registry. NOTE: `apply_substitutions_to_items` iterates *all* entries in
    // `synthetic.types`, not only ones created during this call. This is safe
    // in production because `resolve_typedef` is invoked from `build_registry`
    // with a fresh `SyntheticTypeRegistry::new()` whose types are empty before
    // the call (see `registry/collection/decl.rs`). If a future code path were
    // to invoke `resolve_typedef` on a `fork_dedup_state` synthetic that
    // inherits parent types (post-I-177-E semantics), the substitutions could
    // mutate inherited entries and corrupt them on merge-back. Tracked as a
    // defense-in-depth concern in TODO `[I-177-G]`.
    if let Ok((_, ref mono_subs)) = result {
        synthetic.apply_substitutions_to_items(mono_subs);
    }

    synthetic.restore_type_param_scope(prev_scope);

    result.map(|(td, _)| td)
}

/// TypeDef::Struct の全メンバーを TsTypeInfo → RustType に解決する共有関数。
///
/// fields / methods / constructor / call_signatures / type_params を一括解決する。
/// monomorphization は行わない — 呼び出し元の責務。
///
/// registry 登録用 (`resolve_struct_for_registry`) と IR 生成用 (`resolve_typedef`) の
/// 両方がこの関数を共有することで DRY を維持する。
pub(crate) fn resolve_struct_members(
    type_params: Vec<TypeParam<TsTypeInfo>>,
    fields: Vec<FieldDef<TsTypeInfo>>,
    methods: HashMap<String, Vec<MethodSignature<TsTypeInfo>>>,
    constructor: Option<Vec<MethodSignature<TsTypeInfo>>>,
    call_signatures: Vec<MethodSignature<TsTypeInfo>>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<ResolvedStructMembers> {
    let resolved_fields = fields
        .into_iter()
        .map(|f| resolve_field_def(f, reg, synthetic))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let resolved_methods = methods
        .into_iter()
        .map(|(name, sigs)| {
            let resolved_sigs = sigs
                .into_iter()
                .map(|sig| resolve_method_sig(sig, reg, synthetic))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok((name, resolved_sigs))
        })
        .collect::<anyhow::Result<Vec<_>>>()?
        .into_iter()
        .collect();
    let resolved_ctor = constructor
        .map(|ctors| {
            ctors
                .into_iter()
                .map(|sig| resolve_method_sig(sig, reg, synthetic))
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .transpose()?;
    let resolved_call_sigs = call_signatures
        .into_iter()
        .map(|sig| resolve_method_sig(sig, reg, synthetic))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let resolved_params = resolve_type_params(type_params, reg, synthetic)?;
    Ok(ResolvedStructMembers {
        type_params: resolved_params,
        fields: resolved_fields,
        methods: resolved_methods,
        constructor: resolved_ctor,
        call_signatures: resolved_call_sigs,
    })
}

/// `resolve_struct_members` の結果。
pub(crate) struct ResolvedStructMembers {
    pub type_params: Vec<TypeParam<RustType>>,
    pub fields: Vec<FieldDef<RustType>>,
    pub methods: HashMap<String, Vec<MethodSignature<RustType>>>,
    pub constructor: Option<Vec<MethodSignature<RustType>>>,
    pub call_signatures: Vec<MethodSignature<RustType>>,
}

/// `resolve_typedef` の内部実装。型パラメータスコープの管理は呼び出し元が行う。
///
/// Returns `(resolved_typedef, mono_subs)` where `mono_subs` is the monomorphization
/// substitution map. The caller applies `mono_subs` to synthetic items.
fn resolve_typedef_inner(
    def: TypeDef<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<(TypeDef<RustType>, HashMap<String, RustType>)> {
    match def {
        TypeDef::Struct {
            type_params,
            fields,
            methods,
            constructor,
            call_signatures,
            extends,
            is_interface,
        } => {
            let members = resolve_struct_members(
                type_params,
                fields,
                methods,
                constructor,
                call_signatures,
                reg,
                synthetic,
            )?;
            let (remaining_params, mono_subs) =
                monomorphize_type_params(members.type_params, reg, synthetic);

            let result = TypeDef::Struct {
                type_params: remaining_params,
                fields: members.fields,
                methods: members.methods,
                constructor: members.constructor,
                call_signatures: members.call_signatures,
                extends,
                is_interface,
            };
            Ok((
                apply_substitutions_to_typedef(result, &mono_subs),
                mono_subs,
            ))
        }

        TypeDef::Enum {
            type_params,
            variants,
            string_values,
            tag_field,
            variant_fields,
        } => {
            // string literal union / DU: raw 文字列を PascalCase に変換
            let (pascal_variants, pascal_string_values, pascal_variant_fields) =
                if !string_values.is_empty() {
                    use crate::ir::string_to_pascal_case;
                    let pv: Vec<String> = variants
                        .iter()
                        .map(|v| {
                            if string_values.contains_key(v) {
                                string_to_pascal_case(v)
                            } else {
                                v.clone()
                            }
                        })
                        .collect();
                    let psv: std::collections::HashMap<String, String> = string_values
                        .into_keys()
                        .map(|raw| {
                            let pascal = string_to_pascal_case(&raw);
                            (raw, pascal)
                        })
                        .collect();
                    let pvf: std::collections::HashMap<String, Vec<FieldDef<TsTypeInfo>>> =
                        variant_fields
                            .into_iter()
                            .map(|(raw_key, fields)| {
                                let pascal_key = string_to_pascal_case(&raw_key);
                                (pascal_key, fields)
                            })
                            .collect();
                    (pv, psv, pvf)
                } else {
                    (variants, string_values, variant_fields)
                };

            let resolved_variant_fields = pascal_variant_fields
                .into_iter()
                .map(|(variant, fields)| {
                    let resolved = fields
                        .into_iter()
                        .map(|f| resolve_field_def(f, reg, synthetic))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    Ok((variant, resolved))
                })
                .collect::<anyhow::Result<Vec<_>>>()?
                .into_iter()
                .collect();

            let resolved_params = resolve_type_params(type_params, reg, synthetic)?;
            let (remaining_params, mono_subs) =
                monomorphize_type_params(resolved_params, reg, synthetic);

            let result = TypeDef::Enum {
                type_params: remaining_params,
                variants: pascal_variants,
                string_values: pascal_string_values,
                tag_field,
                variant_fields: resolved_variant_fields,
            };
            Ok((
                apply_substitutions_to_typedef(result, &mono_subs),
                mono_subs,
            ))
        }

        TypeDef::Function {
            type_params,
            params,
            return_type,
            has_rest,
        } => {
            let resolved_params = params
                .into_iter()
                .map(|p| resolve_param_def(p, reg, synthetic))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let resolved_return = return_type
                .map(|rt| resolve_ts_type(&rt, reg, synthetic))
                .transpose()?;

            let resolved_tp = resolve_type_params(type_params, reg, synthetic)?;
            let (remaining_tp, mono_subs) = monomorphize_type_params(resolved_tp, reg, synthetic);

            let result = TypeDef::Function {
                type_params: remaining_tp,
                params: resolved_params,
                return_type: resolved_return,
                has_rest,
            };
            Ok((
                apply_substitutions_to_typedef(result, &mono_subs),
                mono_subs,
            ))
        }

        TypeDef::ConstValue {
            fields,
            elements,
            type_ref_name,
        } => {
            let resolved_fields = fields
                .into_iter()
                .map(|f| {
                    let ty = resolve_ts_type(&f.ty, reg, synthetic)?;
                    Ok(ConstField {
                        name: f.name,
                        ty,
                        string_literal_value: f.string_literal_value,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let resolved_elements = elements
                .into_iter()
                .map(|e| {
                    let ty = resolve_ts_type(&e.ty, reg, synthetic)?;
                    Ok(ConstElement {
                        ty,
                        string_literal_value: e.string_literal_value,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

            Ok((
                TypeDef::ConstValue {
                    fields: resolved_fields,
                    elements: resolved_elements,
                    type_ref_name,
                },
                HashMap::new(),
            ))
        }
    }
}

/// RustType が Rust の trait bound として有効かを判定する。
///
/// Rust では trait のみが `T: Bound` 構文で使用できる。
/// プリミティブ型、struct、enum は trait bound として無効。
///
/// TypeRegistry と SyntheticTypeRegistry の両方をチェックし、
/// どちらにも登録されていない Named 型は外部 trait の可能性があるため
/// 保守的に `true` を返す。
pub(crate) fn is_valid_trait_bound(
    ty: &RustType,
    reg: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) -> bool {
    match ty {
        // プリミティブ型は trait ではない
        RustType::F64
        | RustType::String
        | RustType::Bool
        | RustType::Unit
        | RustType::Any
        | RustType::Never => false,
        // 複合型は trait ではない
        RustType::Vec(_)
        | RustType::Option(_)
        | RustType::Tuple(_)
        | RustType::Fn { .. }
        | RustType::Result { .. }
        | RustType::Ref(_) => false,
        // I-387: 型変数 / 整数 primitive / std コレクションは trait ではない
        RustType::TypeVar { .. } | RustType::Primitive(_) | RustType::StdCollection { .. } => false,
        // Named 型: TypeRegistry + SyntheticTypeRegistry で判定
        RustType::Named { name, .. } => {
            if let Some(td) = reg.get(name) {
                return matches!(
                    td,
                    TypeDef::Struct {
                        is_interface: true,
                        ..
                    }
                );
            }
            // 合成型（union enum, inline struct 等）は trait ではない
            if synthetic.get(name).is_some() {
                return false;
            }
            // どちらにも未登録 → 外部 trait の可能性
            true
        }
        // DynTrait は常に有効
        RustType::DynTrait(_) => true,
        // QSelf (`<T as Trait>::Item`) 自体は associated type の参照であり trait bound
        // としては valid ではない（trait bound の対象は trait 名）
        RustType::QSelf { .. } => false,
    }
}

/// 非 trait 制約を持つ型パラメータをモノモーフィゼーションする。
///
/// 制約が valid trait bound でない型パラメータを substitution map に移し、
/// 型パラメータリストから除去する。残りのパラメータの制約内の参照も置換する。
///
/// # Returns
///
/// `(remaining_params, substitution_map)` — 残りの型パラメータと、モノモーフィゼーションの置換マップ。
pub(crate) fn monomorphize_type_params(
    type_params: Vec<TypeParam>,
    reg: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) -> (Vec<TypeParam>, HashMap<String, RustType>) {
    let mut remaining = type_params;
    let mut all_substitutions: HashMap<String, RustType> = HashMap::new();

    // イテレーティブに処理: チェーン制約（U extends T where T extends number）に対応。
    // T がモノモーフィゼーションされると U の制約が f64 に置換され、
    // 次のイテレーションで U もモノモーフィゼーション対象になる。
    loop {
        let mut new_remaining = Vec::new();
        let mut new_subs: HashMap<String, RustType> = HashMap::new();

        for tp in remaining {
            match &tp.constraint {
                // 制約が他の型パラメータ参照 (TypeVar) → 先行 param の monomorphization
                // 結果を待つ。次パスで substitute 経由で concrete 型に解決される。
                Some(RustType::TypeVar { .. }) => {
                    new_remaining.push(tp);
                }
                Some(ty) if !is_valid_trait_bound(ty, reg, synthetic) => {
                    new_subs.insert(tp.name.clone(), ty.clone());
                }
                _ => {
                    new_remaining.push(tp);
                }
            }
        }

        if new_subs.is_empty() {
            remaining = new_remaining;
            break;
        }

        // 新しい置換を残りのパラメータの制約に適用
        remaining = new_remaining
            .into_iter()
            .map(|tp| tp.substitute(&new_subs))
            .collect();

        all_substitutions.extend(new_subs);
    }

    (remaining, all_substitutions)
}

/// TypeDef 内の全型にモノモーフィゼーション置換を適用する。
fn apply_substitutions_to_typedef(def: TypeDef, subs: &HashMap<String, RustType>) -> TypeDef {
    if subs.is_empty() {
        return def;
    }
    match def {
        TypeDef::Struct {
            type_params,
            fields,
            methods,
            constructor,
            call_signatures,
            extends,
            is_interface,
        } => TypeDef::Struct {
            type_params,
            fields: fields.into_iter().map(|f| f.substitute(subs)).collect(),
            methods: methods
                .into_iter()
                .map(|(name, sigs)| (name, sigs.into_iter().map(|s| s.substitute(subs)).collect()))
                .collect(),
            constructor: constructor
                .map(|ctors| ctors.into_iter().map(|s| s.substitute(subs)).collect()),
            call_signatures: call_signatures
                .into_iter()
                .map(|s| s.substitute(subs))
                .collect(),
            extends,
            is_interface,
        },
        TypeDef::Enum {
            type_params,
            variants,
            string_values,
            tag_field,
            variant_fields,
        } => TypeDef::Enum {
            type_params,
            variants,
            string_values,
            tag_field,
            variant_fields: variant_fields
                .into_iter()
                .map(|(v, fields)| (v, fields.into_iter().map(|f| f.substitute(subs)).collect()))
                .collect(),
        },
        TypeDef::Function {
            type_params,
            params,
            return_type,
            has_rest,
        } => TypeDef::Function {
            type_params,
            params: params.into_iter().map(|p| p.substitute(subs)).collect(),
            return_type: return_type.map(|ty| ty.substitute(subs)),
            has_rest,
        },
        TypeDef::ConstValue { .. } => def,
    }
}

/// FieldDef<TsTypeInfo> → FieldDef<RustType> 変換。optional フラグに基づき Option ラップ。
///
/// **注意**: この関数は `FieldDef<TsTypeInfo>` 専用。`FieldDef<RustType>` に対して
/// 呼ぶと Option が二重にラップされる。型パラメータで防止されている。
pub(crate) fn resolve_field_def(
    field: FieldDef<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<FieldDef<RustType>> {
    let ty = resolve_ts_type(&field.ty, reg, synthetic)?.wrap_if_optional(field.optional);
    Ok(FieldDef {
        name: field.name,
        ty,
        optional: field.optional,
    })
}

/// ParamDef<TsTypeInfo> → ParamDef<RustType> 変換。
///
/// TS の `?:` optional フラグとデフォルト値付き (`has_default`) の両方を、
/// 単一の `Option<T>` エンコーディングに収束させる (I-040)。`wrap_if_optional`
/// を介するため二重ラップは自動抑止される。
///
/// **注意**: この関数は `ParamDef<TsTypeInfo>` 専用。型パラメータで防止されている。
pub(crate) fn resolve_param_def(
    param: ParamDef<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<ParamDef<RustType>> {
    let ty = resolve_ts_type(&param.ty, reg, synthetic)?
        .wrap_if_optional(param.optional || param.has_default);
    Ok(ParamDef {
        name: param.name,
        ty,
        optional: param.optional,
        has_default: param.has_default,
    })
}

/// MethodSignature<TsTypeInfo> → MethodSignature<RustType> 変換。
///
/// I-383 T8': メソッド自身の generic 型パラメータ (`sig.type_params`) を
/// `SyntheticTypeRegistry` の scope に append-merge で push する。これにより
/// `resolve_param_def` / `resolve_ts_type` が `register_union` 等を呼んだ際、
/// メソッド固有の generic を保持した anonymous union/struct が生成される。
/// 例: `class C<S> { foo<M>(x: M | M[]) }` → `enum MOrVecM<M>`。
///
/// scope は外部 (class type_params) に append される (`push_type_param_scope`
/// の append-merge 意味論)。restore は本関数終端で必ず呼ばれる (inner closure で
/// `?` 早期 return を吸収)。
pub(crate) fn resolve_method_sig(
    sig: MethodSignature<TsTypeInfo>,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<MethodSignature<RustType>> {
    let method_tp_names: Vec<String> = sig.type_params.iter().map(|tp| tp.name.clone()).collect();
    let prev_scope = synthetic.push_type_param_scope(method_tp_names);

    let result = (|| -> anyhow::Result<MethodSignature<RustType>> {
        let params = sig
            .params
            .into_iter()
            .map(|p| resolve_param_def(p, reg, synthetic))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let return_type = sig
            .return_type
            .map(|rt| resolve_ts_type(&rt, reg, synthetic))
            .transpose()?;
        // メソッド自身の type_params も RustType 制約に解決する
        let resolved_type_params = sig
            .type_params
            .into_iter()
            .map(|tp| {
                let constraint = tp
                    .constraint
                    .map(|c| resolve_ts_type(&c, reg, synthetic))
                    .transpose()?;
                let default = tp
                    .default
                    .map(|d| resolve_ts_type(&d, reg, synthetic))
                    .transpose()?;
                Ok(crate::ir::TypeParam {
                    name: tp.name,
                    constraint,
                    default,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(MethodSignature {
            params,
            return_type,
            has_rest: sig.has_rest,
            type_params: resolved_type_params,
            // I-205: input `MethodSignature<TsTypeInfo>` の kind を `MethodSignature<RustType>`
            // に lossless propagate (Method/Getter/Setter 区別を Pass 2 で失わせない)。
            kind: sig.kind,
        })
    })();

    synthetic.restore_type_param_scope(prev_scope);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap as StdHashMap;

    fn empty_reg() -> TypeRegistry {
        TypeRegistry::new()
    }

    fn empty_syn() -> SyntheticTypeRegistry {
        SyntheticTypeRegistry::new()
    }

    fn reg_with_interface(name: &str) -> TypeRegistry {
        let mut reg = TypeRegistry::new();
        reg.register(
            name.to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![],
                methods: StdHashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: true,
            },
        );
        reg
    }

    fn reg_with_struct(name: &str) -> TypeRegistry {
        let mut reg = TypeRegistry::new();
        reg.register(
            name.to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![],
                methods: StdHashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        reg
    }

    // ── is_valid_trait_bound ──

    #[test]
    fn primitive_types_are_not_valid_trait_bounds() {
        let reg = empty_reg();
        assert!(!is_valid_trait_bound(&RustType::F64, &reg, &empty_syn()));
        assert!(!is_valid_trait_bound(&RustType::String, &reg, &empty_syn()));
        assert!(!is_valid_trait_bound(&RustType::Bool, &reg, &empty_syn()));
        assert!(!is_valid_trait_bound(&RustType::Unit, &reg, &empty_syn()));
        assert!(!is_valid_trait_bound(&RustType::Any, &reg, &empty_syn()));
        assert!(!is_valid_trait_bound(&RustType::Never, &reg, &empty_syn()));
    }

    #[test]
    fn compound_types_are_not_valid_trait_bounds() {
        let reg = empty_reg();
        let syn = empty_syn();
        assert!(!is_valid_trait_bound(
            &RustType::Vec(Box::new(RustType::F64)),
            &reg,
            &syn,
        ));
        assert!(!is_valid_trait_bound(
            &RustType::Option(Box::new(RustType::String)),
            &reg,
            &syn,
        ));
        assert!(!is_valid_trait_bound(
            &RustType::Tuple(vec![RustType::F64]),
            &reg,
            &syn,
        ));
        assert!(!is_valid_trait_bound(
            &RustType::Fn {
                params: vec![],
                return_type: Box::new(RustType::Unit)
            },
            &reg,
            &syn,
        ));
    }

    #[test]
    fn interface_named_type_is_valid_trait_bound() {
        let reg = reg_with_interface("Serializable");
        let ty = RustType::Named {
            name: "Serializable".to_string(),
            type_args: vec![],
        };
        assert!(is_valid_trait_bound(&ty, &reg, &empty_syn()));
    }

    #[test]
    fn struct_named_type_is_not_valid_trait_bound() {
        let reg = reg_with_struct("MyClass");
        let ty = RustType::Named {
            name: "MyClass".to_string(),
            type_args: vec![],
        };
        assert!(!is_valid_trait_bound(&ty, &reg, &empty_syn()));
    }

    #[test]
    fn unregistered_named_type_assumed_trait() {
        let reg = empty_reg();
        let ty = RustType::Named {
            name: "ExternalTrait".to_string(),
            type_args: vec![],
        };
        assert!(is_valid_trait_bound(&ty, &reg, &empty_syn()));
    }

    #[test]
    fn dyn_trait_is_valid_trait_bound() {
        let reg = empty_reg();
        assert!(is_valid_trait_bound(
            &RustType::DynTrait("Foo".to_string()),
            &reg,
            &empty_syn(),
        ));
    }

    // ── monomorphize_type_params ──

    #[test]
    fn primitive_constraint_is_monomorphized() {
        let reg = empty_reg();
        let params = vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(RustType::F64),
            default: None,
        }];
        let (remaining, subs) = monomorphize_type_params(params, &reg, &empty_syn());
        assert!(remaining.is_empty());
        assert_eq!(subs.get("T"), Some(&RustType::F64));
    }

    #[test]
    fn interface_constraint_is_kept() {
        let reg = reg_with_interface("Serializable");
        let constraint = RustType::Named {
            name: "Serializable".to_string(),
            type_args: vec![],
        };
        let params = vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(constraint.clone()),
            default: None,
        }];
        let (remaining, subs) = monomorphize_type_params(params, &reg, &empty_syn());
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "T");
        assert_eq!(remaining[0].constraint, Some(constraint));
        assert!(subs.is_empty());
    }

    #[test]
    fn struct_constraint_is_monomorphized() {
        let reg = reg_with_struct("MyClass");
        let constraint = RustType::Named {
            name: "MyClass".to_string(),
            type_args: vec![],
        };
        let params = vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(constraint.clone()),
            default: None,
        }];
        let (remaining, subs) = monomorphize_type_params(params, &reg, &empty_syn());
        assert!(remaining.is_empty());
        assert_eq!(subs.get("T"), Some(&constraint));
    }

    #[test]
    fn unconstrained_param_is_kept() {
        let reg = empty_reg();
        let params = vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }];
        let (remaining, subs) = monomorphize_type_params(params, &reg, &empty_syn());
        assert_eq!(remaining.len(), 1);
        assert!(subs.is_empty());
    }

    #[test]
    fn mixed_params_partial_monomorphization() {
        let reg = reg_with_interface("Trait1");
        let params = vec![
            TypeParam {
                name: "T".to_string(),
                constraint: Some(RustType::F64),
                default: None,
            },
            TypeParam {
                name: "U".to_string(),
                constraint: Some(RustType::Named {
                    name: "Trait1".to_string(),
                    type_args: vec![],
                }),
                default: None,
            },
            TypeParam {
                name: "V".to_string(),
                constraint: None,
                default: None,
            },
        ];
        let (remaining, subs) = monomorphize_type_params(params, &reg, &empty_syn());
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].name, "U");
        assert_eq!(remaining[1].name, "V");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs.get("T"), Some(&RustType::F64));
    }

    // ── apply_substitutions_to_typedef ──

    #[test]
    fn apply_substitutions_struct_fields_replaced() {
        let def: TypeDef = TypeDef::Struct {
            type_params: vec![],
            fields: vec![FieldDef {
                name: "x".to_string(),
                ty: RustType::TypeVar {
                    name: "T".to_string(),
                },
                optional: false,
            }],
            methods: StdHashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        };
        let subs = StdHashMap::from([("T".to_string(), RustType::F64)]);
        let result = apply_substitutions_to_typedef(def, &subs);
        if let TypeDef::Struct { fields, .. } = result {
            assert_eq!(fields[0].ty, RustType::F64);
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn apply_substitutions_enum_variant_fields_replaced() {
        let def: TypeDef = TypeDef::Enum {
            type_params: vec![],
            variants: vec!["A".to_string()],
            string_values: StdHashMap::new(),
            tag_field: None,
            variant_fields: [(
                "A".to_string(),
                vec![FieldDef {
                    name: "val".to_string(),
                    ty: RustType::TypeVar {
                        name: "T".to_string(),
                    },
                    optional: false,
                }],
            )]
            .into_iter()
            .collect(),
        };
        let subs = StdHashMap::from([("T".to_string(), RustType::F64)]);
        let result = apply_substitutions_to_typedef(def, &subs);
        if let TypeDef::Enum { variant_fields, .. } = result {
            assert_eq!(variant_fields["A"][0].ty, RustType::F64);
        } else {
            panic!("expected Enum");
        }
    }

    #[test]
    fn apply_substitutions_function_params_and_return_replaced() {
        let def: TypeDef = TypeDef::Function {
            type_params: vec![],
            params: vec![ParamDef {
                name: "x".to_string(),
                ty: RustType::TypeVar {
                    name: "T".to_string(),
                },
                optional: false,
                has_default: false,
            }],
            return_type: Some(RustType::TypeVar {
                name: "T".to_string(),
            }),
            has_rest: false,
        };
        let subs = StdHashMap::from([("T".to_string(), RustType::F64)]);
        let result = apply_substitutions_to_typedef(def, &subs);
        if let TypeDef::Function {
            params,
            return_type,
            ..
        } = result
        {
            assert_eq!(params[0].ty, RustType::F64);
            assert_eq!(return_type, Some(RustType::F64));
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn apply_substitutions_empty_subs_returns_unchanged() {
        let def: TypeDef = TypeDef::Struct {
            type_params: vec![],
            fields: vec![FieldDef {
                name: "x".to_string(),
                ty: RustType::String,
                optional: false,
            }],
            methods: StdHashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        };
        let subs = StdHashMap::new();
        let result = apply_substitutions_to_typedef(def, &subs);
        if let TypeDef::Struct { fields, .. } = result {
            assert_eq!(fields[0].ty, RustType::String);
        } else {
            panic!("expected Struct");
        }
    }

    // ── is_valid_trait_bound: synthetic union enum ──

    #[test]
    fn synthetic_union_enum_is_not_valid_trait_bound() {
        let reg = empty_reg();
        let mut syn = empty_syn();
        let name = syn.register_union(&[RustType::String, RustType::F64]);
        let ty = RustType::Named {
            name,
            type_args: vec![],
        };
        assert!(
            !is_valid_trait_bound(&ty, &reg, &syn),
            "synthetic union enum should not be a valid trait bound"
        );
    }

    #[test]
    fn chained_constraint_substitution() {
        // T extends number, U extends T → both monomorphized
        let reg = empty_reg();
        let params = vec![
            TypeParam {
                name: "T".to_string(),
                constraint: Some(RustType::F64),
                default: None,
            },
            TypeParam {
                name: "U".to_string(),
                constraint: Some(RustType::TypeVar {
                    name: "T".to_string(),
                }),
                default: None,
            },
        ];
        let (remaining, subs) = monomorphize_type_params(params, &reg, &empty_syn());
        // イテレーティブ処理:
        // Pass 1: T → F64 (monomorphized)、U の制約は TypeVar("T") → 型変数参照のため
        //   deferred、remaining に残される。その後 substitute で制約が F64 に解決される。
        // Pass 2: U の制約 F64 は not valid trait bound → U も monomorphized
        assert!(remaining.is_empty());
        assert_eq!(subs.get("T"), Some(&RustType::F64));
        assert_eq!(subs.get("U"), Some(&RustType::F64));
    }
}
