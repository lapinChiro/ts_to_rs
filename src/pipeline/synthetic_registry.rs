//! Central registry for synthetic (compiler-generated) types.
//!
//! Manages union enums, any-type enums, and inline structs with
//! deduplication based on semantic signatures.

use std::collections::{BTreeMap, HashMap};

use crate::ir::{sanitize_field_name, EnumVariant, Item, RustType, StructField, Visibility};

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
    /// Counter for generating unique inline struct names.
    struct_counter: u32,
    /// Counter for generating unique synthetic type names (e.g., _TypeLit0, _Intersection1).
    /// Replaces the global `SYNTHETIC_COUNTER` in transformer/types/mod.rs.
    synthetic_counter: u32,
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
    /// A union type enum (e.g., `string | number` → `StringOrF64`).
    UnionEnum,
    /// An any-type materialization enum (e.g., `ProcessDataInputType`).
    AnyEnum,
    /// An inline type literal struct (e.g., `{ x: number }` → `_TypeLit0`).
    InlineStruct,
    /// An impl block for a synthetic or named struct.
    ImplBlock,
}

impl SyntheticTypeRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            types: BTreeMap::new(),
            union_dedup: HashMap::new(),
            struct_dedup: HashMap::new(),
            struct_counter: 0,
            synthetic_counter: 0,
        }
    }

    /// Registers a union type enum and returns its name.
    ///
    /// If the same combination of member types has been registered before,
    /// returns the existing name (idempotent deduplication).
    pub fn register_union(&mut self, member_types: &[RustType]) -> String {
        let signature = union_signature(member_types);

        if let Some(existing_name) = self.union_dedup.get(&signature) {
            return existing_name.clone();
        }

        let name = generate_union_name(member_types);
        let variants = member_types
            .iter()
            .map(|ty| EnumVariant {
                name: variant_name_for_type(ty),
                value: None,
                data: Some(ty.clone()),
                fields: vec![],
            })
            .collect();

        let item = Item::Enum {
            vis: Visibility::Public,
            name: name.clone(),
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

        let item = Item::Enum {
            vis: Visibility::Public,
            name: name.clone(),
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
    pub fn register_inline_struct(&mut self, fields: &[(String, RustType)]) -> String {
        let signature = struct_signature(fields);

        if let Some(existing_name) = self.struct_dedup.get(&signature) {
            return existing_name.clone();
        }

        let name = format!("_TypeLit{}", self.struct_counter);
        self.struct_counter += 1;

        let struct_fields = fields
            .iter()
            .map(|(name, ty)| StructField {
                vis: None,
                name: sanitize_field_name(name),
                ty: ty.clone(),
            })
            .collect();

        let item = Item::Struct {
            vis: Visibility::Public,
            name: name.clone(),
            type_params: vec![],
            fields: struct_fields,
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
        name
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

        let base = crate::pipeline::type_converter::string_to_pascal_case(name_hint);
        let name = if self.types.contains_key(&base) {
            self.generate_name(&base)
        } else {
            base
        };
        let variants = values
            .iter()
            .map(|v| EnumVariant {
                name: crate::pipeline::type_converter::string_to_pascal_case(v),
                value: Some(crate::ir::EnumValue::Str(v.clone())),
                data: None,
                fields: vec![],
            })
            .collect();

        let item = Item::Enum {
            vis: Visibility::Public,
            name: name.clone(),
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
    /// Replaces the global `SYNTHETIC_COUNTER` / `generate_synthetic_name` in
    /// `transformer/types/mod.rs`. Each `SyntheticTypeRegistry` instance has
    /// its own counter, eliminating the need for `reset_synthetic_counter()`.
    pub fn generate_name(&mut self, prefix: &str) -> String {
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
    /// (e.g., stub traits, utility type structs). No deduplication is performed.
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
            struct_counter: self.struct_counter,
            synthetic_counter: self.synthetic_counter,
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
fn struct_signature(fields: &[(String, RustType)]) -> String {
    let mut parts: Vec<String> = fields
        .iter()
        .map(|(name, ty)| format!("{name}:{ty:?}"))
        .collect();
    parts.sort();
    format!("struct:{}", parts.join(","))
}

/// Generates a union enum name from member types (e.g., `StringOrF64`).
fn generate_union_name(member_types: &[RustType]) -> String {
    let mut names: Vec<String> = member_types.iter().map(variant_name_for_type).collect();
    names.sort();
    names.join("Or")
}

/// Returns a variant name for a RustType (e.g., `String` → `String`, `f64` → `F64`).
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
        RustType::Named { name, .. } => match name.rsplit_once("::") {
            Some((_, last)) => last.to_string(),
            None => name.clone(),
        },
        RustType::Ref(inner) => variant_name_for_type(inner),
        RustType::DynTrait(name) => match name.rsplit_once("::") {
            Some((_, last)) => last.to_string(),
            None => name.clone(),
        },
        RustType::Fn { .. } => "Fn".to_string(),
        RustType::Result { .. } => "Result".to_string(),
        RustType::Tuple(_) => "Tuple".to_string(),
    }
}

/// Converts a string to PascalCase.
use super::any_narrowing::to_pascal_case;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_union_basic() {
        let mut reg = SyntheticTypeRegistry::new();
        let name = reg.register_union(&[RustType::String, RustType::F64]);
        assert!(!name.is_empty());
        assert!(reg.get(&name).is_some());
    }

    #[test]
    fn test_register_union_idempotent() {
        let mut reg = SyntheticTypeRegistry::new();
        let name1 = reg.register_union(&[RustType::String, RustType::F64]);
        let name2 = reg.register_union(&[RustType::String, RustType::F64]);
        assert_eq!(name1, name2, "same union should return same name");
    }

    #[test]
    fn test_register_union_order_independent() {
        let mut reg = SyntheticTypeRegistry::new();
        let name1 = reg.register_union(&[RustType::String, RustType::F64]);
        let name2 = reg.register_union(&[RustType::F64, RustType::String]);
        assert_eq!(
            name1, name2,
            "same members in different order should return same name"
        );
    }

    #[test]
    fn test_register_union_different_types_get_different_names() {
        let mut reg = SyntheticTypeRegistry::new();
        let name1 = reg.register_union(&[RustType::String, RustType::F64]);
        let name2 = reg.register_union(&[RustType::String, RustType::Bool]);
        assert_ne!(name1, name2);
    }

    #[test]
    fn test_register_union_name_format() {
        let mut reg = SyntheticTypeRegistry::new();
        let name = reg.register_union(&[RustType::String, RustType::F64]);
        // Names are sorted alphabetically: F64 comes before String
        assert_eq!(name, "F64OrString");
    }

    #[test]
    fn test_register_inline_struct_basic() {
        let mut reg = SyntheticTypeRegistry::new();
        let name = reg.register_inline_struct(&[
            ("x".to_string(), RustType::F64),
            ("y".to_string(), RustType::String),
        ]);
        assert_eq!(name, "_TypeLit0");
        assert!(reg.get(&name).is_some());
    }

    #[test]
    fn test_register_inline_struct_idempotent() {
        let mut reg = SyntheticTypeRegistry::new();
        let name1 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
        let name2 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
        assert_eq!(name1, name2);
    }

    #[test]
    fn test_register_inline_struct_different_fields() {
        let mut reg = SyntheticTypeRegistry::new();
        let name1 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
        let name2 = reg.register_inline_struct(&[("y".to_string(), RustType::String)]);
        assert_ne!(name1, name2);
        assert_eq!(name1, "_TypeLit0");
        assert_eq!(name2, "_TypeLit1");
    }

    #[test]
    fn test_register_any_enum() {
        let mut reg = SyntheticTypeRegistry::new();
        let name = reg.register_any_enum(
            "processData",
            "input",
            vec![EnumVariant {
                name: "String".to_string(),
                value: None,
                data: Some(RustType::String),
                fields: vec![],
            }],
        );
        assert_eq!(name, "ProcessDataInputType");
        assert!(reg.get(&name).is_some());
    }

    #[test]
    fn test_all_items_returns_all_registered() {
        let mut reg = SyntheticTypeRegistry::new();
        reg.register_union(&[RustType::String, RustType::F64]);
        reg.register_inline_struct(&[("x".to_string(), RustType::Bool)]);
        reg.register_any_enum(
            "foo",
            "bar",
            vec![EnumVariant {
                name: "String".to_string(),
                value: None,
                data: Some(RustType::String),
                fields: vec![],
            }],
        );
        let items = reg.all_items();
        assert_eq!(items.len(), 3, "should have 3 synthetic types");
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let reg = SyntheticTypeRegistry::new();
        assert!(reg.get("NonExistent").is_none());
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("process_data"), "ProcessData");
        assert_eq!(to_pascal_case("processData"), "ProcessData");
        assert_eq!(to_pascal_case("hono-base"), "HonoBase");
    }

    #[test]
    fn test_union_generates_enum_item() {
        let mut reg = SyntheticTypeRegistry::new();
        let name = reg.register_union(&[RustType::String, RustType::F64]);
        let def = reg.get(&name).unwrap();
        match &def.item {
            Item::Enum { variants, .. } => {
                assert_eq!(variants.len(), 2);
            }
            _ => panic!("expected Item::Enum"),
        }
    }

    #[test]
    fn test_inline_struct_generates_struct_item() {
        let mut reg = SyntheticTypeRegistry::new();
        let name = reg.register_inline_struct(&[
            ("x".to_string(), RustType::F64),
            ("y".to_string(), RustType::String),
        ]);
        let def = reg.get(&name).unwrap();
        match &def.item {
            Item::Struct { fields, .. } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[1].name, "y");
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_generate_name_increments() {
        let mut reg = SyntheticTypeRegistry::new();
        assert_eq!(reg.generate_name("TypeLit"), "_TypeLit0");
        assert_eq!(reg.generate_name("TypeLit"), "_TypeLit1");
        assert_eq!(reg.generate_name("Intersection"), "_Intersection2");
    }

    #[test]
    fn test_generate_name_independent_per_instance() {
        let mut reg1 = SyntheticTypeRegistry::new();
        let mut reg2 = SyntheticTypeRegistry::new();
        assert_eq!(reg1.generate_name("TypeLit"), "_TypeLit0");
        assert_eq!(reg2.generate_name("TypeLit"), "_TypeLit0");
    }

    #[test]
    fn test_merge_combines_types() {
        let mut reg1 = SyntheticTypeRegistry::new();
        reg1.register_union(&[RustType::String, RustType::F64]);

        let mut reg2 = SyntheticTypeRegistry::new();
        reg2.register_inline_struct(&[("x".to_string(), RustType::Bool)]);

        reg1.merge(reg2);
        assert_eq!(reg1.all_items().len(), 2);
    }

    #[test]
    fn test_merge_preserves_dedup() {
        let mut reg1 = SyntheticTypeRegistry::new();
        let name1 = reg1.register_union(&[RustType::String, RustType::F64]);

        let mut reg2 = SyntheticTypeRegistry::new();
        let name2 = reg2.register_union(&[RustType::String, RustType::F64]);

        assert_eq!(name1, name2); // Same name independently

        reg1.merge(reg2);
        // Should still be 1 item (dedup)
        let union_count = reg1
            .all_items()
            .iter()
            .filter(|item| matches!(item, Item::Enum { .. }))
            .count();
        assert_eq!(union_count, 1);
    }

    #[test]
    fn test_merge_updates_counters() {
        let mut reg1 = SyntheticTypeRegistry::new();
        reg1.generate_name("TypeLit"); // counter = 1

        let mut reg2 = SyntheticTypeRegistry::new();
        reg2.generate_name("TypeLit"); // counter = 1
        reg2.generate_name("TypeLit"); // counter = 2
        reg2.generate_name("TypeLit"); // counter = 3

        reg1.merge(reg2);
        // After merge, counter should be max(1, 3) = 3
        assert_eq!(reg1.generate_name("TypeLit"), "_TypeLit3");
    }

    #[test]
    fn test_variant_name_named_with_path_uses_last_segment() {
        let ty = RustType::Named {
            name: "serde_json::Value".to_string(),
            type_args: vec![],
        };
        assert_eq!(variant_name_for_type(&ty), "Value");
    }

    #[test]
    fn test_variant_name_named_without_path_unchanged() {
        let ty = RustType::Named {
            name: "String".to_string(),
            type_args: vec![],
        };
        assert_eq!(variant_name_for_type(&ty), "String");
    }

    #[test]
    fn test_variant_name_dyn_trait_with_path_uses_last_segment() {
        let ty = RustType::DynTrait("std::fmt::Display".to_string());
        assert_eq!(variant_name_for_type(&ty), "Display");
    }

    #[test]
    fn test_variant_name_dyn_trait_without_path_unchanged() {
        let ty = RustType::DynTrait("Fn".to_string());
        assert_eq!(variant_name_for_type(&ty), "Fn");
    }

    #[test]
    fn test_union_name_with_path_type_produces_valid_identifier() {
        let mut reg = SyntheticTypeRegistry::new();
        let name = reg.register_union(&[
            RustType::String,
            RustType::Named {
                name: "serde_json::Value".to_string(),
                type_args: vec![],
            },
        ]);
        assert_eq!(name, "StringOrValue");
    }
}
