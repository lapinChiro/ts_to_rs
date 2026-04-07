//! Central registry for synthetic (compiler-generated) types.
//!
//! Manages union enums, any-type enums, and inline structs with
//! deduplication based on semantic signatures.

use std::collections::{BTreeMap, HashMap};

use crate::ir::{
    sanitize_field_name, EnumValue, EnumVariant, Item, RustType, StructField, TypeParam, Visibility,
};

/// A registry of synthetic types with automatic deduplication.
///
/// When the same union type (e.g., `string | number`) appears in multiple
/// locations, only one enum is generated. The registry tracks types by their
/// semantic signature and returns the same name for identical types.
#[derive(Debug)]
pub struct SyntheticTypeRegistry {
    /// Registered types by name (BTreeMap for deterministic iteration order).
    types: BTreeMap<String, SyntheticTypeDef>,
    /// Union deduplication: sorted member signature → registered name.
    union_dedup: HashMap<String, String>,
    /// Inline struct deduplication: field signature → registered name.
    struct_dedup: HashMap<String, String>,
    /// Intersection enum deduplication: variant signature → registered name.
    intersection_enum_dedup: HashMap<String, String>,
    /// Counter for generating unique inline struct names.
    struct_counter: u32,
    /// Counter for generating unique synthetic type names (e.g., _TypeLit0, _Intersection1).
    /// Replaces the global `SYNTHETIC_COUNTER` in transformer/types/mod.rs.
    synthetic_counter: u32,
    /// 現在のスコープで有効な型パラメータ名。
    ///
    /// TypeDef 解決中に設定し、解決完了後にクリアする。
    /// `register_union` が合成 enum に型パラメータを伝播するために使用。
    type_param_scope: Vec<String>,
}

/// A synthetic type definition.
#[derive(Debug)]
pub struct SyntheticTypeDef {
    /// The generated Rust type name.
    pub name: String,
    /// What kind of synthetic type this is.
    pub kind: SyntheticTypeKind,
    /// The IR item (enum or struct) for code generation.
    pub item: Item,
}

/// Classification of synthetic types.
#[derive(Debug)]
pub enum SyntheticTypeKind {
    /// A union type enum (e.g., `string | number` → `F64OrString`).
    UnionEnum,
    /// An any-type materialization enum (e.g., `ProcessDataInputType`).
    AnyEnum,
    /// An inline type literal struct (e.g., `{ x: number }` → `_TypeLit0`).
    InlineStruct,
    /// An impl block for a synthetic or named struct.
    ImplBlock,
    /// A stub trait (e.g., conditional type infer pattern → `Promise` trait).
    Trait,
    /// An auto-generated struct for an external builtin type
    /// (e.g., `ArrayBuffer`, `Date`, `Error`), materialized from `TypeRegistry`
    /// metadata by `external_struct_generator::generate_external_struct`.
    ///
    /// I-376: External types are conceptually synthetic — they are auto-generated,
    /// globally unique, and subject to the same canonical placement as other
    /// synthetic types. Previously they lived in per-file `Vec<Item>` which caused
    /// structural duplication between `file.items` and `synthetic_items`.
    External,
}

