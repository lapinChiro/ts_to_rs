use super::placement::choose_shared_module_path;
use super::*;
use crate::ir::{Item, Param, RustType, Visibility};
use crate::parser::parse_typescript;
use crate::pipeline::module_graph::ModuleGraphBuilder;
use crate::pipeline::{NullModuleResolver, ParsedFiles};
use std::path::PathBuf;

/// テスト用ファイル仕様。所有データを保持し、`OutputFile<'_>` の借用元として使う。
struct TestFile {
    rel_path: PathBuf,
    source: String,
    items: Vec<Item>,
}

impl TestFile {
    fn new(path: &str, source: &str, items: Vec<Item>) -> Self {
        Self {
            rel_path: PathBuf::from(path),
            source: source.to_string(),
            items,
        }
    }
}

/// `TestFile` のスライスから `OutputFile<'_>` のベクタを構築する。
fn outputs_from(files: &[TestFile]) -> Vec<OutputFile<'_>> {
    files
        .iter()
        .map(|f| OutputFile {
            rel_path: f.rel_path.clone(),
            source: &f.source,
            items: &f.items,
        })
        .collect()
}

/// 指定型を return type に持つ関数 Item を作る（user file 内で synthetic を参照する用）。
fn fn_returning(fn_name: &str, ref_type: &str) -> Item {
    Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: fn_name.to_string(),
        type_params: vec![],
        params: vec![],
        return_type: Some(RustType::Named {
            name: ref_type.to_string(),
            type_args: vec![],
        }),
        body: vec![],
    }
}

/// 単一引数の型に指定した型を持つ関数 Item を作る。
/// （Phase F の追加テストで利用予定。現時点では未使用だが API 完備性のため残す。）
#[allow(dead_code)]
fn fn_with_param_type(fn_name: &str, param_type: &str) -> Item {
    Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: fn_name.to_string(),
        type_params: vec![],
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::Named {
                name: param_type.to_string(),
                type_args: vec![],
            }),
        }],
        return_type: None,
        body: vec![],
    }
}

/// テスト用 ModuleGraph を構築する（NodeModuleResolver 使用、re-export 解決付き）。
fn build_module_graph_with_resolver(root: &Path, filenames: &[&str]) -> ModuleGraph {
    use crate::pipeline::module_resolver::NodeModuleResolver;

    let parsed_files: Vec<_> = filenames
        .iter()
        .map(|name| {
            let path = root.join(name);
            let source = std::fs::read_to_string(&path).unwrap();
            crate::pipeline::ParsedFile {
                path,
                source: source.clone(),
                module: parse_typescript(&source).unwrap(),
            }
        })
        .collect();
    let parsed = ParsedFiles {
        files: parsed_files,
    };
    let known: std::collections::HashSet<PathBuf> =
        filenames.iter().map(|n| root.join(n)).collect();
    let resolver = NodeModuleResolver::new(root.to_path_buf(), known);
    ModuleGraphBuilder::new(&parsed, &resolver, root).build()
}

/// テスト用 ModuleGraph を構築する（NullModuleResolver 使用、re-export 解決なし）。
fn build_module_graph(files: &[(PathBuf, &str)]) -> ModuleGraph {
    let parsed_files: Vec<_> = files
        .iter()
        .map(|(path, source)| crate::pipeline::ParsedFile {
            path: path.clone(),
            source: source.to_string(),
            module: parse_typescript(source).unwrap(),
        })
        .collect();
    let parsed = ParsedFiles {
        files: parsed_files,
    };
    let resolver = NullModuleResolver;
    let root = PathBuf::from("src");
    ModuleGraphBuilder::new(&parsed, &resolver, &root).build()
}

// ===== generate_mod_rs tests =====

#[test]
fn test_generate_mod_rs_children_modules() {
    let mg = build_module_graph(&[
        (PathBuf::from("src/a.ts"), "export function fa(): void {}"),
        (PathBuf::from("src/b.ts"), "export function fb(): void {}"),
    ]);
    let writer = OutputWriter::new(&mg);
    let result = writer.generate_mod_rs(Path::new("src"));
    assert!(
        result.contains("pub mod a;"),
        "should contain pub mod a: {result}"
    );
    assert!(
        result.contains("pub mod b;"),
        "should contain pub mod b: {result}"
    );
}

