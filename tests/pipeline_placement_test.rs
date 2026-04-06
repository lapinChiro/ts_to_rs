//! Integration tests for I-371 IR-based synthetic type placement.
//!
//! These tests exercise the full pipeline (`transpile_pipeline` + `OutputWriter`) with
//! multi-file inputs and verify that:
//!  1. Synthetic types referenced from multiple files are placed in `shared_types.rs`,
//!     not duplicated per-file.
//!  2. Inline-placed synthetic types whose dependencies live in shared get correct
//!     `use crate::shared_types::*;` imports.
//!  3. Synthetic types that are not referenced are not emitted at all.
//!  4. Synthetic dependency chains (`A → B → C`) propagate placement correctly.
//!  5. References from sub-directories generate `use crate::shared_types::*;` paths.
//!  6. Self-referential synthetic types do not cause infinite loops.

use std::collections::HashSet;
use std::path::PathBuf;

use ts_to_rs::pipeline::output_writer::OutputWriter;
use ts_to_rs::pipeline::{
    module_resolver::NodeModuleResolver, transpile_pipeline, OutputFile, TranspileInput,
};

/// テスト共通: 与えられた `(rel_path, ts_source)` 入力を pipeline に投げ、出力ディレクトリの
/// `(rel_path, content)` マップを返す。
fn run_pipeline(files: Vec<(&str, &str)>) -> std::collections::HashMap<PathBuf, String> {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let input_dir = tmp.path().join("input");
    let output_dir = tmp.path().join("output");
    std::fs::create_dir_all(&input_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();

    let mut input_files: Vec<(PathBuf, String)> = Vec::new();
    let mut ts_paths: Vec<PathBuf> = Vec::new();
    for (rel, source) in &files {
        let ts_path = input_dir.join(rel);
        if let Some(parent) = ts_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&ts_path, source).unwrap();
        input_files.push((ts_path.clone(), (*source).to_string()));
        ts_paths.push(ts_path);
    }

    let known_files: HashSet<PathBuf> = ts_paths.iter().cloned().collect();
    let pipeline_input = TranspileInput {
        files: input_files,
        builtin_types: None,
        base_synthetic: None,
        module_resolver: Box::new(NodeModuleResolver::new(input_dir.clone(), known_files)),
    };
    let output = transpile_pipeline(pipeline_input).expect("pipeline succeeds");

    let outputs: Vec<OutputFile<'_>> = output
        .files
        .iter()
        .map(|fo| {
            let rel = fo
                .path
                .strip_prefix(&input_dir)
                .map(|p| p.with_extension("rs"))
                .unwrap_or_else(|_| fo.path.with_extension("rs"));
            OutputFile {
                rel_path: rel,
                source: &fo.rust_source,
                items: &fo.items,
            }
        })
        .collect();

    let writer = OutputWriter::new(&output.module_graph);
    writer
        .write_to_directory(&output_dir, &outputs, &output.synthetic_items, false)
        .expect("write_to_directory");

    // Read all written files into a map (recursive)
    let mut result = std::collections::HashMap::new();
    fn walk(
        dir: &std::path::Path,
        base: &std::path::Path,
        out: &mut std::collections::HashMap<PathBuf, String>,
    ) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path.strip_prefix(base).unwrap().to_path_buf();
                let content = std::fs::read_to_string(&path).unwrap();
                out.insert(rel, content);
            }
        }
    }
    walk(&output_dir, &output_dir, &mut result);
    result
}

#[test]
fn test_cross_file_synthetic_dedup_via_shared_types() {
    // 2 ファイルが同じ union 型を生成 → shared_types.rs に集約され、
    // 各ファイルは use crate::shared_types::*; で参照する。各ファイルに重複 stub が
    // 生成されてはならない。
    let outputs = run_pipeline(vec![
        (
            "a.ts",
            "export function f(x: string | number): string { return ''; }",
        ),
        (
            "b.ts",
            "export function g(x: string | number): string { return ''; }",
        ),
    ]);

    let shared = outputs
        .get(&PathBuf::from("shared_types.rs"))
        .expect("shared_types.rs should exist");
    // union enum の本体は shared に存在
    assert!(shared.contains("enum F64OrString") || shared.contains("enum StringOrF64"));

    let a = outputs.get(&PathBuf::from("a.rs")).expect("a.rs");
    let b = outputs.get(&PathBuf::from("b.rs")).expect("b.rs");
    // 各ファイルは shared_types を import する
    assert!(a.contains("use crate::shared_types"));
    assert!(b.contains("use crate::shared_types"));
    // 各ファイルに enum 定義は再生成されない
    assert!(
        !a.contains("enum F64OrString") && !a.contains("enum StringOrF64"),
        "a.rs should not redefine the union enum, got:\n{a}"
    );
    assert!(
        !b.contains("enum F64OrString") && !b.contains("enum StringOrF64"),
        "b.rs should not redefine the union enum, got:\n{b}"
    );
}

#[test]
fn test_referenced_synthetic_defined_exactly_once() {
    // synthetic union がパイプラインを通して **必ず一度だけ** 定義されることを検証する
    // （重複定義禁止と漏れ禁止の同時保証）。これは I-371 のコア goal:
    //   - 同一合成型の重複生成を避ける
    //   - 参照される合成型は出力に必ず含まれる
    // を同時に確認する。
    let outputs = run_pipeline(vec![(
        "only.ts",
        "export function f(x: string | number): string { return ''; }",
    )]);

    let total_enum_defs = outputs
        .values()
        .map(|c| {
            c.matches("pub enum F64OrString").count() + c.matches("pub enum StringOrF64").count()
        })
        .sum::<usize>();
    assert_eq!(
        total_enum_defs,
        1,
        "synthetic union enum must be defined exactly once across all output files; outputs: {:?}",
        outputs.keys().collect::<Vec<_>>()
    );
}

