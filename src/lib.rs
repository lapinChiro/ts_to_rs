//! ts_to_rs library: TypeScript → Rust transpiler.
//!
//! This library provides the core transformation pipeline:
//! TypeScript source → SWC AST → IR → Rust source.

pub mod directory;
pub mod generator;
pub mod ir;
pub mod parser;
pub mod registry;
pub mod transformer;

use anyhow::Result;
use serde::Serialize;

use crate::registry::{build_registry, TypeRegistry};
use crate::transformer::UnsupportedSyntaxError;

/// A report entry for an unsupported TypeScript syntax encountered during transformation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UnsupportedSyntax {
    /// The SWC AST node kind (e.g., `"ExportDefaultExpr"`, `"TsModuleDecl"`)
    pub kind: String,
    /// Source location as `"line:col"` (1-based)
    pub location: String,
}

/// Build a shared [`TypeRegistry`] from multiple TypeScript sources.
///
/// Each source is parsed independently; sources that fail to parse are silently skipped.
/// The resulting registry contains type information from all successfully parsed sources.
pub fn build_shared_registry(sources: &[&str]) -> TypeRegistry {
    let mut shared = TypeRegistry::new();
    for source in sources {
        if let Ok(module) = parser::parse_typescript(source) {
            let reg = build_registry(&module);
            shared.merge(&reg);
        }
    }
    shared
}

/// Transpiles TypeScript source code to Rust source code.
///
/// Chains the full pipeline: parse → build registry → transform → generate.
///
/// # Errors
///
/// Returns an error if parsing or transformation fails.
pub fn transpile(ts_source: &str) -> Result<String> {
    transformer::types::reset_synthetic_counter();
    let module = parser::parse_typescript(ts_source)?;
    let reg = build_registry(&module);
    let items = transformer::transform_module(&module, &reg)?;
    Ok(generator::generate(&items))
}

/// Transpiles TypeScript source code to Rust source code using a pre-built [`TypeRegistry`].
///
/// Used in directory mode where the registry is constructed from multiple files.
///
/// # Errors
///
/// Returns an error if parsing or transformation fails.
pub fn transpile_with_registry(ts_source: &str, registry: &TypeRegistry) -> Result<String> {
    transformer::types::reset_synthetic_counter();
    let module = parser::parse_typescript(ts_source)?;
    let mut reg = build_registry(&module);
    reg.merge(registry);
    let items = transformer::transform_module(&module, &reg)?;
    Ok(generator::generate(&items))
}

/// Transpiles TypeScript source code, collecting unsupported syntax instead of aborting.
///
/// Returns the Rust source for the supported portions and a list of unsupported syntax entries.
///
/// # Errors
///
/// Returns an error if parsing fails. Unsupported syntax errors are collected, not propagated.
pub fn transpile_collecting(ts_source: &str) -> Result<(String, Vec<UnsupportedSyntax>)> {
    transpile_collecting_with_registry(ts_source, &TypeRegistry::new())
}

/// Like [`transpile_collecting`] but with a pre-built [`TypeRegistry`].
///
/// # Errors
///
/// Returns an error if parsing fails. Unsupported syntax errors are collected, not propagated.
pub fn transpile_collecting_with_registry(
    ts_source: &str,
    registry: &TypeRegistry,
) -> Result<(String, Vec<UnsupportedSyntax>)> {
    transformer::types::reset_synthetic_counter();
    let module = parser::parse_typescript(ts_source)?;
    let mut reg = build_registry(&module);
    reg.merge(registry);
    let (items, raw_unsupported) = transformer::transform_module_collecting(&module, &reg)?;
    let output = generator::generate(&items);
    let unsupported = raw_unsupported
        .into_iter()
        .map(|raw| resolve_unsupported(ts_source, raw))
        .collect();
    Ok((output, unsupported))
}

/// Resolves an [`UnsupportedSyntaxError`] into an [`UnsupportedSyntax`] with line/col info.
fn resolve_unsupported(source: &str, raw: UnsupportedSyntaxError) -> UnsupportedSyntax {
    let (line, col) = byte_pos_to_line_col(source, raw.byte_pos);
    UnsupportedSyntax {
        kind: raw.kind,
        location: format!("{line}:{col}"),
    }
}

