use std::fs;
use std::process::Command;
use std::sync::Mutex;

use ts_to_rs::pipeline::module_resolver::TrivialResolver;
use ts_to_rs::pipeline::TranspileInput;
use ts_to_rs::{transpile_collecting, transpile_with_builtins};

#[path = "test_helpers.rs"]
mod test_helpers;

/// Path to the fixed Cargo project used for compile checking.
const COMPILE_CHECK_DIR: &str = "tests/compile-check";

/// Mutex to serialize compile tests (they share the same compile-check project).
static COMPILE_LOCK: Mutex<()> = Mutex::new(());

/// Lint configuration for compile tests.
///
/// Allow: lints that are expected in transpiler output (unused definitions, unused variables, etc.).
/// Deny: lints that indicate genuine conversion quality problems.
const COMPILE_TEST_LINT_PRELUDE: &str = "\
    #![allow(dead_code, unused_variables, unused_imports, unused_assignments)]\n\
    #![deny(unused_mut, unreachable_code)]\n";

/// Simplifies `use crate::...::module::Name` to `use module::Name` for multi-file compilation.
///
/// In multi-file compile tests, sibling modules are available as `mod <name>;`,
/// so `use crate::long::path::env::Bindings` becomes `use env::Bindings`.
fn simplify_use_statements(rs_source: &str) -> String {
    rs_source
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if (trimmed.starts_with("use crate::") || trimmed.starts_with("pub use crate::"))
                && trimmed.ends_with(';')
            {
                // Extract the last two segments: module::Name
                let path_part = trimmed
                    .trim_start_matches("pub use crate::")
                    .trim_start_matches("use crate::")
                    .trim_end_matches(';');
                let segments: Vec<&str> = path_part.split("::").collect();
                if segments.len() >= 2 {
                    let short = segments[segments.len() - 2..].join("::");
                    format!("use {};", short)
                } else {
                    line.to_string()
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

use test_helpers::{strip_internal_use_statements, TempFile};

/// Compiles the given Rust source code via `cargo check` against the fixed
/// compile-check project (which has external crate dependencies).
///
/// Caller must hold `COMPILE_LOCK` before calling this function.
fn assert_compiles(rs_source: &str, fixture_name: &str) {
    let compilable_source = strip_internal_use_statements(rs_source);

    // Write the generated code to the compile-check project's src/lib.rs
    let lib_path = format!("{COMPILE_CHECK_DIR}/src/lib.rs");
    // Suppress warnings and import external crate items used by generated code.
    // Auto-detect additional imports needed (mirrors output_writer::generate_types_rs_imports).
    let mut auto_imports = String::from("use serde::{Serialize, Deserialize};\n");
    if compilable_source.contains("serde_json::") {
        auto_imports.push_str("use serde_json;\n");
    }
    if compilable_source.contains("HashMap<") {
        auto_imports.push_str("use std::collections::HashMap;\n");
    }
    let full_source = format!(
        "{COMPILE_TEST_LINT_PRELUDE}\
         {auto_imports}\
         {}",
        compilable_source
    );
    fs::write(&lib_path, &full_source)
        .unwrap_or_else(|e| panic!("failed to write {lib_path}: {e}"));

    let output = Command::new("cargo")
        .args(["check", "--message-format=short"])
        .current_dir(COMPILE_CHECK_DIR)
        .output()
        .expect("failed to execute cargo check");

    assert!(
        output.status.success(),
        "cargo check failed for fixture '{fixture_name}':\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_all_fixtures_compile() {
    let _lock = COMPILE_LOCK.lock().unwrap();

    let fixture_dir = "tests/fixtures";
    let mut fixture_count = 0;

    // Fixtures that cannot compile in isolation due to reasons OTHER than missing crates:
    let skip_compile = [
        // Indexed access type `Env['Bindings']` generates `Env::Bindings` which references
        // undefined type `Env`. Requires multi-file compilation (tested in test_multi_file_fixtures_compile).
        "indexed-access-type",
        // trait-coercion uses `null as any` which generates `None` (not a valid Box<dyn Trait>).
        // The trait coercion (&*g) is correct; the issue is unrelated `null as any` conversion.
        "trait-coercion",
        // (union-fallback: I-008 derive 条件付き化で解消。skip 解除)
        // any-type-narrowing uses `null` assigned to enum type which generates `None`.
        // Same root cause as I-201 (null as any → None).
        "any-type-narrowing",
        // (ternary-union: I-009 union return wrap で解消。skip 解除)
        // vec-method-expected-type: ビルトイン型（Array メソッドシグネチャ）が必要。
        // コンパイルテストは transpile_collecting（ビルトインなし）で実行されるため、
        // push の引数に expected type が伝播せず _TypeLit0 が生成される。
        // snapshot テストは transpile_with_builtins で正しく動作。
        "vec-method-expected-type",
        // type-narrowing: I-212 (enum 重複定義) は P8 統一パイプラインで解消済み。
        // 残存コンパイルエラー（I-212 とは無関係）:
        //   - f64.toFixed(2): JS 固有メソッドの Rust 変換が未対応（TODO: format!("{:.N}", v) に変換）
        //   - StringOrF64 の Display 未実装: println! で enum を表示するには Display trait が必要
        "type-narrowing",
        // (array-builtin-methods: I-011 deref_closure_params + I-012 find return type preservation
        // により解消。Step 2 PRD 完了)
        // instanceof-builtin: any-narrowing generates enum variants Date(Date), Error(Error), RegExp(RegExp).
        // With builtins loaded, struct definitions are generated (I-270). However, method calls
        // (e.g., x.toISOString()) require impl blocks which are not yet generated (I-270c).
        // String(x) constructor call also doesn't compile.
        "instanceof-builtin",
        // external-type-struct: requires builtin types loaded (transpile_with_builtins) to generate
        // external type struct definitions. The compile test uses transpile_collecting (no builtins).
        // Tested separately in test_external_type_struct integration test with builtins.
        "external-type-struct",
        // intersection-empty-object: `type NonIdentity<T> = HashMap<String, String>` has unused
        // type parameter T (E0091). Mapped type with non-identity value type loses T usage (I-314).
        "intersection-empty-object",
        // (basic-types: コンパイル通過確認済み。skip 解除)
        // (async-await: Phase A Step 4 で I-023 解消。skip 解除 — try body 常時 return +
        // throw 無しのケースで `!`-typed labeled block を検出し machinery を drop)
        // closures: Box wrap (I-020) 解消済。残: closure capture move/FnMut (I-048 所有権推論)。
        "closures",
        // (discriminated-union: Phase A Step 4 で I-021 解消。skip 解除 — template literal
        // 内の DU field access を `x.clone()` に rewrite + unit variant pattern を
        // `Pattern::UnitStruct` に変更)
        // (I-273 fixed: generic-class removed from skip list)
        // (I-325 fixed: object-destructuring removed from skip list)
        // (ternary: I-009 union return wrap で解消。skip 解除)
        // functions: Vec<String> index move (I-319). (I-020 Box wrap は Step 3 で解消)
        "functions",
        // keyword-types: I-025 implicit None は解消。残: I-146 (`return undefined` on void fn → `None` instead of `return;`)。
        "keyword-types",
        // (nullish-coalescing: I-022 + I-142 で解消。skip 解除)
        // string-methods: slice/indexOf/split/charAt/repeat conversion bugs (I-329).
        "string-methods",
        // type-assertion: `as unknown as T` and union assertion type mismatch.
        "type-assertion",
        // (void-type: I-025 implicit None で解消。Step 3 skip 解除)
        // (async-class-method: P4.2 で Promise unwrap 実装完了。skip 解除)
        // (callable-interface, call-signature-rest, interface-mixed,
        // callable-interface-param-rename, callable-interface-inner:
        // P8.2 統合チェックポイントで復帰完了)
        // callable-interface-generic-arity-mismatch: 意図的に変換 error を発生させる
        // error-case fixture (INV-4)。transpile_collecting が Err を返すため skip 必須。
        "callable-interface-generic-arity-mismatch",
    ];

    let mut entries: Vec<_> = fs::read_dir(fixture_dir)
        .expect("failed to read fixtures directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| {
                    n.ends_with(".input.ts") && !skip_compile.iter().any(|s| n.starts_with(s))
                })
        })
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let fixture_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("invalid fixture filename");

        let ts_source = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("failed to read fixture: {}", path.display()));
        let (rs_source, _unsupported) = transpile_collecting(&ts_source)
            .unwrap_or_else(|_| panic!("failed to transpile fixture: {}", path.display()));

        assert_compiles(&rs_source, fixture_name);
        fixture_count += 1;
    }

    assert!(
        fixture_count > 0,
        "no fixtures found in {fixture_dir} — test is vacuously passing"
    );
}

