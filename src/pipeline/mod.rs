//! Unified transformation pipeline.
//!
//! This module provides the new multi-pass pipeline architecture:
//! Parse → ModuleGraph → TypeCollection → TypeResolution → Transform → Generate → Output.

pub mod module_graph;
pub mod module_resolver;
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

/// Unified transpilation pipeline (bridge implementation).
///
/// Currently delegates to the existing `transpile_collecting_with_registry` for each file.
/// This will be replaced with the full multi-pass pipeline in P2-P8.
///
/// # Errors
///
/// Returns an error if parsing or transformation fails.
pub fn transpile_pipeline(input: TranspileInput) -> Result<TranspileOutput> {
    let parsed = parse_files(input.files)?;
    let builtin_registry = input.builtin_types.unwrap_or_default();

    // Build shared registry from all files (existing Pass 1 logic)
    let source_strs: Vec<&str> = parsed.files.iter().map(|f| f.source.as_str()).collect();
    let mut shared_registry = crate::build_shared_registry(&source_strs);
    shared_registry.merge(&builtin_registry);

    let mut file_outputs = Vec::new();
    for file in &parsed.files {
        // Bridge: delegate to existing logic.
        // Note: current_file_dir is not computed here — directory mode import
        // resolution will be handled properly by ModuleGraph in P2.
        let (rust_source, unsupported) =
            crate::transpile_collecting_with_registry(&file.source, &shared_registry)?;

        file_outputs.push(FileOutput {
            path: file.path.with_extension("rs"),
            rust_source,
            unsupported,
        });
    }

    Ok(TranspileOutput {
        files: file_outputs,
        // module_graph and synthetic_types will be added in P8
        // when the full pipeline is assembled.
    })
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
