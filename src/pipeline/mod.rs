//! Unified transformation pipeline.
//!
//! This module provides the new multi-pass pipeline architecture:
//! Parse → ModuleGraph → TypeCollection → TypeResolution → Transform → Generate → Output.

pub mod any_enum_analyzer;
pub(crate) mod any_narrowing;
pub mod external_struct_generator;
pub mod module_graph;
pub mod module_resolver;
pub(crate) mod narrowing_patterns;
pub mod output_writer;
pub mod placement;
pub mod synthetic_registry;
pub mod type_converter;
pub mod type_resolution;
pub mod type_resolver;
mod types;

pub use module_graph::{ExportOrigin, ModuleGraph, ModuleGraphBuilder, ResolvedImport};
pub use synthetic_registry::{SyntheticTypeDef, SyntheticTypeKind, SyntheticTypeRegistry};
pub(crate) use types::PerFileTransformed;
pub use types::{
    FileOutput, ModuleResolver, NullModuleResolver, OutputFile, ParsedFile, ParsedFiles,
    ResolvedType, TranspileInput, TranspileOutput,
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

    // Any Enum Analysis (per-file: detect any-typed variables with typeof/instanceof narrowing,
    // register synthetic enum types, record overrides for TypeResolver)
    let mut synthetic = input.base_synthetic.unwrap_or_default();
    let mut per_file_any_synthetics: Vec<SyntheticTypeRegistry> =
        Vec::with_capacity(parsed.files.len());
    let mut per_file_any_overrides: Vec<_> = Vec::with_capacity(parsed.files.len());
    for file in &parsed.files {
        let mut resolution = type_resolution::FileTypeResolution::empty();
        let mut file_any_synthetic = SyntheticTypeRegistry::new();
        any_enum_analyzer::analyze_any_enums(
            &file.module,
            &mut resolution,
            &mut file_any_synthetic,
        );
        per_file_any_overrides.push(resolution.any_enum_overrides);
        // Register any-enum types in shared TypeRegistry so that transformer can look them up
        crate::registry::register_extra_enums(&mut shared_registry, &file_any_synthetic);
        per_file_any_synthetics.push(file_any_synthetic);
    }

    // Type Resolution (all files first, so SyntheticTypeRegistry becomes immutable before Transform).
    // TypeResolver receives any_enum_overrides so it registers the correct enum types
    // in expr_types from the start, eliminating the need for fallback lookups.
    // Each file gets its own per-file synthetic for anonymous struct generation; these are
    // merged into the shared synthetic after resolution.
    let mut type_resolutions = Vec::with_capacity(parsed.files.len());
    let mut per_file_resolver_synthetics = Vec::with_capacity(parsed.files.len());
    for (file, any_overrides) in parsed.files.iter().zip(per_file_any_overrides) {
        let mut file_resolver_synthetic = synthetic.fork_dedup_state();
        let type_resolution = {
            let mut resolver =
                type_resolver::TypeResolver::new(&shared_registry, &mut file_resolver_synthetic);
            resolver.set_any_enum_overrides(any_overrides);
            resolver.resolve_file(file)
        };
        type_resolutions.push(type_resolution);
        per_file_resolver_synthetics.push(file_resolver_synthetic);
    }

    // Register anonymous structs from TypeResolver in the shared TypeRegistry.
    // This lets the Transformer's resolve_field_type() look up field info for
    // anonymous struct types via the standard reg().get() path.
    for resolver_synthetic in &per_file_resolver_synthetics {
        register_synthetic_structs_in_registry(resolver_synthetic, &mut shared_registry);
    }

    // Pass 4: Transformation (per file) — 全ファイルを transform して synthetic を完全に
    // 蓄積してから Pass 5 (code generation) に進む。
    //
    // I-371: クロスファイル冗長定義（criterion 3）を解消するため、stub 生成は全ファイル
    // 横断の synthetic registry を見て行う必要がある。例えば file A が `AlgorithmOrString`
    // という合成型を生成し、file B がそれを参照する場合、file B 単体では未定義に見えるが、
    // 実際にはグローバルに存在する。Pass 4 を完了してから Pass 5 を回すことで、stub 生成
    // が「真に未定義の型」だけを対象にできる。
    let mut transformed: Vec<PerFileTransformed> = Vec::with_capacity(parsed.files.len());
    for (((file, type_resolution), any_synthetic), resolver_synthetic) in parsed
        .files
        .iter()
        .zip(type_resolutions.iter())
        .zip(per_file_any_synthetics)
        .zip(per_file_resolver_synthetics)
    {
        let tctx = crate::transformer::context::TransformContext::new(
            &module_graph,
            &shared_registry,
            type_resolution,
            &file.path,
        );
        // Start with any-enum types from analysis phase, merge TypeResolver synthetics,
        // then Transformer adds more
        let mut file_synthetic = any_synthetic;
        file_synthetic.merge(resolver_synthetic);
        let (items, unsupported) =
            crate::transformer::Transformer::for_module(&tctx, &mut file_synthetic)
                .transform_module_collecting(&file.module)?;

        // 合成型は OutputWriter が単一正準配置（I-371）を決定する。
        // 共有 synthetic にのみ蓄積する。Pass 5a で全ファイル横断の外部型解決を行う。
        synthetic.merge(file_synthetic);

        transformed.push(PerFileTransformed {
            path: file.path.clone(),
            source: file.source.clone(),
            items,
            unsupported,
        });
    }

    // Pass 5a: Global External Type Resolution (I-376).
    //
    // 全ファイルの transform が完了した時点で、user file items と synthetic registry の
    // 両方を横断 scan し、未定義外部型の推移閉包を計算して 1 度だけ生成する。生成された
    // struct は `synthetic` registry に `SyntheticTypeKind::External` として登録される。
    //
    // これにより:
    //  1. 外部型 struct は `synthetic` ただ 1 か所にのみ存在し、`file_outputs[i].items`
    //     には構造的に入らない。`file.items` と `synthetic_items` 間の重複が構造的に
    //     不可能になる。
    //  2. モノモーフィゼーションは global で 1 回しか走らないため、per-file 間の state
    //     差異による silent divergence が原理的に消滅する。
    //  3. downstream の `OutputWriter::resolve_synthetic_placement` が外部型を他の合成型
    //     と完全に同じルール (inline/shared) で uniform に扱える。
    resolve_external_types_globally(&transformed, &mut synthetic, &shared_registry);

    // Pass 5b: Per-file codegen.
    // `tf.items` は user IR のみを含む。外部型 struct は一切焼き込まれない。
    let mut file_outputs = Vec::new();
    for tf in transformed {
        let rust_source = crate::generator::generate(&tf.items);
        file_outputs.push(FileOutput {
            path: tf.path.with_extension("rs"),
            source: tf.source,
            rust_source,
            unsupported: tf.unsupported,
            items: tf.items,
        });
    }

    // Pass 5c: synthetic items 抽出 (I-382)。
    // 外部型は Phase 5a で解決済み。user 定義型への参照は OutputWriter が配置決定時に
    // `use crate::<module>::Type;` import を生成する。
    let synthetic_items: Vec<crate::ir::Item> =
        synthetic.all_items().into_iter().cloned().collect();

    Ok(TranspileOutput {
        files: file_outputs,
        module_graph,
        synthetic_items,
    })
}