#[test]
fn test_generate_mod_rs_empty_dir() {
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let result = writer.generate_mod_rs(Path::new("src"));
    assert!(
        result.is_empty(),
        "empty dir should produce empty string: {result}"
    );
}

#[test]
fn test_generate_mod_rs_reexports() {
    // Integration test: re-export を含む ModuleGraph から pub use が生成されることを確認。
    // E2E テスト（integration_test.rs）でも同等の検証がされるため、
    // ここでは ModuleGraph の re-export 解決が動作する前提でのフォーマットを確認。
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    std::fs::write(src.join("foo.ts"), "export function hello(): void {}").unwrap();
    std::fs::write(src.join("index.ts"), r#"export { hello } from "./foo";"#).unwrap();

    let mg = build_module_graph_with_resolver(src, &["foo.ts", "index.ts"]);
    let writer = OutputWriter::new(&mg);
    let result = writer.generate_mod_rs(src);
    // children_of が foo を検出するはず
    assert!(
        result.contains("pub mod foo;"),
        "should contain pub mod foo: {result}"
    );
    // re-export の pub use は ModuleGraph の re-export 解決に依存する。
    // NullModuleResolver 相当のケースでは pub use が生成されない可能性がある。
    // この検証は E2E テスト（Hono ベンチマーク）で網羅する。
}

#[test]
fn test_generate_mod_rs_mixed() {
    let mg = build_module_graph(&[
        (PathBuf::from("src/a.ts"), "export function fa(): void {}"),
        (PathBuf::from("src/b.ts"), "export function fb(): void {}"),
    ]);
    let writer = OutputWriter::new(&mg);
    let result = writer.generate_mod_rs(Path::new("src"));
    assert!(
        result.contains("pub mod a;"),
        "should contain pub mod a: {result}"
    );
    assert!(
        result.contains("pub mod b;"),
        "should contain pub mod b: {result}"
    );
}

// ===== resolve_synthetic_placement tests =====

fn make_synthetic_enum(name: &str) -> Item {
    Item::Enum {
        vis: Visibility::Public,
        name: name.to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![],
    }
}

#[test]
fn test_resolve_synthetic_placement_single_file() {
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let files = vec![TestFile::new(
        "a.rs",
        "fn foo() -> StringOrF64 { todo!() }",
        vec![fn_returning("foo", "StringOrF64")],
    )];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("StringOrF64")];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);
    assert!(
        placement.inline.contains_key(Path::new("a.rs")),
        "should be inline in a.rs"
    );
    assert!(
        placement.shared_module.is_none(),
        "should not have shared module"
    );
}

#[test]
fn test_resolve_synthetic_placement_multi_file() {
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let files = vec![
        TestFile::new(
            "a.rs",
            "fn foo() -> StringOrF64 { todo!() }",
            vec![fn_returning("foo", "StringOrF64")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> StringOrF64 { todo!() }",
            vec![fn_returning("bar", "StringOrF64")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("StringOrF64")];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);
    assert!(placement.inline.is_empty(), "should not be inline");
    assert!(
        placement.shared_module.is_some(),
        "should have shared module"
    );
}

#[test]
fn test_resolve_synthetic_placement_unused() {
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    // user file は何も synthetic を参照しない（items 空）
    let files = vec![TestFile::new("a.rs", "fn foo() {}", vec![])];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("UnusedEnum")];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);
    assert!(placement.inline.is_empty(), "should not be inline");
    assert!(
        placement.shared_module.is_none(),
        "should not have shared module"
    );
}