/// Same as `test_all_fixtures_compile` but with built-in types loaded.
///
/// This catches regressions in code that depends on builtin type definitions
/// (Array methods, Response/Request constructors, etc.) which the builtins-free
/// test cannot detect.
#[test]
fn test_all_fixtures_compile_with_builtins() {
    let _lock = COMPILE_LOCK.lock().unwrap();

    let fixture_dir = "tests/fixtures";
    let mut fixture_count = 0;

    // Fixtures that cannot compile even WITH builtins (non-builtin-related issues):
    let skip_compile_with_builtins = [
        "indexed-access-type",
        "trait-coercion",
        // (union-fallback: I-008 derive 条件付き化で解消。skip 解除)
        "any-type-narrowing",
        // (ternary-union: I-009 union return wrap で解消。skip 解除)
        "type-narrowing",
        // (array-builtin-methods: I-011 + I-012 解消により with-builtins でも通過)
        // instanceof-builtin: method impl blocks not generated (I-270c).
        "instanceof-builtin",
        // (external-type-struct: I-007 Display 生成で解消。skip 解除)
        // intersection-empty-object: unused type parameter T (E0091) (I-314)
        "intersection-empty-object",
        // (basic-types: コンパイル通過確認済み。skip 解除)
        // (I-273 fixed: generic-class removed from skip list)
        // (I-325 fixed: object-destructuring removed from skip list)
        // (ternary: I-009 union return wrap で解消。skip 解除)
        // (async-await: Phase A Step 4 で I-023 解消。skip 解除)
        "closures",
        // (discriminated-union: Phase A Step 4 で I-021 解消。skip 解除)
        "functions",
        "keyword-types",
        // (nullish-coalescing: I-022 + I-142 で解消。skip 解除)
        "string-methods",
        "type-assertion",
        // (void-type: Step 3 skip 解除)
        // (async-class-method: P4.2 で Promise unwrap 実装完了。skip 解除)
        // (callable-interface, call-signature-rest, interface-mixed,
        // callable-interface-param-rename: P8.2 統合チェックポイントで復帰完了)
        // callable-interface-generic-arity-mismatch: error-case fixture (INV-4)
        "callable-interface-generic-arity-mismatch",
    ];

    let mut entries: Vec<_> = fs::read_dir(fixture_dir)
        .expect("failed to read fixtures directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| {
                    n.ends_with(".input.ts")
                        && !skip_compile_with_builtins.iter().any(|s| n.starts_with(s))
                })
        })
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let fixture_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("invalid fixture filename");

        let ts_source = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("failed to read fixture: {}", path.display()));
        let (rs_source, _unsupported) = transpile_with_builtins(&ts_source)
            .unwrap_or_else(|_| panic!("failed to transpile fixture: {}", path.display()));

        assert_compiles(&rs_source, fixture_name);
        fixture_count += 1;
    }

    assert!(
        fixture_count > 0,
        "no fixtures found in {fixture_dir} — test is vacuously passing"
    );
}

