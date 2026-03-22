//! Unified transformation pipeline.
//!
//! This module provides the new multi-pass pipeline architecture:
//! Parse → ModuleGraph → TypeCollection → TypeResolution → Transform → Generate → Output.

pub mod module_graph;
pub mod module_resolver;
pub mod output_writer;
pub mod synthetic_registry;
pub mod type_converter;
pub mod type_resolution;
pub mod type_resolver;
mod types;

pub use module_graph::{ExportOrigin, ModuleGraph, ModuleGraphBuilder, ResolvedImport};
pub use synthetic_registry::{SyntheticTypeDef, SyntheticTypeKind, SyntheticTypeRegistry};
pub use types::{
    FileOutput, ModuleResolver, NullModuleResolver, ParsedFile, ParsedFiles, ResolvedType,
    TranspileInput, TranspileOutput,
};

use anyhow::{Context, Result};

/// Parses multiple TypeScript source files into a shared `ParsedFiles` collection.
///
/// Each file is parsed independently. If any file fails to parse, the entire
/// operation returns an error.
///
/// # Errors
///
/// Returns an error if any file fails to parse.
pub fn parse_files(files: Vec<(std::path::PathBuf, String)>) -> Result<ParsedFiles> {
    let mut parsed = Vec::with_capacity(files.len());
    for (path, source) in files {
        let module = crate::parser::parse_typescript(&source)
            .with_context(|| format!("failed to parse: {}", path.display()))?;
        parsed.push(ParsedFile {
            path,
            source,
            module,
        });
    }
    Ok(ParsedFiles { files: parsed })
}

/// 統一変換パイプライン。全モードで同一のコードパスを通る。
///
/// Pass 0: Parse → Pass 1: ModuleGraph → Pass 2: TypeCollection →
/// Pass 3: TypeResolution → Pass 4-5: Transform + Generate
///
/// # Errors
///
/// Returns an error if parsing or transformation fails.
pub fn transpile_pipeline(input: TranspileInput) -> Result<TranspileOutput> {
    // Pass 0: Parse
    let parsed = parse_files(input.files)?;

    // Pass 1: Module Graph
    let root_dir = find_common_root(&parsed);
    let module_graph = ModuleGraphBuilder::new(&parsed, &*input.module_resolver, &root_dir).build();

    // Pass 2: Type Collection (shared registry from all files)
    let mut shared_registry = input.builtin_types.unwrap_or_default();
    for file in &parsed.files {
        let file_registry = crate::registry::build_registry(&file.module);
        shared_registry.merge(&file_registry);
    }

    // Pass 3: Type Resolution (all files first, so SyntheticTypeRegistry becomes immutable before Transform)
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut type_resolutions = Vec::with_capacity(parsed.files.len());
    for file in &parsed.files {
        let type_resolution = {
            let mut resolver =
                type_resolver::TypeResolver::new(&shared_registry, &mut synthetic, &module_graph);
            resolver.resolve_file(file)
        };
        type_resolutions.push(type_resolution);
    }

    // Pass 4-5: Transformation + Code Generation (per file)
    let mut file_outputs = Vec::new();
    for (file, type_resolution) in parsed.files.iter().zip(type_resolutions.iter()) {
        let tctx = crate::transformer::context::TransformContext::new(
            &module_graph,
            &shared_registry,
            type_resolution,
            &file.path,
        );
        let mut file_synthetic = SyntheticTypeRegistry::new();
        let (items, unsupported) = crate::transformer::transform_module_collecting_with_path(
            &file.module,
            &tctx,
            &shared_registry,
            tctx.file_path.parent().and_then(|p| p.to_str()),
            &mut file_synthetic,
        )?;

        // per-file synthetic types をファイル出力に含める（旧 API 互換）
        // 同時に共有 synthetic にも蓄積する（OutputWriter 用）
        let file_synthetic_items: Vec<crate::ir::Item> =
            file_synthetic.all_items().into_iter().cloned().collect();
        synthetic.merge(file_synthetic);

        let mut all_items = file_synthetic_items;
        all_items.extend(items);
        let rust_source = crate::generator::generate(&all_items);

        file_outputs.push(FileOutput {
            path: file.path.with_extension("rs"),
            rust_source,
            unsupported,
        });
    }

    let synthetic_items = synthetic.into_items();

    Ok(TranspileOutput {
        files: file_outputs,
        module_graph,
        synthetic_items,
    })
}

/// 単一ファイルの簡易 API。
///
/// 内部で `TranspileInput` を構築し、統一パイプラインを呼ぶ。
///
/// # Errors
///
/// Returns an error if parsing or transformation fails.
pub fn transpile_single(source: &str) -> Result<String> {
    let input = TranspileInput {
        files: vec![(std::path::PathBuf::from("input.ts"), source.to_string())],
        builtin_types: None,
        module_resolver: Box::new(NullModuleResolver),
    };
    let output = transpile_pipeline(input)?;
    Ok(output
        .files
        .into_iter()
        .next()
        .map(|f| f.rust_source)
        .unwrap_or_default())
}

