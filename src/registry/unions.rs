//! string literal union / discriminated union の検出・収集。

use std::collections::HashMap;

use swc_ecma_ast as ast;

use super::{TypeDef, TypeRegistry};
use crate::ir::RustType;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::SyntheticTypeRegistry;

/// string literal union type alias を検出し、`TypeDef::Enum` を返す。
///
/// `type Direction = "up" | "down"` のように、全メンバーが文字列リテラルの union type を検出する。
pub(super) fn try_collect_string_literal_union(alias: &ast::TsTypeAliasDecl) -> Option<TypeDef> {
    use crate::pipeline::type_converter::string_to_pascal_case;

    let union = match alias.type_ann.as_ref() {
        ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return None,
    };

    let mut variants = Vec::new();
    let mut string_values = HashMap::new();
    for ty in &union.types {
        match ty.as_ref() {
            ast::TsType::TsLitType(lit) => match &lit.lit {
                swc_ecma_ast::TsLit::Str(s) => {
                    let value = s.value.to_string_lossy().into_owned();
                    let variant_name = string_to_pascal_case(&value);
                    string_values.insert(value, variant_name.clone());
                    variants.push(variant_name);
                }
                _ => return None,
            },
            _ => return None,
        }
    }

    Some(TypeDef::Enum {
        type_params: vec![],
        variants,
        string_values,
        tag_field: None,
        variant_fields: HashMap::new(),
    })
}

/// discriminated union type alias を検出し、`TypeDef::Enum` を返す。
///
/// `type Shape = { kind: "circle", r: number } | { kind: "square", s: number }` を検出する。
/// 全メンバーがオブジェクト型リテラルで、共通の文字列リテラル discriminant フィールドを持つ場合に該当。
pub(super) fn try_collect_discriminated_union(
    alias: &ast::TsTypeAliasDecl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<TypeDef> {
    use crate::pipeline::type_converter::string_to_pascal_case;

    let union = match alias.type_ann.as_ref() {
        ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return None,
    };

    // All members must be object type literals
    let type_lits: Vec<&swc_ecma_ast::TsTypeLit> = union
        .types
        .iter()
        .filter_map(|ty| match ty.as_ref() {
            ast::TsType::TsTypeLit(lit) => Some(lit),
            _ => None,
        })
        .collect();

    if type_lits.len() != union.types.len() || type_lits.len() < 2 {
        return None;
    }

    // Find a common discriminant field with string literal types in all members
    let tag = find_registry_discriminant_field(&type_lits)?;

    let mut variants = Vec::new();
    let mut string_values = HashMap::new();
    let mut variant_fields_map = HashMap::new();

    for type_lit in &type_lits {
        let (disc_value, fields) =
            extract_registry_variant_info(type_lit, &tag, lookup, synthetic)?;
        let variant_name = string_to_pascal_case(&disc_value);
        string_values.insert(disc_value, variant_name.clone());
        variant_fields_map.insert(variant_name.clone(), fields);
        variants.push(variant_name);
    }

    Some(TypeDef::Enum {
        type_params: vec![],
        variants,
        string_values,
        tag_field: Some(tag),
        variant_fields: variant_fields_map,
    })
}

/// discriminated union の discriminant フィールドを見つける。
///
/// 全メンバーに共通し、すべて文字列リテラル型であるフィールド名を返す。
fn find_registry_discriminant_field(type_lits: &[&swc_ecma_ast::TsTypeLit]) -> Option<String> {
    let first = type_lits[0];
    for member in &first.members {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            let name = match prop.key.as_ref() {
                ast::Expr::Ident(ident) => ident.sym.to_string(),
                _ => continue,
            };
            // Check if this field has a string literal type in all members
            let is_discriminant = type_lits.iter().all(|lit| {
                lit.members.iter().any(|m| {
                    if let ast::TsTypeElement::TsPropertySignature(p) = m {
                        let field_name = match p.key.as_ref() {
                            ast::Expr::Ident(id) => id.sym.to_string(),
                            _ => return false,
                        };
                        if field_name != name {
                            return false;
                        }
                        // Check if type annotation is a string literal
                        if let Some(ann) = &p.type_ann {
                            matches!(
                                ann.type_ann.as_ref(),
                                ast::TsType::TsLitType(lit) if matches!(&lit.lit, swc_ecma_ast::TsLit::Str(_))
                            )
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
            });
            if is_discriminant {
                return Some(name);
            }
        }
    }
    None
}

/// discriminated union の 1 つのバリアントから discriminant 値と非 discriminant フィールドを抽出する。
fn extract_registry_variant_info(
    type_lit: &swc_ecma_ast::TsTypeLit,
    tag_field: &str,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<(String, Vec<(String, RustType)>)> {
    let mut disc_value = None;
    let mut fields = Vec::new();

    for member in &type_lit.members {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            let name = match prop.key.as_ref() {
                ast::Expr::Ident(ident) => ident.sym.to_string(),
                _ => continue,
            };
            if name == tag_field {
                // Extract string literal value
                if let Some(ann) = &prop.type_ann {
                    if let ast::TsType::TsLitType(lit) = ann.type_ann.as_ref() {
                        if let swc_ecma_ast::TsLit::Str(s) = &lit.lit {
                            disc_value = Some(s.value.to_string_lossy().into_owned());
                        }
                    }
                }
            } else {
                // Non-discriminant field: convert type
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_ts_type(&ann.type_ann, synthetic, lookup) {
                        let ty = if prop.optional {
                            RustType::Option(Box::new(ty))
                        } else {
                            ty
                        };
                        fields.push((name, ty));
                    }
                }
            }
        }
    }

    Some((disc_value?, fields))
}
