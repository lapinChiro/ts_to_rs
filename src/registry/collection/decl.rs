//! Pass 2 of the 2-pass registry collection: dispatch on the
//! declaration kind and build the final `TypeDef<RustType>` entry.
//!
//! For most branches (interface, DU, function, class) this delegates
//! to [`super::class::collect_class_info`] or the interface helpers in
//! [`crate::registry::interfaces`], then runs
//! [`crate::ts_type_info::resolve::resolve_typedef`] to go from
//! `TypeDef<TsTypeInfo>` to `TypeDef<RustType>`.
//!
//! The `type` alias branch is the most complex: it fans out into three
//! sub-paths depending on the RHS shape:
//!
//! - **TypeLiteral** (`{ ... }`): build via
//!   [`super::type_literals::build_struct_from_type_literal`] →
//!   [`super::resolvers::resolve_struct_for_registry`]
//! - **Intersection** (`A & B`):
//!   [`super::resolvers::resolve_intersection_for_registry`]
//! - **Everything else** (TypeRef, utility types, …):
//!   [`super::resolvers::resolve_type_ref_for_registry`]

use std::collections::HashMap;

use swc_ecma_ast as ast;

use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::ts_type_info::resolve::resolve_typedef;
use crate::ts_type_info::{convert_to_ts_type_info, TsTypeInfo};

use super::callable::{classify_callable_interface, CallableInterfaceKind};
use super::class::collect_class_info;
use super::collect_type_params;
use super::const_values::collect_const_value_def;
use super::resolvers::{
    resolve_intersection_for_registry, resolve_struct_for_registry, resolve_type_ref_for_registry,
};
use super::type_literals::build_struct_from_type_literal;

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
pub(in crate::registry) fn collect_decl(
    reg: &mut TypeRegistry,
    decl: &ast::Decl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) {
    match decl {
        ast::Decl::TsInterface(iface) => {
            if let Ok(fields) = crate::registry::interfaces::collect_interface_fields(iface) {
                let sigs = crate::registry::interfaces::collect_interface_signatures(iface);
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
            if let Some(enum_def) = crate::registry::unions::try_collect_string_literal_union(alias)
            {
                // TypeDef<TsTypeInfo>::Enum → resolve で PascalCase 適用
                if let Ok(resolved) = resolve_typedef(enum_def, lookup, synthetic) {
                    reg.register(alias.id.sym.to_string(), resolved);
                }
            } else if let Some(mut enum_def) =
                crate::registry::unions::try_collect_discriminated_union(alias)
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
            } else if let Some(func_def) =
                crate::registry::functions::try_collect_fn_type_alias(alias)
            {
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
            if let Ok(func_def) =
                crate::registry::functions::collect_fn_def_with_extras(&fn_decl.function)
            {
                if let Ok(resolved) = resolve_typedef(func_def, lookup, synthetic) {
                    let fn_name = fn_decl.ident.sym.to_string();
                    reg.register(fn_name, resolved);
                }
            }
        }
        ast::Decl::Var(var_decl) => {
            for d in &var_decl.decls {
                let ident = match &d.name {
                    ast::Pat::Ident(ident) => ident,
                    _ => continue,
                };
                let name = ident.id.sym.to_string();

                // Arrow function: check if type annotation references a callable interface.
                // If so, register as ConstValue with type_ref_name (not Function).
                if let Some(init) = &d.init {
                    if let ast::Expr::Arrow(arrow) = init.as_ref() {
                        // Extract type annotation name if it's a simple TsTypeRef
                        let type_ann_name =
                            ident
                                .type_ann
                                .as_ref()
                                .and_then(|ann| match &*ann.type_ann {
                                    ast::TsType::TsTypeRef(type_ref) => {
                                        if let ast::TsEntityName::Ident(id) = &type_ref.type_name {
                                            Some(id.sym.to_string())
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                });

                        // If the type annotation refers to a callable interface,
                        // register as ConstValue so the transformer can route through
                        // convert_callable_trait_const
                        if let Some(ref ann_name) = type_ann_name {
                            if let Some(def) = lookup.get(ann_name) {
                                if !matches!(
                                    classify_callable_interface(def),
                                    CallableInterfaceKind::NonCallable
                                ) {
                                    reg.register(
                                        name,
                                        TypeDef::ConstValue {
                                            fields: vec![],
                                            elements: vec![],
                                            type_ref_name: Some(ann_name.clone()),
                                        },
                                    );
                                    continue;
                                }
                            }
                        }

                        // Non-callable arrow: collect as Function (existing path)
                        if let Ok(func_def) =
                            crate::registry::functions::collect_arrow_def_with_extras(arrow)
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
            // I-205 Iteration v9: empty body の class でも `extends Parent` がある場合は
            // registry に Pass 2 結果を登録する必要がある (B7 inherited dispatch detection の
            // 前提)。旧 condition `!fields.is_empty() || !methods.is_empty() ||
            // constructor.is_some()` では `class Sub extends Base {}` が placeholder の
            // 空 TypeDef (= extends: []) のまま放置されていた。`extends.is_empty()` も
            // condition に追加し、extends を持つ class は body が空でも登録する。
            if let TypeDef::Struct {
                ref fields,
                ref methods,
                ref constructor,
                ref extends,
                ..
            } = ts_def
            {
                if !fields.is_empty()
                    || !methods.is_empty()
                    || constructor.is_some()
                    || !extends.is_empty()
                {
                    if let Ok(resolved) = resolve_typedef(ts_def, lookup, synthetic) {
                        reg.register(class.ident.sym.to_string(), resolved);
                    }
                }
            }
        }
        _ => {}
    }
}
