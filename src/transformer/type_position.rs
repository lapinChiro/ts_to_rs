//! Type position utilities for trait type wrapping.

use crate::ir::{RustType, StdCollectionKind};
use crate::registry::TypeRegistry;

/// Position where a type annotation appears. Determines trait type wrapping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypePosition {
    /// Function parameter: trait types become `&dyn Trait`
    Param,
    /// Value position (variable declaration, return type): trait types become `Box<dyn Trait>`
    Value,
    /// General expression position: no trait wrapping applied
    General,
}

/// Wraps a trait type based on position.
///
/// - `Param` → `&dyn Trait`
/// - `Value` → `Box<dyn Trait>`
/// - `General` → unchanged
///
/// Non-trait types are returned unchanged regardless of position.
///
/// **I-387**: `RustType::Named` のみが trait 判定対象。`TypeVar` / `Primitive` /
/// `StdCollection` は構造上 user 定義 trait ではないため wrap 対象外。
pub(crate) fn wrap_trait_for_position(
    ty: RustType,
    position: TypePosition,
    reg: &TypeRegistry,
) -> RustType {
    if let RustType::Named {
        ref name,
        ref type_args,
    } = ty
    {
        if type_args.is_empty() && reg.is_trait_type(name) {
            return match position {
                TypePosition::Param => RustType::Ref(Box::new(RustType::DynTrait(name.clone()))),
                // I-387: Box wrapper を StdCollection で構造化。
                TypePosition::Value => RustType::StdCollection {
                    kind: StdCollectionKind::Box,
                    args: vec![RustType::DynTrait(name.clone())],
                },
                TypePosition::General => ty,
            };
        }
    }
    ty
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::registry::{MethodSignature, TypeDef};

    fn make_trait_registry() -> TypeRegistry {
        let mut reg = TypeRegistry::new();
        let mut methods = HashMap::new();
        methods.insert(
            "greet".to_string(),
            vec![MethodSignature {
                params: vec![],
                return_type: None,
                has_rest: false,
                type_params: vec![],
            }],
        );
        reg.register(
            "Greeter".to_string(),
            TypeDef::new_interface(vec![], vec![], methods, vec![]),
        );
        reg
    }

    #[test]
    fn test_wrap_trait_for_position_param_wraps_as_ref_dyn() {
        let reg = make_trait_registry();
        let ty = RustType::Named {
            name: "Greeter".to_string(),
            type_args: vec![],
        };
        let result = wrap_trait_for_position(ty, TypePosition::Param, &reg);
        assert_eq!(
            result,
            RustType::Ref(Box::new(RustType::DynTrait("Greeter".to_string())))
        );
    }

    #[test]
    fn test_wrap_trait_for_position_value_wraps_as_box_dyn() {
        let reg = make_trait_registry();
        let ty = RustType::Named {
            name: "Greeter".to_string(),
            type_args: vec![],
        };
        let result = wrap_trait_for_position(ty, TypePosition::Value, &reg);
        // I-387: Box wrapper を StdCollection で構造化。
        assert_eq!(
            result,
            RustType::StdCollection {
                kind: StdCollectionKind::Box,
                args: vec![RustType::DynTrait("Greeter".to_string())],
            }
        );
    }

    #[test]
    fn test_wrap_trait_for_position_general_no_wrap() {
        let reg = make_trait_registry();
        let ty = RustType::Named {
            name: "Greeter".to_string(),
            type_args: vec![],
        };
        let result = wrap_trait_for_position(ty.clone(), TypePosition::General, &reg);
        assert_eq!(result, ty);
    }

    #[test]
    fn test_wrap_trait_for_position_non_trait_unchanged() {
        let reg = make_trait_registry();
        for position in [
            TypePosition::Param,
            TypePosition::Value,
            TypePosition::General,
        ] {
            let result = wrap_trait_for_position(RustType::String, position, &reg);
            assert_eq!(result, RustType::String);
        }
    }

    // --- I-387 T4c: TypeVar / Primitive / StdCollection は trait wrap 対象外 ---

    #[test]
    fn test_wrap_trait_for_position_type_var_unchanged() {
        let reg = make_trait_registry();
        let ty = RustType::TypeVar {
            name: "T".to_string(),
        };
        for position in [
            TypePosition::Param,
            TypePosition::Value,
            TypePosition::General,
        ] {
            assert_eq!(wrap_trait_for_position(ty.clone(), position, &reg), ty);
        }
    }

    #[test]
    fn test_wrap_trait_for_position_primitive_unchanged() {
        let reg = make_trait_registry();
        let ty = RustType::Primitive(crate::ir::PrimitiveIntKind::Usize);
        for position in [
            TypePosition::Param,
            TypePosition::Value,
            TypePosition::General,
        ] {
            assert_eq!(wrap_trait_for_position(ty.clone(), position, &reg), ty);
        }
    }

    #[test]
    fn test_wrap_trait_for_position_std_collection_unchanged() {
        let reg = make_trait_registry();
        let ty = RustType::StdCollection {
            kind: StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        };
        for position in [
            TypePosition::Param,
            TypePosition::Value,
            TypePosition::General,
        ] {
            assert_eq!(wrap_trait_for_position(ty.clone(), position, &reg), ty);
        }
    }
}