/// Compiles a directory of TS files as a multi-module Rust project.
///
/// All `.ts` files in the directory are transpiled with a shared TypeRegistry.
/// `main.ts` → `src/lib.rs`, other files → `src/<name>.rs` with `mod` declarations.
///
/// Caller must hold `COMPILE_LOCK` before calling this function.
fn assert_compiles_directory(dir: &str, fixture_name: &str) {
    // Collect all .ts files
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read dir {dir}: {e}"))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ts"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    // Build TranspileInput with all files
    let files: Vec<(std::path::PathBuf, String)> = entries
        .iter()
        .map(|e| {
            let source = fs::read_to_string(e.path())
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", e.path().display()));
            (e.path(), source)
        })
        .collect();

    let input = TranspileInput {
        files,
        builtin_types: None,
        base_synthetic: None,
        module_resolver: Box::new(TrivialResolver),
    };
    let output = ts_to_rs::pipeline::transpile_pipeline(input)
        .unwrap_or_else(|e| panic!("transpile_pipeline failed for '{fixture_name}': {e}"));

    let mut mod_names: Vec<String> = Vec::new();
    let mut mod_guards: Vec<TempFile> = Vec::new();
    let mut lib_rs = String::new();

    for file_output in &output.files {
        let stem = file_output
            .path
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        // マルチファイルテストでは同一ディレクトリ内の use は保持し、
        // crate パス部分だけを短縮する（`use crate::long::path::module::Name` → `use module::Name`）
        let rs_source = simplify_use_statements(&file_output.rust_source);

        if stem == "main" {
            lib_rs = rs_source;
        } else {
            let mod_path = format!("{COMPILE_CHECK_DIR}/src/{stem}.rs");
            mod_guards.push(TempFile::new(mod_path, &rs_source));
            mod_names.push(stem);
        }
    }

    // Build lib.rs with mod declarations and prelude
    let mod_decls: String = mod_names.iter().map(|m| format!("mod {m};\n")).collect();
    let full_source = format!(
        "{COMPILE_TEST_LINT_PRELUDE}\
         use serde::{{Serialize, Deserialize}};\n\
         {mod_decls}{lib_rs}"
    );

    let lib_path = format!("{COMPILE_CHECK_DIR}/src/lib.rs");
    fs::write(&lib_path, &full_source)
        .unwrap_or_else(|e| panic!("failed to write {lib_path}: {e}"));

    let cmd_output = Command::new("cargo")
        .args(["check", "--message-format=short"])
        .current_dir(COMPILE_CHECK_DIR)
        .output()
        .expect("failed to execute cargo check");

    // Drop module guards before assert to clean up even on failure
    drop(mod_guards);

    assert!(
        cmd_output.status.success(),
        "cargo check failed for multi-file fixture '{fixture_name}':\n{}\ngenerated lib.rs:\n{full_source}",
        String::from_utf8_lossy(&cmd_output.stderr)
    );
}

#[test]
fn test_multi_file_fixtures_compile() {
    let _lock = COMPILE_LOCK.lock().unwrap();

    let multi_dir = "tests/fixtures/multi";
    let Ok(entries) = fs::read_dir(multi_dir) else {
        return; // No multi-file fixtures yet
    };

    let mut dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    dirs.sort_by_key(|e| e.file_name());

    for dir_entry in &dirs {
        let dir_name = dir_entry.file_name().to_string_lossy().into_owned();
        let dir_path = dir_entry.path().to_string_lossy().into_owned();
        assert_compiles_directory(&dir_path, &dir_name);
    }

    assert!(
        !dirs.is_empty(),
        "no multi-file fixtures found in {multi_dir}"
    );
}
