//! TypeScript source code parser using SWC.
//!
//! Parses TypeScript source strings into SWC AST [`Module`] nodes.

use anyhow::{anyhow, Result};
use swc_common::{input::StringInput, FileName, SourceMap, Span, Spanned};
use swc_ecma_ast::Module;
use swc_ecma_parser::{Parser, Syntax, TsSyntax};

/// Parses a TypeScript source string into an SWC AST [`Module`].
///
/// # Errors
///
/// Returns an error if the source contains syntax errors.
pub fn parse_typescript(source: &str) -> Result<Module> {
    let cm: std::sync::Arc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("input.ts".into()).into(),
        source.to_string(),
    );

    let lexer = swc_ecma_parser::lexer::Lexer::new(
        Syntax::Typescript(TsSyntax::default()),
        Default::default(),
        StringInput::from(&*fm),
        None,
    );

    let mut parser = Parser::new_from(lexer);
    let module = parser.parse_typescript_module().map_err(|e| {
        let span: Span = e.span();
        anyhow!("Parse error at {:?}: {:?}", span, e.into_kind())
    })?;

    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_typescript_empty_source() {
        let module = parse_typescript("").expect("should parse empty source");
        assert!(module.body.is_empty());
    }

    #[test]
    fn test_parse_typescript_interface() {
        let source = r#"interface Foo { name: string; age: number; }"#;
        let module = parse_typescript(source).expect("should parse interface");
        assert_eq!(module.body.len(), 1);
    }

    #[test]
    fn test_parse_typescript_syntax_error() {
        let source = r#"interface { }"#;
        let result = parse_typescript(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_typescript_type_alias() {
        let source = r#"type Name = string;"#;
        let module = parse_typescript(source).expect("should parse type alias");
        assert_eq!(module.body.len(), 1);
    }
}