impl SyntheticTypeRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            types: BTreeMap::new(),
            union_dedup: HashMap::new(),
            struct_dedup: HashMap::new(),
            intersection_enum_dedup: HashMap::new(),
            struct_counter: 0,
            synthetic_counter: 0,
            type_param_scope: Vec::new(),
        }
    }

    /// 型パラメータスコープを設定し、以前のスコープを返す。
    ///
    /// 呼び出し元は処理完了後（正常・エラー問わず）に `restore_type_param_scope` で復元する。
    /// これにより `?` による early return でもスコープが正しく復元される。
    pub fn push_type_param_scope(&mut self, names: Vec<String>) -> Vec<String> {
        std::mem::replace(&mut self.type_param_scope, names)
    }

    /// 型パラメータスコープを復元する。
    pub fn restore_type_param_scope(&mut self, prev: Vec<String>) {
        self.type_param_scope = prev;
    }

    /// Registers a union type enum and returns its name.
    ///
    /// If the same combination of member types has been registered before,
    /// returns the existing name (idempotent deduplication).
    ///
    /// Automatically deduplicates member types that produce the same variant name
    /// (e.g., `Named("Foo", [String])` and `Named("ns::Foo", [])` both produce `"Foo"`
    /// after path extraction). The first occurrence wins.
    pub fn register_union(&mut self, member_types: &[RustType]) -> String {
        let signature = union_signature(member_types);

        if let Some(existing_name) = self.union_dedup.get(&signature) {
            return existing_name.clone();
        }

        // Deduplicate by variant name to prevent invalid Rust enums with
        // duplicate variant identifiers. First occurrence wins.
        let mut deduped: Vec<RustType> = Vec::new();
        let mut seen_names: Vec<String> = Vec::new();
        for ty in member_types {
            let vname = variant_name_for_type(ty);
            if !seen_names.contains(&vname) {
                seen_names.push(vname);
                deduped.push(ty.clone());
            }
        }

        let name = generate_union_name(&deduped);
        let variants = deduped
            .iter()
            .map(|ty| EnumVariant {
                name: variant_name_for_type(ty),
                value: None,
                data: Some(ty.clone()),
                fields: vec![],
            })
            .collect();

        // 型パラメータスコープから、メンバー型で使用されている型パラメータを検出
        let type_params: Vec<TypeParam> = self
            .type_param_scope
            .iter()
            .filter(|tp_name| member_types.iter().any(|ty| ty.uses_param(tp_name)))
            .map(|tp_name| TypeParam {
                name: tp_name.clone(),
                constraint: None,
            })
            .collect();

        let item = Item::Enum {
            vis: Visibility::Public,
            name: name.clone(),
            type_params,
            serde_tag: None,
            variants,
        };

        self.types.insert(
            name.clone(),
            SyntheticTypeDef {
                name: name.clone(),
                kind: SyntheticTypeKind::UnionEnum,
                item,
            },
        );
        self.union_dedup.insert(signature, name.clone());
        name
    }

    /// Registers an any-type materialization enum and returns its name.
    pub fn register_any_enum(
        &mut self,
        function_name: &str,
        param_name: &str,
        variants: Vec<EnumVariant>,
    ) -> String {
        let name = format!(
            "{}{}Type",
            to_pascal_case(function_name),
            to_pascal_case(param_name)
        );

        if let Some(existing) = self.types.get(&name) {
            return existing.name.clone();
        }

        let item = Item::Enum {
            vis: Visibility::Public,
            name: name.clone(),
            type_params: vec![],
            serde_tag: None,
            variants,
        };

        self.types.insert(
            name.clone(),
            SyntheticTypeDef {
                name: name.clone(),
                kind: SyntheticTypeKind::AnyEnum,
                item,
            },
        );
        name
    }

    /// Registers an inline struct and returns its name.
    ///
    /// If the same field structure has been registered before,
    /// returns the existing name (idempotent deduplication).
    /// Converts raw `(name, type)` pairs to `StructField` with sanitized names
    /// and `Visibility::Public`, then delegates to the shared dedup logic.
    pub fn register_inline_struct(&mut self, fields: &[(String, RustType)]) -> String {
        let struct_fields: Vec<StructField> = fields
            .iter()
            .map(|(name, ty)| StructField {
                vis: Some(Visibility::Public),
                name: sanitize_field_name(name),
                ty: ty.clone(),
            })
            .collect();
        let (name, _is_new) = self.register_struct_dedup(&struct_fields);
        name
    }

    /// Registers an intersection struct and returns its name with dedup.
    ///
    /// Uses the same `struct_dedup` cache as `register_inline_struct`, enabling
    /// cross-origin structural equivalence: an intersection struct `{ a: T } & { b: U }`
    /// and a type literal `{ a: T, b: U }` with the same fields produce the same type.
    ///
    /// Returns `(name, is_new)` where `is_new` is `false` on a dedup hit.
    pub fn register_intersection_struct(&mut self, fields: &[StructField]) -> (String, bool) {
        self.register_struct_dedup(fields)
    }

    /// Shared dedup logic for struct registration.
    ///
    /// Both `register_inline_struct` and `register_intersection_struct` delegate here.
    /// The dedup signature is computed from sanitized field names + types (order-independent).
    fn register_struct_dedup(&mut self, fields: &[StructField]) -> (String, bool) {
        let pairs: Vec<(String, RustType)> = fields
            .iter()
            .map(|f| (f.name.clone(), f.ty.clone()))
            .collect();
        let signature = struct_signature(&pairs);

        if let Some(existing_name) = self.struct_dedup.get(&signature) {
            return (existing_name.clone(), false);
        }

        let name = format!("_TypeLit{}", self.struct_counter);
        self.struct_counter += 1;

        let item = Item::Struct {
            vis: Visibility::Public,
            name: name.clone(),
            type_params: vec![],
            fields: fields.to_vec(),
        };

        self.types.insert(
            name.clone(),
            SyntheticTypeDef {
                name: name.clone(),
                kind: SyntheticTypeKind::InlineStruct,
                item,
            },
        );
        self.struct_dedup.insert(signature, name.clone());
        (name, true)
    }

    /// Registers an intersection enum and returns its name with dedup.
    ///
    /// Computes a canonical signature from the serde tag and variant structure
    /// (names, values, fields). If the same enum structure has been registered
    /// before, returns the existing name.
    ///
    /// Returns `(name, is_new)` where `is_new` is `false` on a dedup hit.
    pub fn register_intersection_enum(
        &mut self,
        serde_tag: Option<&str>,
        variants: Vec<EnumVariant>,
    ) -> (String, bool) {
        let signature = intersection_enum_signature(serde_tag, &variants);

        if let Some(existing_name) = self.intersection_enum_dedup.get(&signature) {
            return (existing_name.clone(), false);
        }

        let name = self.generate_name("Intersection");

        let item = Item::Enum {
            vis: Visibility::Public,
            name: name.clone(),
            type_params: vec![],
            serde_tag: serde_tag.map(|s| s.to_string()),
            variants,
        };

        self.types.insert(
            name.clone(),
            SyntheticTypeDef {
                name: name.clone(),
                kind: SyntheticTypeKind::UnionEnum,
                item,
            },
        );
        self.intersection_enum_dedup.insert(signature, name.clone());
        (name, true)
    }

    /// Registers a string literal enum and returns its name.
    ///
    /// Creates an enum with variants derived from string literal values.
    /// Uses deduplication based on the sorted set of string values.
    pub fn register_string_literal_enum(&mut self, name_hint: &str, values: &[String]) -> String {
        // Deduplication key
        let mut sorted = values.to_vec();
        sorted.sort();
        let signature = format!("string_enum:{}", sorted.join("|"));

        if let Some(existing_name) = self.union_dedup.get(&signature) {
            return existing_name.clone();
        }

        let base = crate::ir::string_to_pascal_case(name_hint);
        let name = if self.types.contains_key(&base) {
            self.generate_name(&base)
        } else {
            base
        };
        let variants = values
            .iter()
            .map(|v| EnumVariant {
                name: crate::ir::string_to_pascal_case(v),
                value: Some(crate::ir::EnumValue::Str(v.clone())),
                data: None,
                fields: vec![],
            })
            .collect();

        let item = Item::Enum {
            vis: Visibility::Public,
            name: name.clone(),
            type_params: vec![],
            serde_tag: None,
            variants,
        };

        self.types.insert(
            name.clone(),
            SyntheticTypeDef {
                name: name.clone(),
                kind: SyntheticTypeKind::UnionEnum,
                item,
            },
        );
        self.union_dedup.insert(signature, name.clone());
        name
    }

    /// Generates a unique synthetic name with the given prefix.
    ///
    /// Internal helper used by `register_intersection_enum` and
    /// `register_string_literal_enum`. Not intended for direct external use —
    /// prefer the `register_*` methods which provide structural deduplication.
    fn generate_name(&mut self, prefix: &str) -> String {
        let id = self.synthetic_counter;
        self.synthetic_counter += 1;
        format!("_{prefix}{id}")
    }

    /// Gets a synthetic type definition by name.
    pub fn get(&self, name: &str) -> Option<&SyntheticTypeDef> {
        self.types.get(name)
    }

    /// Registers an arbitrary synthetic item by name.
    ///
    /// Used for synthetic types that don't fit the union/struct/any-enum categories
    /// (e.g., stub traits, utility type structs, external type structs).
    ///
    /// # Deduplication semantics
    ///
    /// No structural signature-based dedup is performed. If an entry with the same
    /// `name` already exists, it is **overwritten** (last-write-wins). Callers are
    /// responsible for guarding against unintended overwrites when the same `name`
    /// could originate from multiple pipeline phases.
    pub fn push_item(&mut self, name: String, kind: SyntheticTypeKind, item: Item) {
        self.types
            .insert(name.clone(), SyntheticTypeDef { name, kind, item });
    }

    /// Returns all registered synthetic types as IR items.
    ///
    /// Iteration order is deterministic (name-sorted) because `types` is a `BTreeMap`.
    pub fn all_items(&self) -> Vec<&Item> {
        self.types.values().map(|def| &def.item).collect()
    }

    /// Merges another registry into this one.
    ///
    /// Items from `other` are added to `self`. If the same name exists in both,
    /// the entry from `other` overwrites.
    pub fn merge(&mut self, other: SyntheticTypeRegistry) {
        for (name, def) in other.types {
            self.types.insert(name.clone(), def);
        }
        for (sig, name) in other.union_dedup {
            self.union_dedup.insert(sig, name);
        }
        for (sig, name) in other.struct_dedup {
            self.struct_dedup.insert(sig, name);
        }
        for (sig, name) in other.intersection_enum_dedup {
            self.intersection_enum_dedup.insert(sig, name);
        }
        // Take the max counter to avoid name collisions
        self.synthetic_counter = self.synthetic_counter.max(other.synthetic_counter);
        self.struct_counter = self.struct_counter.max(other.struct_counter);
    }

    /// Creates a new registry that inherits deduplication state from `self`.
    ///
    /// The returned registry has no types registered, but knows the dedup signatures
    /// and counters from `self`. This prevents duplicate generation when a second pass
    /// (e.g., TypeResolver) processes the same file that already had synthetic types
    /// generated in a first pass (e.g., TypeCollector).
    pub fn fork_dedup_state(&self) -> Self {
        Self {
            types: BTreeMap::new(),
            union_dedup: self.union_dedup.clone(),
            struct_dedup: self.struct_dedup.clone(),
            intersection_enum_dedup: self.intersection_enum_dedup.clone(),
            struct_counter: self.struct_counter,
            synthetic_counter: self.synthetic_counter,
            type_param_scope: Vec::new(),
        }
    }

    /// Applies monomorphization substitutions to all registered synthetic items.
    ///
    /// Iterates all items and substitutes type parameter references in fields,
    /// variant data, etc. with their concrete types from the substitution map.
    pub fn apply_substitutions_to_items(
        &mut self,
        subs: &std::collections::HashMap<String, RustType>,
    ) {
        if subs.is_empty() {
            return;
        }
        for def in self.types.values_mut() {
            def.item = def.item.substitute(subs);
        }
    }

    /// Consumes the registry and returns all items as owned values.
    ///
    /// Iteration order is deterministic (name-sorted) because `types` is a `BTreeMap`.
    pub fn into_items(self) -> Vec<Item> {
        self.types.into_values().map(|def| def.item).collect()
    }
}