#[test]
fn test_resolve_synthetic_placement_skips_unnamed_items() {
    // canonical_name() == None の Item（Use/Comment/RawCode）が synthetic_items に
    // 混入した場合、それらは配置対象外として skip される。
    // 旧実装の `unwrap_or("").to_string()` では空文字列で contains() が全マッチし
    // shared_module に誤配置されていた。本テストは regression 防止用。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let files = vec![TestFile::new(
        "a.rs",
        "fn foo() -> Real { todo!() }",
        vec![fn_returning("foo", "Real")],
    )];
    let outputs = outputs_from(&files);
    let items = vec![
        Item::Comment("a comment".to_string()),
        Item::Use {
            vis: crate::ir::Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["Bar".to_string()],
        },
        Item::RawCode("fn raw() {}".to_string()),
        make_synthetic_enum("Real"),
    ];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);
    // Real は inline 配置される
    assert!(
        placement.inline.contains_key(Path::new("a.rs")),
        "Real は inline 配置される"
    );
    // shared_module は生成されない（unnamed items が誤って入らない）
    assert!(
        placement.shared_module.is_none(),
        "unnamed items が誤って shared_module に配置されないこと: {:?}",
        placement.shared_module
    );
    // shared_imports も空
    assert!(placement.shared_imports.is_empty());
}

// ===== write_to_directory tests =====

#[test]
fn test_write_to_directory_creates_files() {
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path();

    let files = vec![
        TestFile::new("a.rs", "fn a() {}", vec![]),
        TestFile::new("sub/b.rs", "fn b() {}", vec![]),
    ];
    let outputs = outputs_from(&files);

    writer
        .write_to_directory(out, &outputs, &[], false)
        .unwrap();

    assert!(out.join("a.rs").exists(), "a.rs should exist");
    assert!(out.join("sub/b.rs").exists(), "sub/b.rs should exist");
}

#[test]
fn test_write_to_directory_inline_synthetic() {
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path();

    let files = vec![TestFile::new(
        "a.rs",
        "fn foo() -> MyEnum { todo!() }",
        vec![fn_returning("foo", "MyEnum")],
    )];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("MyEnum")];

    writer
        .write_to_directory(out, &outputs, &items, false)
        .unwrap();

    let content = std::fs::read_to_string(out.join("a.rs")).unwrap();
    assert!(
        content.starts_with("#[derive"),
        "inline synthetic should be at file start: {content}"
    );
    assert!(
        content.contains("fn foo()"),
        "original content should be preserved: {content}"
    );
}