/// Registers inline structs from a `SyntheticTypeRegistry` into a `TypeRegistry`.
///
/// Anonymous structs generated by TypeResolver need to be accessible to the Transformer
/// through the standard `TypeRegistry::get()` path. This ensures `resolve_field_type()`
/// works uniformly for both declared types and anonymous structs.
fn register_synthetic_structs_in_registry(
    synthetic: &SyntheticTypeRegistry,
    registry: &mut crate::registry::TypeRegistry,
) {
    for item in synthetic.all_items() {
        if let crate::ir::Item::Struct {
            name,
            fields,
            type_params,
            ..
        } = item
        {
            // Only register if not already in the registry (avoid overwriting declared types)
            if registry.get(name).is_none() {
                let field_defs: Vec<crate::registry::FieldDef> = fields
                    .iter()
                    .map(|f| crate::registry::FieldDef {
                        name: f.name.clone(),
                        ty: f.ty.clone(),
                        optional: false,
                    })
                    .collect();
                registry.register(
                    name.clone(),
                    crate::registry::TypeDef::Struct {
                        type_params: type_params.clone(),
                        fields: field_defs,
                        methods: std::collections::HashMap::new(),
                        constructor: None,
                        call_signatures: vec![],
                        extends: vec![],
                        is_interface: false,
                    },
                );
            }
        }
    }
}

