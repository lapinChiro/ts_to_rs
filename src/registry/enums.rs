//! 合成 enum 登録。

use std::collections::HashMap;

use super::{TypeDef, TypeRegistry};
use crate::ir::Item;
use crate::pipeline::SyntheticTypeRegistry;

/// Registers enum items from a `SyntheticTypeRegistry` into the `TypeRegistry`.
///
/// Skips enums already present in the registry (avoids overwriting declared types).
pub(crate) fn register_extra_enums(reg: &mut TypeRegistry, synthetic: &SyntheticTypeRegistry) {
    for item in synthetic.all_items() {
        register_single_enum(reg, item);
    }
}

/// Registers a single enum item in the TypeRegistry if not already present.
fn register_single_enum(reg: &mut TypeRegistry, item: &Item) {
    if let Item::Enum { name, variants, .. } = item {
        if reg.get(name).is_some() {
            return;
        }
        let variant_names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
        reg.register(
            name.clone(),
            TypeDef::Enum {
                type_params: vec![],
                variants: variant_names,
                string_values: HashMap::new(),
                tag_field: None,
                variant_fields: HashMap::new(),
            },
        );
    }
}