#[test]
fn test_truly_unreferenced_synthetic_not_emitted() {
    // 完全に参照されない合成型は出力されない。union を一切登場させない TS から
    // pipeline を回し、出力に union enum が含まれないことを確認する。
    let outputs = run_pipeline(vec![(
        "noref.ts",
        "export function f(): string { return ''; }",
    )]);

    let total_enum_defs = outputs
        .values()
        .map(|c| {
            c.matches("pub enum F64OrString").count() + c.matches("pub enum StringOrF64").count()
        })
        .sum::<usize>();
    assert_eq!(
        total_enum_defs, 0,
        "no synthetic union enum should be emitted when none is referenced"
    );
}

#[test]
fn test_synthetic_chain_a_to_b_all_referenced_types_emitted() {
    // 合成型が他の合成型を参照する連鎖ケースの検証。
    //
    // 検証戦略: 2 ファイルが (string|number)[] を引数として受ける。各ファイルは
    //   - 配列要素型として union (StringOrF64) を生成
    //   - パラメータ型として `Vec<StringOrF64>` を経由して間接参照
    // となり、union enum 1 個が shared に置かれる。出力に union 定義が必ず存在し、
    // 重複しないことを確認する（union が複数生成される TS なら推移閉包が機能している）。
    let outputs = run_pipeline(vec![
        (
            "a.ts",
            "export function f(items: (string | number)[]): string { return ''; }",
        ),
        (
            "b.ts",
            "export function g(items: (string | number)[]): string { return ''; }",
        ),
    ]);
    // 全出力ファイル中、union enum 定義はちょうど 1 個
    let total_enum_defs = outputs
        .values()
        .map(|c| {
            c.matches("pub enum F64OrString").count() + c.matches("pub enum StringOrF64").count()
        })
        .sum::<usize>();
    assert_eq!(
        total_enum_defs, 1,
        "Vec<union> 経由でも union 定義は 1 個に dedup される"
    );
    // shared_types.rs に置かれている
    let shared = outputs
        .get(&PathBuf::from("shared_types.rs"))
        .expect("shared_types.rs");
    assert!(
        shared.contains("pub enum F64OrString") || shared.contains("pub enum StringOrF64"),
        "クロスファイル参照の union は shared に置かれる"
    );
}

#[test]
fn test_synthetic_inner_dependency_chain() {
    // synthetic 内部依存の推移閉包テスト（unit-test レベルでは placement.rs の
    // test_render_transitive_synthetic_emitted で検証済）。
    //
    // 統合テスト層では Vec<union>, Option<union> 等の合成型 in 合成型のケースが
    // クラッシュしないことを確認する。
    let outputs = run_pipeline(vec![
        (
            "a.ts",
            "export function f(x: (string | number) | null): string { return ''; }",
        ),
        (
            "b.ts",
            "export function g(x: (string | number) | null): string { return ''; }",
        ),
    ]);
    // パイプラインが完了し、出力ファイルが生成されること
    assert!(outputs.contains_key(&PathBuf::from("a.rs")));
    assert!(outputs.contains_key(&PathBuf::from("b.rs")));
}

#[test]
fn test_subdirectory_uses_crate_shared_types_path() {
    // サブディレクトリ配下のファイルからも `use crate::shared_types::*;` 形式で参照できる
    let outputs = run_pipeline(vec![
        (
            "utils/x.ts",
            "export function f(x: string | number): string { return ''; }",
        ),
        (
            "root.ts",
            "export function g(x: string | number): string { return ''; }",
        ),
    ]);
    let utils_x = outputs
        .get(&PathBuf::from("utils/x.rs"))
        .expect("utils/x.rs");
    assert!(
        utils_x.contains("use crate::shared_types"),
        "subdirectory file should use crate-rooted path, got:\n{utils_x}"
    );
}

#[test]
fn test_self_referential_synthetic_does_not_loop() {
    // 自己参照型（再帰型）を含む TS。pipeline + OutputWriter は無限ループせず完了する。
    let outputs = run_pipeline(vec![(
        "rec.ts",
        // タプルや union を介した自己参照は transformer が生成する synthetic 型でしか
        // 起きないため、最低限「pipeline が終了する」ことを検証する。
        "export interface Node { children: Node[]; value: string; }",
    )]);
    // 完了すれば成功。出力ファイルが少なくとも 1 つ存在することを確認。
    assert!(!outputs.is_empty());
}

#[test]
fn test_inline_placement_when_only_one_file_references_synthetic() {
    // 2 ファイルあるが synthetic を使うのは 1 ファイルのみ → inline 配置（shared には
    // 置かない）。inline 配置のファイルは use crate::shared_types を持たない。
    let outputs = run_pipeline(vec![
        (
            "uses.ts",
            "export function f(x: string | number): string { return ''; }",
        ),
        (
            "plain.ts",
            "export function g(x: string): string { return ''; }",
        ),
    ]);

    let uses = outputs.get(&PathBuf::from("uses.rs")).expect("uses.rs");
    // synthetic 型本体が uses.rs 内に inline で配置されている
    assert!(
        uses.contains("enum F64OrString") || uses.contains("enum StringOrF64"),
        "single-referencer file should inline the synthetic enum, got:\n{uses}"
    );
    // shared_types.rs が存在する場合、その中に該当 enum はいない
    if let Some(shared) = outputs.get(&PathBuf::from("shared_types.rs")) {
        assert!(
            !shared.contains("enum F64OrString") && !shared.contains("enum StringOrF64"),
            "single-use synthetic must not be in shared_types.rs"
        );
    }
}
