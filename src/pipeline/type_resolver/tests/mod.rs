use super::*;
use crate::pipeline::{parse_files, SyntheticTypeRegistry};
use crate::registry::{build_registry, MethodKind, MethodSignature, TypeDef, TypeRegistry};
use std::path::PathBuf;

mod basics;
mod complex_features;
mod destructuring_types;
mod du_analysis;
mod expected_types;
mod narrowing;
mod prd_2_7;

pub(super) fn resolve(source: &str) -> FileTypeResolution {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    resolver.resolve_file(file)
}

/// Helper: resolve with a pre-built registry for struct/enum definitions.
pub(super) fn resolve_with_reg(source: &str, reg: &TypeRegistry) -> FileTypeResolution {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(reg, &mut synthetic);
    resolver.resolve_file(file)
}

pub(super) fn resolve_with_reg_and_synthetic(
    source: &str,
    reg: &TypeRegistry,
) -> (FileTypeResolution, SyntheticTypeRegistry) {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(reg, &mut synthetic);
    let result = resolver.resolve_file(file);
    (result, synthetic)
}

/// Resolve with the full pipeline including any_enum_analyzer.
///
/// Runs `analyze_any_enums` before TypeResolver, matching the production pipeline.
pub(super) fn resolve_with_any_analysis(
    source: &str,
) -> (FileTypeResolution, SyntheticTypeRegistry) {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolution = FileTypeResolution::empty();

    crate::pipeline::any_enum_analyzer::analyze_any_enums(
        &file.module,
        &mut resolution,
        &mut synthetic,
    );

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    resolver.set_any_enum_overrides(resolution.any_enum_overrides);
    let result = resolver.resolve_file(file);
    (result, synthetic)
}

pub(super) fn resolve_with_synthetic(source: &str) -> (FileTypeResolution, SyntheticTypeRegistry) {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let result = resolver.resolve_file(file);
    (result, synthetic)
}

pub(super) fn build_shape_registry() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64).into()],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![
            ("width".to_string(), RustType::F64).into(),
            ("height".to_string(), RustType::F64).into(),
        ],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );
    reg
}

/// Resolves with built-in type definitions (ECMAScript + Web API) loaded.
///
/// This enables testing TypeResolver behavior with real Array/String/Promise methods
/// from ecmascript.json and web_api.json, rather than manually constructed MethodSignatures.
pub(super) fn resolve_with_builtins(source: &str) -> FileTypeResolution {
    let (builtin_reg, _base_synthetic) = crate::external_types::load_builtin_types().unwrap();
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let mut reg = build_registry(&file.module);
    reg.merge(&builtin_reg);
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    resolver.resolve_file(file)
}

pub(super) fn make_sig(param_types: Vec<RustType>, ret: Option<RustType>) -> MethodSignature {
    MethodSignature {
        params: param_types
            .into_iter()
            .enumerate()
            .map(|(i, ty)| (format!("p{i}"), ty))
            .map(Into::into)
            .collect(),
        return_type: ret,
        has_rest: false,
        type_params: vec![],
        kind: MethodKind::Method,
    }
}