/// ファイルリストの共通ルートディレクトリを求める。
fn find_common_root(parsed: &ParsedFiles) -> std::path::PathBuf {
    if parsed.files.is_empty() {
        return std::path::PathBuf::new();
    }
    if parsed.files.len() == 1 {
        return parsed.files[0]
            .path
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .to_path_buf();
    }
    // 全ファイルの共通 prefix を求める
    let first = &parsed.files[0].path;
    let mut common = first
        .parent()
        .unwrap_or(std::path::Path::new(""))
        .to_path_buf();
    for file in &parsed.files[1..] {
        while !file.path.starts_with(&common) {
            common = match common.parent() {
                Some(p) => p.to_path_buf(),
                None => return std::path::PathBuf::new(),
            };
        }
    }
    common
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_files_single_valid_source() {
        let files = vec![(
            PathBuf::from("test.ts"),
            "interface Foo { name: string; }".to_string(),
        )];
        let parsed = parse_files(files).unwrap();
        assert_eq!(parsed.files.len(), 1);
        assert!(
            !parsed.files[0].module.body.is_empty(),
            "parsed module body should not be empty"
        );
    }

    #[test]
    fn test_parse_files_multiple_sources() {
        let files = vec![
            (
                PathBuf::from("a.ts"),
                "interface A { x: number; }".to_string(),
            ),
            (
                PathBuf::from("b.ts"),
                "interface B { y: string; }".to_string(),
            ),
            (
                PathBuf::from("c.ts"),
                "interface C { z: boolean; }".to_string(),
            ),
        ];
        let parsed = parse_files(files).unwrap();
        assert_eq!(parsed.files.len(), 3);
    }

    #[test]
    fn test_parse_files_parse_error_returns_err() {
        let files = vec![(PathBuf::from("bad.ts"), "function {{{".to_string())];
        let result = parse_files(files);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_files_preserves_path_and_source() {
        let source = "const x: number = 42;".to_string();
        let files = vec![(PathBuf::from("my/file.ts"), source.clone())];
        let parsed = parse_files(files).unwrap();
        assert_eq!(parsed.files[0].path, PathBuf::from("my/file.ts"));
        assert_eq!(parsed.files[0].source, source);
    }

    #[test]
    fn test_null_resolver_always_returns_none() {
        let resolver = NullModuleResolver;
        assert_eq!(
            resolver.resolve(std::path::Path::new("any/file.ts"), "./foo"),
            None
        );
        assert_eq!(
            resolver.resolve(std::path::Path::new("other.ts"), "../bar"),
            None
        );
        assert_eq!(
            resolver.resolve(std::path::Path::new("x.ts"), "lodash"),
            None
        );
    }

    #[test]
    fn test_pipeline_single_interface_produces_struct() {
        let input = TranspileInput {
            files: vec![(
                PathBuf::from("test.ts"),
                "interface Foo { name: string; }".to_string(),
            )],
            builtin_types: None,
            module_resolver: Box::new(NullModuleResolver),
        };
        let output = transpile_pipeline(input).unwrap();
        assert_eq!(output.files.len(), 1);
        assert!(
            output.files[0].rust_source.contains("struct Foo"),
            "output should contain struct Foo, got: {}",
            output.files[0].rust_source
        );
    }

    #[test]
    fn test_pipeline_multiple_files_produces_all_outputs() {
        let input = TranspileInput {
            files: vec![
                (
                    PathBuf::from("a.ts"),
                    "interface A { x: number; }".to_string(),
                ),
                (
                    PathBuf::from("b.ts"),
                    "interface B { y: string; }".to_string(),
                ),
                (
                    PathBuf::from("c.ts"),
                    "interface C { z: boolean; }".to_string(),
                ),
            ],
            builtin_types: None,
            module_resolver: Box::new(NullModuleResolver),
        };
        let output = transpile_pipeline(input).unwrap();
        assert_eq!(output.files.len(), 3);
        assert!(output.files[0].rust_source.contains("struct A"));
        assert!(output.files[1].rust_source.contains("struct B"));
        assert!(output.files[2].rust_source.contains("struct C"));
    }

    #[test]
    fn test_pipeline_unsupported_syntax_collected() {
        let input = TranspileInput {
            files: vec![(PathBuf::from("test.ts"), "export default 42;".to_string())],
            builtin_types: None,
            module_resolver: Box::new(NullModuleResolver),
        };
        let output = transpile_pipeline(input).unwrap();
        assert_eq!(output.files.len(), 1);
        assert!(
            !output.files[0].unsupported.is_empty(),
            "unsupported syntax should be collected"
        );
    }

    #[test]
    fn test_pipeline_output_path_has_rs_extension() {
        let input = TranspileInput {
            files: vec![(PathBuf::from("src/foo.ts"), "interface Foo {}".to_string())],
            builtin_types: None,
            module_resolver: Box::new(NullModuleResolver),
        };
        let output = transpile_pipeline(input).unwrap();
        assert_eq!(output.files[0].path, PathBuf::from("src/foo.rs"));
    }
}