impl Default for SyntheticTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Computes a canonical signature for a union type (sorted member types).
fn union_signature(member_types: &[RustType]) -> String {
    let mut names: Vec<String> = member_types.iter().map(|t| format!("{t:?}")).collect();
    names.sort();
    format!("union:{}", names.join(","))
}

/// Computes a canonical signature for an inline struct (sorted fields).
///
/// Field names are normalized via `sanitize_field_name` so that raw names
/// (e.g., `"my-field"`) and pre-sanitized names (e.g., `"my_field"`) produce
/// the same signature. This enables cross-origin deduplication between
/// TypeLit structs and intersection structs.
fn struct_signature(fields: &[(String, RustType)]) -> String {
    let mut parts: Vec<String> = fields
        .iter()
        .map(|(name, ty)| format!("{}:{ty:?}", sanitize_field_name(name)))
        .collect();
    parts.sort();
    format!("struct:{}", parts.join(","))
}

/// Computes a canonical signature for an intersection enum.
///
/// The signature includes the serde tag and all variant details (name, value, fields)
/// in sorted order, ensuring order-independent deduplication.
fn intersection_enum_signature(serde_tag: Option<&str>, variants: &[EnumVariant]) -> String {
    let tag_part = serde_tag.unwrap_or("none");
    let mut variant_parts: Vec<String> = variants
        .iter()
        .map(|v| {
            let value_part = match &v.value {
                Some(EnumValue::Str(s)) => format!("={s}"),
                Some(EnumValue::Number(n)) => format!("={n}"),
                Some(EnumValue::Expr(e)) => format!("=expr({e})"),
                None => String::new(),
            };
            let mut field_strs: Vec<String> = v
                .fields
                .iter()
                .map(|f| format!("{}:{:?}", sanitize_field_name(&f.name), f.ty))
                .collect();
            field_strs.sort();
            let data_part = v
                .data
                .as_ref()
                .map(|d| format!("({d:?})"))
                .unwrap_or_default();
            format!(
                "{}{}{}[{}]",
                v.name,
                value_part,
                data_part,
                field_strs.join(",")
            )
        })
        .collect();
    variant_parts.sort();
    format!("intersection_enum:{tag_part}:{}", variant_parts.join("|"))
}