/// Converts a SWC `BytePos` (1-based byte position) to a 1-based (line, col) pair.
fn byte_pos_to_line_col(source: &str, byte_pos: u32) -> (usize, usize) {
    if byte_pos == 0 {
        return (1, 1);
    }
    let offset = (byte_pos as usize).saturating_sub(1);
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.bytes().enumerate() {
        if i >= offset {
            break;
        }
        if ch == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byte_pos_to_line_col_first_byte() {
        assert_eq!(byte_pos_to_line_col("hello", 1), (1, 1));
    }

    #[test]
    fn test_byte_pos_to_line_col_second_line() {
        // "abc\ndef" — 'd' is at byte 4 (0-based), BytePos = 5
        assert_eq!(byte_pos_to_line_col("abc\ndef", 5), (2, 1));
    }

    #[test]
    fn test_byte_pos_to_line_col_mid_line() {
        // "abc\ndef" — 'e' is at byte 5 (0-based), BytePos = 6
        assert_eq!(byte_pos_to_line_col("abc\ndef", 6), (2, 2));
    }

    #[test]
    fn test_byte_pos_to_line_col_zero_returns_1_1() {
        assert_eq!(byte_pos_to_line_col("hello", 0), (1, 1));
    }

    #[test]
    fn test_build_shared_registry_single_source_registers_type() {
        let sources = vec!["interface Foo { x: string; }"];
        let reg = build_shared_registry(&sources);
        assert!(reg.get("Foo").is_some(), "Foo should be in the registry");
    }

    #[test]
    fn test_build_shared_registry_multiple_sources_cross_reference() {
        let sources = vec![
            "interface Foo { x: string; }",
            "interface Bar { y: number; }",
        ];
        let reg = build_shared_registry(&sources);
        assert!(reg.get("Foo").is_some(), "Foo should be in the registry");
        assert!(reg.get("Bar").is_some(), "Bar should be in the registry");
    }

    #[test]
    fn test_build_shared_registry_invalid_source_skipped() {
        let sources = vec!["{{{invalid", "interface Valid { x: string; }"];
        let reg = build_shared_registry(&sources);
        assert!(
            reg.get("Valid").is_some(),
            "Valid should be in the registry despite invalid source"
        );
    }

    #[test]
    fn test_build_shared_registry_empty_sources_returns_empty() {
        let sources: Vec<&str> = vec![];
        let reg = build_shared_registry(&sources);
        assert!(
            reg.get("Foo").is_none(),
            "empty registry should not contain any types"
        );
    }

    #[test]
    fn test_transpile_collecting_all_supported_returns_empty_unsupported() {
        let source = "interface Foo { name: string; }";
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(!output.is_empty());
        assert!(unsupported.is_empty());
    }

    #[test]
    fn test_transpile_collecting_with_unsupported_collects_items() {
        let source = "export default 42;";
        let (_output, unsupported) = transpile_collecting(source).unwrap();
        assert_eq!(unsupported.len(), 1);
        assert_eq!(unsupported[0].kind, "ExportDefaultExpr");
        assert_eq!(unsupported[0].location, "1:1");
    }

    #[test]
    fn test_transpile_collecting_still_generates_supported_output() {
        let source = r#"
interface Foo { name: string; }
export default 42;
"#;
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(output.contains("struct Foo"));
        assert_eq!(unsupported.len(), 1);
    }

    #[test]
    fn test_transpile_default_mode_errors_on_unsupported() {
        let source = "export default 42;";
        let result = transpile(source);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unsupported syntax"));
    }

    #[test]
    fn test_transpile_collecting_transformer_internal_error_collected() {
        // Unsupported default value (new Map()) triggers an error inside convert_param,
        // which is a transformer-internal error (not UnsupportedSyntaxError).
        let source = "function foo(x: Map = new Map()) { return x; }";
        let result = transpile_collecting(source);
        assert!(result.is_ok(), "should not be a fatal error: {result:?}");
        let (_output, unsupported) = result.unwrap();
        assert!(
            !unsupported.is_empty(),
            "should report the function as unsupported"
        );
    }

    #[test]
    fn test_transpile_collecting_mixed_supported_and_internal_error() {
        // interface is convertible, function with unsupported default value is not
        let source = r#"
interface Foo { name: string; }
function bar(x: Map = new Map()) { return x; }
"#;
        let result = transpile_collecting(source);
        assert!(result.is_ok(), "should not be a fatal error: {result:?}");
        let (output, unsupported) = result.unwrap();
        assert!(
            output.contains("struct Foo"),
            "convertible items should still appear in output"
        );
        assert!(
            !unsupported.is_empty(),
            "unconvertible function should be in unsupported list"
        );
    }

    #[test]
    fn test_transpile_collecting_default_param_converts_successfully() {
        // Default parameter with literal value should now convert successfully
        let source = "function foo(x: number = 0) { return x; }";
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn foo"),
            "function with default param should be converted, got: {output}"
        );
        assert!(
            unsupported.is_empty(),
            "literal default param should not be unsupported"
        );
    }

    #[test]
    fn test_transpile_collecting_non_nullable_union_param_generates_enum() {
        // Non-nullable union type generates an enum and references it
        let source = "export function foo(x: string | number): void { }";
        let (output, _unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn foo"),
            "function should be converted, got: {output}"
        );
        assert!(
            output.contains("x: StringOrF64"),
            "non-nullable union param should reference generated enum, got: {output}"
        );
        assert!(
            output.contains("enum StringOrF64"),
            "generated enum should be in output, got: {output}"
        );
    }

    #[test]
    fn test_transpile_collecting_mixed_param_types_union_generates_enum() {
        // Supported param type + non-nullable union param type in the same function
        let source = "export function bar(a: string, b: string | number): void { }";
        let (output, _unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn bar"),
            "function should be converted, got: {output}"
        );
        assert!(
            output.contains("a: String"),
            "supported param should have normal type, got: {output}"
        );
        assert!(
            output.contains("b: StringOrF64"),
            "non-nullable union param should reference generated enum, got: {output}"
        );
    }

    #[test]
    fn test_transpile_collecting_non_nullable_union_return_type_generates_enum() {
        // Non-nullable union return type generates an enum
        let source = "export function baz(x: number): string | number { return x; }";
        let (output, _unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn baz"),
            "function should be converted, got: {output}"
        );
        assert!(
            output.contains("-> StringOrF64"),
            "non-nullable union return type should reference generated enum, got: {output}"
        );
    }

    #[test]
    fn test_transpile_collecting_non_nullable_union_no_unsupported_report() {
        // Non-nullable union is now supported (fallback to first type), so it
        // should not appear in the unsupported report
        let source = "export function foo(x: string | number): void { }";
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(output.contains("fn foo"), "function should be converted");
        assert!(
            unsupported.is_empty(),
            "no unsupported items should be reported, got: {unsupported:?}"
        );
    }

    #[test]
    fn test_transpile_default_non_nullable_union_param_succeeds() {
        // Non-nullable union is now supported, so strict mode should succeed
        let source = "function foo(x: string | number): void { }";
        let result = transpile(source);
        assert!(
            result.is_ok(),
            "non-nullable union param should succeed, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_transpile_collecting_parse_error_still_returns_err() {
        // Invalid TypeScript syntax should still be a fatal error
        let source = "function {{{";
        let result = transpile_collecting(source);
        assert!(result.is_err(), "parse errors should still propagate");
    }

    #[test]
    fn test_transpile_arrow_untyped_param_errors_in_default_mode() {
        // Arrow function with untyped param should error in strict mode
        let source = "export const f = (c) => c;";
        let result = transpile(source);
        assert!(
            result.is_err(),
            "untyped arrow param should error in strict mode"
        );
    }

    #[test]
    fn test_transpile_collecting_arrow_untyped_param_fallback_to_any() {
        // Arrow function with untyped param should fallback to Any in collecting mode
        let source = "export const f = (c) => c;";
        let (output, unsupported) = transpile_collecting(source).unwrap();
        assert!(
            output.contains("fn f"),
            "function should still be converted, got: {output}"
        );
        assert!(
            output.contains("serde_json::Value"),
            "untyped param should fallback to serde_json::Value, got: {output}"
        );
        assert!(
            !unsupported.is_empty(),
            "untyped param should be reported as unsupported"
        );
    }
}