#[test]
fn test_write_to_directory_shared_synthetic() {
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path();

    let files = vec![
        TestFile::new(
            "a.rs",
            "fn foo() -> SharedEnum { todo!() }",
            vec![fn_returning("foo", "SharedEnum")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SharedEnum { todo!() }",
            vec![fn_returning("bar", "SharedEnum")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("SharedEnum")];

    writer
        .write_to_directory(out, &outputs, &items, false)
        .unwrap();

    assert!(
        out.join("shared_types.rs").exists(),
        "shared_types.rs should be created for shared synthetics"
    );
    // mod.rs に pub mod shared_types; が含まれる
    let mod_content = std::fs::read_to_string(out.join("mod.rs")).unwrap();
    assert!(
        mod_content.contains("pub mod shared_types;"),
        "mod.rs should contain pub mod shared_types: {mod_content}"
    );
}

#[test]
fn test_write_to_directory_shared_synthetic_avoids_collision() {
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path();

    // ユーザーファイルに shared_types.rs が含まれる場合、衝突回避名を使用する
    let files = vec![
        TestFile::new(
            "shared_types.rs",
            "fn foo() -> SharedEnum { todo!() }",
            vec![fn_returning("foo", "SharedEnum")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SharedEnum { todo!() }",
            vec![fn_returning("bar", "SharedEnum")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("SharedEnum")];

    writer
        .write_to_directory(out, &outputs, &items, false)
        .unwrap();

    // shared_types.rs は衝突するので shared_types_0.rs に配置
    assert!(
        out.join("shared_types_0.rs").exists(),
        "shared_types_0.rs should be created when shared_types.rs collides"
    );
    // ユーザーの shared_types.rs は上書きされない
    let user_content = std::fs::read_to_string(out.join("shared_types.rs")).unwrap();
    assert!(
        user_content.contains("fn foo()"),
        "user file should not be overwritten: {user_content}"
    );
    // mod.rs に衝突回避後のモジュール名が含まれる
    let mod_content = std::fs::read_to_string(out.join("mod.rs")).unwrap();
    assert!(
        mod_content.contains("pub mod shared_types_0;"),
        "mod.rs should contain pub mod shared_types_0: {mod_content}"
    );
}

#[test]
fn test_write_to_directory_types_rs_not_overwritten() {
    use crate::ir::StructField;
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path();

    // ユーザーファイルに types.rs が含まれる場合（Hono のケース）
    // SharedEnum が 2 ファイルから参照されるため shared module に配置される
    // types.rs の user item: pub struct TypedResponse { data: SharedEnum }
    let typed_response = Item::Struct {
        vis: Visibility::Public,
        name: "TypedResponse".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "data".to_string(),
            ty: RustType::Named {
                name: "SharedEnum".to_string(),
                type_args: vec![],
            },
        }],
    };
    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct TypedResponse { data: SharedEnum }",
            vec![typed_response],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SharedEnum { todo!() }",
            vec![fn_returning("bar", "SharedEnum")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("SharedEnum")];

    writer
        .write_to_directory(out, &outputs, &items, false)
        .unwrap();

    // types.rs はユーザーコードを含む
    let types_content = std::fs::read_to_string(out.join("types.rs")).unwrap();
    assert!(
        types_content.contains("TypedResponse"),
        "types.rs should contain user code: {types_content}"
    );
    // 共有合成型は shared_types.rs に配置（types.rs と衝突しない）
    assert!(
        out.join("shared_types.rs").exists(),
        "shared_types.rs should be created"
    );
    let shared_content = std::fs::read_to_string(out.join("shared_types.rs")).unwrap();
    assert!(
        shared_content.contains("SharedEnum"),
        "shared_types.rs should contain SharedEnum: {shared_content}"
    );
}

// ===== I-371: shared_imports / 単一正準配置のテスト =====

#[test]
fn test_resolve_synthetic_placement_shared_imports_multi_file() {
    // 2 ファイルから参照される合成型は shared_types に配置され、
    // 両ファイルに `use crate::shared_types::Type;` のインポートが付与される。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let files = vec![
        TestFile::new(
            "a.rs",
            "fn foo() -> StringOrF64 { todo!() }",
            vec![fn_returning("foo", "StringOrF64")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> StringOrF64 { todo!() }",
            vec![fn_returning("bar", "StringOrF64")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("StringOrF64")];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    assert!(placement.shared_module.is_some());
    let imports_a = placement
        .shared_imports
        .get(Path::new("a.rs"))
        .expect("a.rs should have shared imports");
    assert_eq!(imports_a.len(), 1);
    assert_eq!(imports_a[0], "use crate::shared_types::StringOrF64;");
    let imports_b = placement
        .shared_imports
        .get(Path::new("b.rs"))
        .expect("b.rs should have shared imports");
    assert_eq!(imports_b[0], "use crate::shared_types::StringOrF64;");
}

#[test]
fn test_resolve_synthetic_placement_no_imports_for_inline() {
    // 1 ファイルからのみ参照される型は inline 配置で、shared_imports は空。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let files = vec![TestFile::new(
        "a.rs",
        "fn foo() -> LocalEnum { todo!() }",
        vec![fn_returning("foo", "LocalEnum")],
    )];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("LocalEnum")];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    assert!(placement.shared_imports.is_empty());
    assert!(placement.inline.contains_key(Path::new("a.rs")));
}

#[test]
fn test_resolve_synthetic_placement_imports_grouped() {
    // 同じファイルが複数の shared 型を参照する場合、単一の `use` 文に集約される。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let files = vec![
        TestFile::new(
            "a.rs",
            "fn foo() -> AlphaEnum { todo!() } fn baz() -> BetaEnum { todo!() }",
            vec![
                fn_returning("foo", "AlphaEnum"),
                fn_returning("baz", "BetaEnum"),
            ],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> AlphaEnum { todo!() } fn qux() -> BetaEnum { todo!() }",
            vec![
                fn_returning("bar", "AlphaEnum"),
                fn_returning("qux", "BetaEnum"),
            ],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![
        make_synthetic_enum("AlphaEnum"),
        make_synthetic_enum("BetaEnum"),
    ];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let imports_a = placement.shared_imports.get(Path::new("a.rs")).unwrap();
    assert_eq!(imports_a.len(), 1);
    assert_eq!(
        imports_a[0],
        "use crate::shared_types::{AlphaEnum, BetaEnum};"
    );
}

#[test]
fn test_write_to_directory_emits_shared_imports() {
    // shared_types.rs に配置された型を参照するファイルの先頭に
    // `use crate::shared_types::T;` が出力されること。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path();

    let files = vec![
        TestFile::new(
            "a.rs",
            "fn foo() -> SharedEnum { todo!() }",
            vec![fn_returning("foo", "SharedEnum")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SharedEnum { todo!() }",
            vec![fn_returning("bar", "SharedEnum")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("SharedEnum")];

    writer
        .write_to_directory(out, &outputs, &items, false)
        .unwrap();

    let a_content = std::fs::read_to_string(out.join("a.rs")).unwrap();
    assert!(
        a_content.contains("use crate::shared_types::SharedEnum;"),
        "a.rs should import SharedEnum: {a_content}"
    );
    let b_content = std::fs::read_to_string(out.join("b.rs")).unwrap();
    assert!(
        b_content.contains("use crate::shared_types::SharedEnum;"),
        "b.rs should import SharedEnum: {b_content}"
    );
}

#[test]
fn test_write_to_directory_no_duplicate_synthetic_definition() {
    // I-371 問題 1 の回帰テスト: 合成型が shared と inline の両方に
    // 重複定義されないことを検証する。
    // 単一ファイルから参照される合成型は inline 配置のみで shared_types.rs に出ない。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path();

    let files = vec![TestFile::new(
        "a.rs",
        "fn foo() -> OnlyOne { todo!() }",
        vec![fn_returning("foo", "OnlyOne")],
    )];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("OnlyOne")];

    writer
        .write_to_directory(out, &outputs, &items, false)
        .unwrap();

    let a_content = std::fs::read_to_string(out.join("a.rs")).unwrap();
    // a.rs に 1 回だけ enum 定義が出現
    assert_eq!(
        a_content.matches("pub enum OnlyOne").count(),
        1,
        "OnlyOne should be defined exactly once: {a_content}"
    );
    // shared_types.rs は作られない
    assert!(!out.join("shared_types.rs").exists());
}

#[test]
fn test_write_to_directory_shared_collision_uses_subscripted_import() {
    // shared_types.rs と衝突する場合、サフィックス付きモジュール名が
    // インポート文にも反映される。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path();

    let files = vec![
        TestFile::new("shared_types.rs", "pub struct UserDefined;", vec![]),
        TestFile::new(
            "a.rs",
            "fn foo() -> ConflictEnum { todo!() }",
            vec![fn_returning("foo", "ConflictEnum")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> ConflictEnum { todo!() }",
            vec![fn_returning("bar", "ConflictEnum")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum("ConflictEnum")];

    writer
        .write_to_directory(out, &outputs, &items, false)
        .unwrap();

    let a_content = std::fs::read_to_string(out.join("a.rs")).unwrap();
    assert!(
        a_content.contains("use crate::shared_types_0::ConflictEnum;"),
        "a.rs should import from shared_types_0: {a_content}"
    );
}

#[test]
fn test_choose_shared_module_path_no_collision() {
    let files = vec![
        TestFile::new("a.rs", "", vec![]),
        TestFile::new("b.rs", "", vec![]),
    ];
    let outputs = outputs_from(&files);
    let result = choose_shared_module_path(&outputs);
    assert_eq!(result, PathBuf::from("shared_types.rs"));
}

#[test]
fn test_choose_shared_module_path_collision() {
    let files = vec![
        TestFile::new("shared_types.rs", "", vec![]),
        TestFile::new("b.rs", "", vec![]),
    ];
    let outputs = outputs_from(&files);
    let result = choose_shared_module_path(&outputs);
    assert_eq!(result, PathBuf::from("shared_types_0.rs"));
}

#[test]
fn test_choose_shared_module_path_double_collision() {
    let files = vec![
        TestFile::new("shared_types.rs", "", vec![]),
        TestFile::new("shared_types_0.rs", "", vec![]),
    ];
    let outputs = outputs_from(&files);
    let result = choose_shared_module_path(&outputs);
    assert_eq!(result, PathBuf::from("shared_types_1.rs"));
}
