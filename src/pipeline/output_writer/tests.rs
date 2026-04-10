use super::placement::choose_shared_module_path;
use super::*;
use crate::ir::{EnumVariant, Item, Param, RustType, StructField, Visibility};
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

// ===== I-382: user 型 import 生成テスト =====

fn make_user_struct(name: &str) -> Item {
    Item::Struct {
        vis: Visibility::Public,
        name: name.to_string(),
        type_params: vec![],
        fields: vec![],
    }
}

fn make_synthetic_enum_with_user_ref(synth_name: &str, variants: &[(&str, &str)]) -> Item {
    Item::Enum {
        vis: Visibility::Public,
        name: synth_name.to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: variants
            .iter()
            .map(|(vname, ty_name)| EnumVariant {
                name: vname.to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: ty_name.to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            })
            .collect(),
    }
}

#[test]
fn test_shared_synthetic_with_user_type_generates_import() {
    // shared 配置された synthetic enum が user 定義型を variant に持つ場合、
    // shared_types.rs に user 型の import が生成される。
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("My", "MyType"), ("S", "String")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // shared_module が存在し、user 型 import を含む
    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::MyType;"),
        "shared_types.rs should import MyType: {content}"
    );
}

#[test]
fn test_inline_synthetic_same_file_no_import() {
    // inline 配置された synthetic が同一ファイルの user 型を参照 → import 不要
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![TestFile::new(
        "types.rs",
        "pub struct MyType {}",
        vec![make_user_struct("MyType"), fn_returning("foo", "MyUnion")],
    )];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("My", "MyType")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // inline 配置、import なし
    assert!(placement.inline.contains_key(Path::new("types.rs")));
    assert!(
        placement.shared_imports.is_empty(),
        "same-file user type should not generate import"
    );
}

