//! I-376: 外部型 struct の構造的 dedup 検証。
//!
//! 外部型 (ArrayBuffer/Date/Error 等、`TypeRegistry::is_external` が true を返す型) は
//! `SyntheticTypeRegistry` に 1 回だけ登録され、`file_outputs[i].items` には構造的に
//! 含まれないことを直接 assert する。これにより `file.items` と `synthetic_items` の
//! 外部型重複が構造的に不可能であることを保証する。

use std::collections::HashSet;
use std::path::PathBuf;

use ts_to_rs::external_types::load_builtin_types;
use ts_to_rs::ir::Item;
use ts_to_rs::pipeline::{module_resolver::TrivialResolver, transpile_pipeline, TranspileInput};

fn run(sources: Vec<(&str, &str)>) -> ts_to_rs::pipeline::TranspileOutput {
    let (builtin, base) = load_builtin_types().expect("load builtins");
    let files = sources
        .into_iter()
        .map(|(name, src)| (PathBuf::from(name), src.to_string()))
        .collect();
    let input = TranspileInput {
        files,
        builtin_types: Some(builtin),
        base_synthetic: Some(base),
        module_resolver: Box::new(TrivialResolver),
    };
    transpile_pipeline(input).expect("pipeline succeeds")
}

fn file_struct_names(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Struct { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn synthetic_struct_names(items: &[Item]) -> Vec<&str> {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Struct { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

/// `synthetic_items` に同名 struct が 2 件以上存在しないことを厳格に検証する。
fn assert_no_duplicate_synthetic_structs(items: &[Item]) {
    let mut seen = HashSet::new();
    for name in synthetic_struct_names(items) {
        assert!(
            seen.insert(name.to_string()),
            "synthetic_items contains duplicate struct: {name}"
        );
    }
}

#[test]
fn external_type_not_duplicated_in_file_items_single_file() {
    // ArrayBuffer を参照する単一 TS ファイル。
    let ts = r#"
function takes(buf: ArrayBuffer): number {
    return buf.byteLength;
}
"#;
    let output = run(vec![("a.ts", ts)]);

    // file_outputs[0].items には外部型 struct が含まれてはならない。
    let file_structs = file_struct_names(&output.files[0].items);
    assert!(
        !file_structs.iter().any(|n| n == "ArrayBuffer"),
        "file.items should not contain external type ArrayBuffer struct, got: {file_structs:?}"
    );

    // synthetic_items には ArrayBuffer が 1 つだけ存在するべき。
    let ab_count = output
        .synthetic_items
        .iter()
        .filter(|i| matches!(i, Item::Struct { name, .. } if name == "ArrayBuffer"))
        .count();
    assert_eq!(
        ab_count, 1,
        "synthetic_items must contain exactly one ArrayBuffer struct; got {ab_count}"
    );
    assert_no_duplicate_synthetic_structs(&output.synthetic_items);
}

#[test]
fn external_type_not_duplicated_in_file_items_multi_file() {
    // 2 ファイルが同じ外部型を参照しても、synthetic_items に 1 つだけ。
    let a = r#"
function takes_a(buf: ArrayBuffer): number { return buf.byteLength; }
"#;
    let b = r#"
function takes_b(buf: ArrayBuffer): number { return buf.byteLength; }
"#;
    let output = run(vec![("a.ts", a), ("b.ts", b)]);

    for (i, f) in output.files.iter().enumerate() {
        let names = file_struct_names(&f.items);
        assert!(
            !names.iter().any(|n| n == "ArrayBuffer"),
            "file[{i}].items must not contain ArrayBuffer, got: {names:?}"
        );
    }

    assert_no_duplicate_synthetic_structs(&output.synthetic_items);
    let ab_count = output
        .synthetic_items
        .iter()
        .filter(|i| matches!(i, Item::Struct { name, .. } if name == "ArrayBuffer"))
        .count();
    assert_eq!(
        ab_count, 1,
        "multi-file: synthetic_items must contain exactly one ArrayBuffer struct; got {ab_count}"
    );
}

#[test]
fn external_type_transitive_dependency_resolved_once() {
    // `Response` / `Request` はフィールド型として `Headers` / `Body` 等の他の外部型を
    // 参照する (builtin_types/web_api.json 定義)。Phase 5a の fixpoint が推移依存を
    // 正しく解決し、参照される外部型がすべて synthetic_items に登録されることを検証する。
    let ts = r#"
function handle(req: Request): Response {
    return new Response();
}
"#;
    let output = run(vec![("a.ts", ts)]);

    let synth_names: HashSet<&str> = synthetic_struct_names(&output.synthetic_items)
        .into_iter()
        .collect();

    // 直接参照された Request / Response は synthetic に入っているはず。
    assert!(
        synth_names.contains("Request"),
        "Request should be in synthetic_items; got: {synth_names:?}"
    );
    assert!(
        synth_names.contains("Response"),
        "Response should be in synthetic_items; got: {synth_names:?}"
    );

    // 推移依存で参照される Headers も含まれるはず (Response.headers の型)。
    assert!(
        synth_names.contains("Headers"),
        "transitive dep Headers should be resolved; got: {synth_names:?}"
    );

    // file.items には外部型 struct が一切入っていない。
    let file_structs = file_struct_names(&output.files[0].items);
    for name in ["Request", "Response", "Headers"] {
        assert!(
            !file_structs.iter().any(|n| n == name),
            "file.items must not contain external type {name}; got: {file_structs:?}"
        );
    }

    assert_no_duplicate_synthetic_structs(&output.synthetic_items);
}

#[test]
fn user_defined_type_not_stubbed_in_synthetic_items() {
    // I-376 C3: Phase 5c stub pass は user 定義型を stub 化してはならない。
    // 以前は synthetic any-enum が user 定義型 (MyError 等) を variant で参照するとき、
    // shared_types.rs stub 生成が空の `pub struct MyError;` を synthetic_items に
    // 混入させていた。これは user 定義の MyError と型が別物になるため silent
    // semantic bug。`defined_elsewhere_names` の exclusion が正しく機能することを検証。
    let ts = r#"
class MyError {
    message: string;
    constructor(msg: string) { this.message = msg; }
}

function classify(x: any): string {
    if (x instanceof MyError) {
        return x.message;
    }
    return "other";
}
"#;
    let output = run(vec![("a.ts", ts)]);

    // synthetic_items に `MyError` の空 stub struct が含まれないこと。
    // (user 定義版は file.items にあり、synthetic 側に stub を作ると二重定義になる)
    let synth_my_error: Vec<&Item> = output
        .synthetic_items
        .iter()
        .filter(|i| matches!(i, Item::Struct { name, .. } if name == "MyError"))
        .collect();
    assert!(
        synth_my_error.is_empty(),
        "synthetic_items must not contain a stub for user-defined MyError; got {} item(s)",
        synth_my_error.len()
    );

    // file.items には MyError 定義が存在する (user 由来)。
    let file_structs = file_struct_names(&output.files[0].items);
    assert!(
        file_structs.iter().any(|n| n == "MyError"),
        "user-defined MyError should remain in file.items; got: {file_structs:?}"
    );
}