/// I-376: 全ファイルの user IR と synthetic registry を横断 scan し、未定義外部型の
/// 推移閉包を固定点まで反復生成して `synthetic` registry に登録する。
///
/// 生成される struct は `SyntheticTypeKind::External` として登録される。downstream の
/// `OutputWriter::resolve_synthetic_placement` はこの variant を特別扱いせず、他の合成型
/// と同じ inline/shared 配置ルールで扱う。
///
/// # 推移依存の収束保証
///
/// 生成された外部型 struct のフィールドが新たな外部型を参照する場合、次の iteration で
/// 検出される。**各 iteration は検出された全 undefined 名について必ず 1 件以上 synthetic
/// に登録する** ため (`generate_external_struct` が `None` を返した場合も空 stub を push
/// して name を claim する)、収束は monotone increase で保証される。
/// [`external_struct_generator::UNDEFINED_REFS_FIXPOINT_MAX_ITERATIONS`] を安全網とし、
/// 超過は構造的バグとして **panic** する。
///
/// # Panics
///
/// 固定点が [`external_struct_generator::UNDEFINED_REFS_FIXPOINT_MAX_ITERATIONS`] 以内で
/// 収束しない場合。実データでは深さ 2〜3 で収束する想定。
fn resolve_external_types_globally(
    transformed: &[PerFileTransformed],
    synthetic: &mut SyntheticTypeRegistry,
    registry: &crate::registry::TypeRegistry,
) {
    use external_struct_generator::UNDEFINED_REFS_FIXPOINT_MAX_ITERATIONS as MAX_ITERATIONS;
    for iter in 0..=MAX_ITERATIONS {
        // Scan pool は 1 iteration 内でのみ生存する。ブロックスコープで借用を閉じて
        // から `synthetic` への可変借用に移る。
        let undefined: Vec<String> = {
            let synth_items = synthetic.all_items();
            let pool: Vec<&crate::ir::Item> = transformed
                .iter()
                .flat_map(|tf| tf.items.iter())
                .chain(synth_items)
                .collect();
            let mut names: Vec<String> =
                external_struct_generator::collect_undefined_type_references(&pool, registry)
                    .into_iter()
                    .collect();
            names.sort();
            names
        };

        if undefined.is_empty() {
            return;
        }
        assert!(
            iter < MAX_ITERATIONS,
            "resolve_external_types_globally: fixpoint did not converge in {MAX_ITERATIONS} \
             iterations (unresolved external types: {undefined:?}). This indicates a bug in \
             collect_undefined_type_references or SyntheticTypeRegistry::push_item idempotency."
        );

        for name in &undefined {
            // `generate_external_struct` は `TypeDef::Struct` のみフル生成し、`Function` /
            // `ConstValue` では `None` を返す。None の場合は空 stub を push して **必ず**
            // name を claim する。次 iteration の `defined_types` 集合に含まれることで
            // 無限ループを防ぎ、同時に生成コードが参照した時点で Rust コンパイラが
            // 型不整合を検出できる。
            let item =
                external_struct_generator::generate_external_struct(name, registry, synthetic)
                    .unwrap_or_else(|| crate::ir::Item::Struct {
                        vis: crate::ir::Visibility::Public,
                        name: name.clone(),
                        type_params: vec![],
                        fields: vec![],
                    });
            synthetic.push_item(
                name.clone(),
                crate::pipeline::SyntheticTypeKind::External,
                item,
            );
        }
    }
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
        base_synthetic: None,
        module_resolver: Box::new(crate::pipeline::module_resolver::TrivialResolver),
    };
    let output = transpile_pipeline(input)?;
    let TranspileOutput {
        files,
        synthetic_items,
        ..
    } = output;
    let Some(file) = files.into_iter().next() else {
        return Ok(String::new());
    };
    let prepended =
        placement::render_referenced_synthetics_for_file(&file.path, &file.items, &synthetic_items);
    if prepended.is_empty() {
        Ok(file.rust_source)
    } else if file.rust_source.is_empty() {
        Ok(prepended)
    } else {
        Ok(format!("{prepended}\n\n{}", file.rust_source))
    }
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

    // ===== I-376 Phase 5a helpers =====

    #[test]
    fn test_resolve_external_types_globally_handles_non_struct_external_typedef() {
        // `generate_external_struct` が `None` を返すケース (Function TypeDef 等) の
        // 空 stub フォールバック。Phase 5a が 1 iteration 内で必ず name を claim し、
        // 無限ループ → panic を防ぐ regression guard。
        use crate::ir::{Item, RustType, StructField, Visibility};
        use crate::pipeline::SyntheticTypeKind;
        use crate::registry::TypeDef;
        let mut registry = crate::registry::TypeRegistry::new();
        // Function TypeDef を外部型として登録。
        registry.register_external(
            "ExternalCallback".to_string(),
            TypeDef::Function {
                type_params: vec![],
                params: vec![],
                return_type: Some(RustType::Unit),
                has_rest: false,
            },
        );
        // ExternalCallback を field で参照する user struct を持つ transformed を構築。
        let transformed = vec![PerFileTransformed {
            path: PathBuf::from("a.ts"),
            source: String::new(),
            items: vec![Item::Struct {
                vis: Visibility::Public,
                name: "Holder".to_string(),
                type_params: vec![],
                fields: vec![StructField {
                    vis: Some(Visibility::Public),
                    name: "cb".to_string(),
                    ty: RustType::Named {
                        name: "ExternalCallback".to_string(),
                        type_args: vec![],
                    },
                }],
            }],
            unsupported: vec![],
        }];
        let mut synthetic = SyntheticTypeRegistry::new();
        // 本関数は panic せずに完了するはず。
        resolve_external_types_globally(&transformed, &mut synthetic, &registry);
        // ExternalCallback が空 stub として登録されていることを確認。
        let entry = synthetic
            .get("ExternalCallback")
            .expect("ExternalCallback should be claimed as a fallback stub");
        assert!(matches!(entry.kind, SyntheticTypeKind::External));
        assert!(
            matches!(&entry.item, Item::Struct { fields, .. } if fields.is_empty()),
            "fallback stub should be an empty struct"
        );
    }

    #[test]
    fn test_resolve_external_types_globally_transitive_closure() {
        // External A -> External B の推移依存が 2 iteration 以内で収束し、両方が
        // synthetic に登録されることを unit level で検証。
        use crate::ir::{Item, RustType, StructField, Visibility};
        use crate::registry::{FieldDef, TypeDef};
        let mut registry = crate::registry::TypeRegistry::new();
        registry.register_external(
            "Outer".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![FieldDef {
                    name: "inner".to_string(),
                    ty: RustType::Named {
                        name: "Inner".to_string(),
                        type_args: vec![],
                    },
                    optional: false,
                }],
                methods: std::collections::HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        registry.register_external(
            "Inner".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![FieldDef {
                    name: "value".to_string(),
                    ty: RustType::F64,
                    optional: false,
                }],
                methods: std::collections::HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let transformed = vec![PerFileTransformed {
            path: PathBuf::from("a.ts"),
            source: String::new(),
            items: vec![Item::Struct {
                vis: Visibility::Public,
                name: "User".to_string(),
                type_params: vec![],
                fields: vec![StructField {
                    vis: Some(Visibility::Public),
                    name: "o".to_string(),
                    ty: RustType::Named {
                        name: "Outer".to_string(),
                        type_args: vec![],
                    },
                }],
            }],
            unsupported: vec![],
        }];
        let mut synthetic = SyntheticTypeRegistry::new();
        resolve_external_types_globally(&transformed, &mut synthetic, &registry);
        assert!(
            synthetic.get("Outer").is_some(),
            "Outer should be resolved on iteration 1"
        );
        assert!(
            synthetic.get("Inner").is_some(),
            "Inner should be resolved transitively on iteration 2"
        );
    }

    #[test]
    fn test_resolve_external_types_globally_noop_when_no_externals_referenced() {
        // 外部型参照なし → synthetic に 1 件も追加されず、即座に return。
        use crate::ir::{Item, RustType, StructField, Visibility};
        let registry = crate::registry::TypeRegistry::new();
        let transformed = vec![PerFileTransformed {
            path: PathBuf::from("a.ts"),
            source: String::new(),
            items: vec![Item::Struct {
                vis: Visibility::Public,
                name: "PureUser".to_string(),
                type_params: vec![],
                fields: vec![StructField {
                    vis: Some(Visibility::Public),
                    name: "n".to_string(),
                    ty: RustType::F64,
                }],
            }],
            unsupported: vec![],
        }];
        let mut synthetic = SyntheticTypeRegistry::new();
        resolve_external_types_globally(&transformed, &mut synthetic, &registry);
        assert_eq!(
            synthetic.all_items().len(),
            0,
            "no externals referenced → synthetic stays empty"
        );
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
            base_synthetic: None,
            module_resolver: Box::new(crate::pipeline::module_resolver::TrivialResolver),
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
            base_synthetic: None,
            module_resolver: Box::new(crate::pipeline::module_resolver::TrivialResolver),
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
            base_synthetic: None,
            module_resolver: Box::new(crate::pipeline::module_resolver::TrivialResolver),
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
            base_synthetic: None,
            module_resolver: Box::new(crate::pipeline::module_resolver::TrivialResolver),
        };
        let output = transpile_pipeline(input).unwrap();
        assert_eq!(output.files[0].path, PathBuf::from("src/foo.rs"));
    }

    // ===== find_common_root tests =====

    #[test]
    fn test_find_common_root_empty_files() {
        let parsed = ParsedFiles { files: vec![] };
        assert_eq!(find_common_root(&parsed), PathBuf::new());
    }

    #[test]
    fn test_find_common_root_single_file() {
        let parsed = parse_files(vec![(
            PathBuf::from("src/foo.ts"),
            "const x = 1;".to_string(),
        )])
        .unwrap();
        assert_eq!(find_common_root(&parsed), PathBuf::from("src"));
    }

    #[test]
    fn test_find_common_root_single_file_no_parent() {
        let parsed =
            parse_files(vec![(PathBuf::from("foo.ts"), "const x = 1;".to_string())]).unwrap();
        assert_eq!(find_common_root(&parsed), PathBuf::from(""));
    }

    #[test]
    fn test_find_common_root_same_directory() {
        let parsed = parse_files(vec![
            (PathBuf::from("src/a.ts"), "const a = 1;".to_string()),
            (PathBuf::from("src/b.ts"), "const b = 2;".to_string()),
        ])
        .unwrap();
        assert_eq!(find_common_root(&parsed), PathBuf::from("src"));
    }

    #[test]
    fn test_find_common_root_nested_directories() {
        let parsed = parse_files(vec![
            (PathBuf::from("src/a/x.ts"), "const x = 1;".to_string()),
            (PathBuf::from("src/b/y.ts"), "const y = 2;".to_string()),
        ])
        .unwrap();
        assert_eq!(find_common_root(&parsed), PathBuf::from("src"));
    }

    #[test]
    fn test_find_common_root_deeply_nested() {
        let parsed = parse_files(vec![
            (
                PathBuf::from("project/src/a/x.ts"),
                "const x = 1;".to_string(),
            ),
            (
                PathBuf::from("project/src/b/c/y.ts"),
                "const y = 2;".to_string(),
            ),
        ])
        .unwrap();
        assert_eq!(find_common_root(&parsed), PathBuf::from("project/src"));
    }

    // ===== Absolute path pipeline integration tests =====

    #[test]
    fn test_pipeline_absolute_paths_export_all_no_path_leak() {
        // Regression test: absolute file paths must not leak into generated `use` paths.
        // Previously, `export * from './types'` with absolute file paths generated
        // `pub use crate::/tmp/project/helper/conninfo::types::*` instead of
        // `pub use crate::types::*`.
        use crate::pipeline::module_resolver::NodeModuleResolver;

        // In production, known_files are absolute paths from collect_ts_files()
        let root = PathBuf::from("/tmp/project");
        let known: std::collections::HashSet<PathBuf> = [
            "/tmp/project/adapter/server.ts",
            "/tmp/project/adapter/types.ts",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let resolver = NodeModuleResolver::new(root, known);

        let input = TranspileInput {
            files: vec![
                (
                    PathBuf::from("/tmp/project/adapter/server.ts"),
                    "export * from './types';".to_string(),
                ),
                (
                    PathBuf::from("/tmp/project/adapter/types.ts"),
                    "export interface ConnInfo { address: string; }".to_string(),
                ),
            ],
            builtin_types: None,
            base_synthetic: None,
            module_resolver: Box::new(resolver),
        };
        let output = transpile_pipeline(input).unwrap();

        // Find the server.ts output (re-export file)
        let server_output = output
            .files
            .iter()
            .find(|f| f.path.to_str().unwrap().contains("server"))
            .expect("should have server output");

        // Must contain correct crate path, not absolute filesystem path
        assert!(
            server_output.rust_source.contains("crate::"),
            "output should contain crate:: path: {}",
            server_output.rust_source
        );
        assert!(
            !server_output.rust_source.contains("/tmp"),
            "output must not contain absolute path: {}",
            server_output.rust_source
        );
    }

    #[test]
    fn test_pipeline_absolute_paths_import() {
        use crate::pipeline::module_resolver::NodeModuleResolver;

        let root = PathBuf::from("/tmp/project");
        let known: std::collections::HashSet<PathBuf> =
            ["/tmp/project/adapter/server.ts", "/tmp/project/types.ts"]
                .iter()
                .map(PathBuf::from)
                .collect();
        let resolver = NodeModuleResolver::new(root, known);

        let input = TranspileInput {
            files: vec![
                (
                    PathBuf::from("/tmp/project/adapter/server.ts"),
                    "import { Config } from '../types';\nexport const port: number = 8080;"
                        .to_string(),
                ),
                (
                    PathBuf::from("/tmp/project/types.ts"),
                    "export interface Config { port: number; }".to_string(),
                ),
            ],
            builtin_types: None,
            base_synthetic: None,
            module_resolver: Box::new(resolver),
        };
        let output = transpile_pipeline(input).unwrap();

        let server_output = output
            .files
            .iter()
            .find(|f| f.path.to_str().unwrap().contains("server"))
            .expect("should have server output");

        assert!(
            !server_output.rust_source.contains("/tmp"),
            "output must not contain absolute path: {}",
            server_output.rust_source
        );
    }

    #[test]
    fn test_find_common_root_absolute_paths() {
        let parsed = parse_files(vec![
            (
                PathBuf::from("/tmp/project/adapter/server.ts"),
                "const x = 1;".to_string(),
            ),
            (
                PathBuf::from("/tmp/project/types.ts"),
                "const y = 2;".to_string(),
            ),
        ])
        .unwrap();
        assert_eq!(find_common_root(&parsed), PathBuf::from("/tmp/project"));
    }

    #[test]
    fn test_pipeline_any_enum_registered_in_type_registry() {
        // Verify that any-narrowing enums generated by pipeline's any_enum_analyzer
        // are properly registered in the shared TypeRegistry (G22).
        let input = TranspileInput {
            files: vec![(
                PathBuf::from("test.ts"),
                r#"
function process(data: any): string {
    if (typeof data === "string") {
        return data;
    }
    return "";
}
"#
                .to_string(),
            )],
            builtin_types: None,
            base_synthetic: None,
            module_resolver: Box::new(crate::pipeline::module_resolver::TrivialResolver),
        };
        let output = transpile_pipeline(input).unwrap();
        // The generated Rust should contain the any-narrowing enum
        assert!(
            output.files[0].rust_source.contains("ProcessDataType"),
            "output should contain any-narrowing enum: {}",
            output.files[0].rust_source
        );
    }

    #[test]
    fn test_pipeline_unknown_param_typeof_generates_enum() {
        // I-333: unknown-typed parameters should get synthetic enum like any-typed ones
        let input = TranspileInput {
            files: vec![(
                PathBuf::from("test.ts"),
                r#"
function process(data: unknown): string {
    if (typeof data === "string") {
        return data.toUpperCase();
    }
    return "";
}
"#
                .to_string(),
            )],
            builtin_types: None,
            base_synthetic: None,
            module_resolver: Box::new(crate::pipeline::module_resolver::TrivialResolver),
        };
        let output = transpile_pipeline(input).unwrap();
        // Should NOT contain "if false" — should have proper enum narrowing
        assert!(
            !output.files[0].rust_source.contains("if false"),
            "unknown typeof should not produce 'if false': {}",
            output.files[0].rust_source
        );
        // Should contain a synthetic enum (ProcessDataType or similar)
        assert!(
            output.files[0].rust_source.contains("if let"),
            "unknown typeof should produce if-let pattern: {}",
            output.files[0].rust_source
        );
    }
}