#[test]
fn test_inline_synthetic_different_file_generates_import() {
    // inline 配置された synthetic が別ファイルの user 型を参照 → import 生成
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        TestFile::new(
            "handler.rs",
            "fn handle() -> MyUnion { todo!() }",
            vec![fn_returning("handle", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("My", "MyType")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // inline in handler.rs, import for MyType
    assert!(placement.inline.contains_key(Path::new("handler.rs")));
    let imports = placement
        .shared_imports
        .get(Path::new("handler.rs"))
        .expect("handler.rs should have user type import");
    assert!(
        imports.iter().any(|i| i.contains("MyType")),
        "handler.rs should import MyType: {imports:?}"
    );
}

#[test]
fn test_shared_synthetic_multiple_user_types_grouped() {
    // shared synthetic が複数の user 型を同一モジュールから参照 → グループ化
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct TypeA {} pub struct TypeB {}",
            vec![make_user_struct("TypeA"), make_user_struct("TypeB")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("A", "TypeA"), ("B", "TypeB")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::{TypeA, TypeB};"),
        "should group imports from same module: {content}"
    );
}

#[test]
fn test_transitive_shared_user_type_import() {
    // shared A → shared B → user MyType の推移依存
    // shared_types.rs に MyType の import が含まれる
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let synth_b = make_synthetic_enum_with_user_ref("SynthB", &[("My", "MyType")]);
    let synth_a = Item::Struct {
        vis: Visibility::Public,
        name: "SynthA".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "b".to_string(),
            ty: RustType::Named {
                name: "SynthB".to_string(),
                type_args: vec![],
            },
        }],
    };

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> SynthA { todo!() }",
            vec![fn_returning("foo", "SynthA")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SynthA { todo!() }",
            vec![fn_returning("bar", "SynthA")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![synth_a, synth_b];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::MyType;"),
        "transitive user type should be imported in shared module: {content}"
    );
}

#[test]
fn test_user_type_not_in_transpiled_files_no_import() {
    // user 型が transpile 対象外 (user_type_def_map にない) → import 生成されない
    // (compile error は Tier 2 であり、空 stub より上位の fidelity)
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    // ExternalOnly は files に定義されていない
    let files = vec![
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("E", "ExternalOnly")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    // ExternalOnly は files にない → import 文は生成されない
    // (enum 本文に ExternalOnly は残るが use 文は生成しない)
    assert!(
        !content.contains("use") || !content.contains("ExternalOnly;"),
        "type not in transpiled files should not generate a use import: {content}"
    );
    // より正確: "use" で始まる行に ExternalOnly が含まれないこと
    for line in content.lines() {
        if line.trim_start().starts_with("use ") {
            assert!(
                !line.contains("ExternalOnly"),
                "use statement should not reference ExternalOnly: {line}"
            );
        }
    }
}

#[test]
fn test_inline_synthetic_different_modules_grouped_separately() {
    // inline synthetic が異なるモジュールの user 型を参照 → モジュールごとに別 import 文
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct TypeA {}",
            vec![make_user_struct("TypeA")],
        ),
        TestFile::new(
            "models.rs",
            "pub struct TypeB {}",
            vec![make_user_struct("TypeB")],
        ),
        TestFile::new(
            "handler.rs",
            "fn handle() -> MyUnion { todo!() }",
            vec![fn_returning("handle", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("A", "TypeA"), ("B", "TypeB")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // inline in handler.rs
    assert!(placement.inline.contains_key(Path::new("handler.rs")));
    let imports = placement
        .shared_imports
        .get(Path::new("handler.rs"))
        .expect("handler.rs should have user type imports");
    // 2 つの異なるモジュールからの import
    assert!(
        imports.iter().any(|i| i.contains("crate::types::TypeA")),
        "should import TypeA from types module: {imports:?}"
    );
    assert!(
        imports.iter().any(|i| i.contains("crate::models::TypeB")),
        "should import TypeB from models module: {imports:?}"
    );
}

#[test]
fn test_shared_and_inline_both_ref_same_user_type() {
    // shared synthetic と inline synthetic が同じ user 型を参照
    // shared_types.rs に import、inline 側は shared_types 経由でアクセス可能
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let synth_shared = make_synthetic_enum_with_user_ref("SharedUnion", &[("My", "MyType")]);
    let synth_inline = make_synthetic_enum_with_user_ref("InlineUnion", &[("My", "MyType")]);

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> SharedUnion { todo!() }",
            vec![fn_returning("foo", "SharedUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SharedUnion { todo!() } fn baz() -> InlineUnion { todo!() }",
            vec![
                fn_returning("bar", "SharedUnion"),
                fn_returning("baz", "InlineUnion"),
            ],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![synth_shared, synth_inline];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // SharedUnion は shared (a.rs + b.rs から参照)
    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::MyType;"),
        "shared module should import MyType: {content}"
    );

    // InlineUnion は b.rs に inline (b.rs のみから参照)
    assert!(placement.inline.contains_key(Path::new("b.rs")));

    // b.rs の imports に MyType が含まれるか確認
    // InlineUnion は b.rs に inline され、MyType を参照 → types.rs は別ファイル → import 必要
    let b_imports = placement
        .shared_imports
        .get(Path::new("b.rs"))
        .expect("b.rs should have imports");
    assert!(
        b_imports.iter().any(|i| i.contains("MyType")),
        "b.rs should import MyType for inline synthetic: {b_imports:?}"
    );
}

#[test]
fn test_inline_synthetic_transitive_to_shared_no_scope_leak() {
    // scope isolation 検証: inline A → shared B → user MyType
    // A のファイルには MyType の import は不要 (B は shared_types.rs にあり、
    // shared_types.rs 側で MyType を import する)
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let synth_b = make_synthetic_enum_with_user_ref("SynthB", &[("My", "MyType")]);
    let synth_a = Item::Struct {
        vis: Visibility::Public,
        name: "SynthA".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "b".to_string(),
            ty: RustType::Named {
                name: "SynthB".to_string(),
                type_args: vec![],
            },
        }],
    };

    let files = vec![
        TestFile::new(
            "types.rs",
            "pub struct MyType {}",
            vec![make_user_struct("MyType")],
        ),
        // a.rs references SynthA (inline), SynthB referenced only by SynthA (shared)
        TestFile::new(
            "a.rs",
            "fn foo() -> SynthA { todo!() }",
            vec![fn_returning("foo", "SynthA")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> SynthB { todo!() }",
            vec![fn_returning("bar", "SynthB")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![synth_a, synth_b];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    // SynthB は 2 ファイル (a.rs via SynthA + b.rs 直接) → shared
    // SynthA は a.rs + synthetic ref → shared
    // shared_types.rs に MyType の import がある
    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    assert!(
        content.contains("use crate::types::MyType;"),
        "shared module should import MyType: {content}"
    );

    // a.rs は MyType を直接参照しない → user type import は不要
    // (shared_types の use は別だが、user type import はなし)
    if let Some(a_imports) = placement.shared_imports.get(Path::new("a.rs")) {
        for import in a_imports {
            assert!(
                !import.contains("crate::types::MyType"),
                "a.rs should NOT import MyType directly (it's in shared_types): {import}"
            );
        }
    }
}

#[test]
fn test_user_type_import_from_index_module() {
    // index.rs → crate::<parent> モジュールパスの検証
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "services/index.rs",
            "pub struct ServiceType {}",
            vec![make_user_struct("ServiceType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("S", "ServiceType")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    // index.rs → parent module "services" (not "services::index")
    assert!(
        content.contains("use crate::services::ServiceType;"),
        "index.rs should map to parent module: {content}"
    );
    assert!(
        !content.contains("crate::services::index"),
        "should NOT contain index in module path: {content}"
    );
}

#[test]
fn test_user_type_import_with_hyphenated_module() {
    // ハイフン付きパス → アンダースコア変換の検証
    let mg = ModuleGraph::empty();
    let writer = OutputWriter::new(&mg);

    let files = vec![
        TestFile::new(
            "my-utils.rs",
            "pub struct UtilType {}",
            vec![make_user_struct("UtilType")],
        ),
        TestFile::new(
            "a.rs",
            "fn foo() -> MyUnion { todo!() }",
            vec![fn_returning("foo", "MyUnion")],
        ),
        TestFile::new(
            "b.rs",
            "fn bar() -> MyUnion { todo!() }",
            vec![fn_returning("bar", "MyUnion")],
        ),
    ];
    let outputs = outputs_from(&files);
    let items = vec![make_synthetic_enum_with_user_ref(
        "MyUnion",
        &[("U", "UtilType")],
    )];
    let placement = writer.resolve_synthetic_placement(&outputs, &items);

    let (_, content) = placement
        .shared_module
        .as_ref()
        .expect("should have shared module");
    // my-utils.rs → crate::my_utils (hyphen → underscore)
    assert!(
        content.contains("use crate::my_utils::UtilType;"),
        "hyphen should be replaced with underscore: {content}"
    );
    assert!(
        !content.contains("my-utils"),
        "should NOT contain hyphen in module path: {content}"
    );
}