/// Generates a union enum name from member types (e.g., `StringOrF64`).
fn generate_union_name(member_types: &[RustType]) -> String {
    let mut names: Vec<String> = member_types.iter().map(variant_name_for_type).collect();
    names.sort();
    names.join("Or")
}

/// Returns a variant name for a RustType (e.g., `String` → `String`, `f64` → `F64`).
///
/// For compound types, recursively includes inner type information to avoid
/// name collisions (e.g., `Named("Foo", [String])` → `"FooString"`,
/// `Tuple([String, F64])` → `"TupleStringF64"`).
///
/// For path-qualified types (e.g., `serde_json::Value`), extracts the last segment
/// to produce a valid Rust identifier (e.g., `Value`).
pub(crate) fn variant_name_for_type(ty: &RustType) -> String {
    match ty {
        RustType::String => "String".to_string(),
        RustType::F64 => "F64".to_string(),
        RustType::Bool => "Bool".to_string(),
        RustType::Unit => "Unit".to_string(),
        RustType::Any => "Any".to_string(),
        RustType::Never => "Never".to_string(),
        RustType::Vec(inner) => format!("Vec{}", variant_name_for_type(inner)),
        RustType::Option(inner) => format!("Option{}", variant_name_for_type(inner)),
        RustType::Named { name, type_args } => {
            let base = match name.rsplit_once("::") {
                Some((_, last)) => last,
                None => name.as_str(),
            };
            if type_args.is_empty() {
                base.to_string()
            } else {
                let args: Vec<String> = type_args.iter().map(variant_name_for_type).collect();
                format!("{base}{}", args.join(""))
            }
        }
        RustType::Ref(inner) => variant_name_for_type(inner),
        RustType::DynTrait(name) => match name.rsplit_once("::") {
            Some((_, last)) => last.to_string(),
            None => name.clone(),
        },
        RustType::Fn { return_type, .. } => {
            format!("Fn{}", variant_name_for_type(return_type))
        }
        RustType::Result { ok, err } => {
            format!(
                "Result{}{}",
                variant_name_for_type(ok),
                variant_name_for_type(err)
            )
        }
        RustType::Tuple(elems) => {
            if elems.is_empty() {
                "Tuple".to_string()
            } else {
                let parts: Vec<String> = elems.iter().map(variant_name_for_type).collect();
                format!("Tuple{}", parts.join(""))
            }
        }
        RustType::QSelf {
            qself,
            trait_ref,
            item,
        } => {
            // QSelf は union variant 名としては trait::item の連結を採用する。
            // 例: `<T as Promise>::Output` → `PromiseOutput`
            let trait_short = trait_ref
                .name
                .rsplit("::")
                .next()
                .unwrap_or(&trait_ref.name);
            format!("{}{trait_short}{item}", variant_name_for_type(qself))
        }
    }
}

/// Converts a string to PascalCase.
use super::any_narrowing::to_pascal_case;

#[cfg(test)]
mod tests;
