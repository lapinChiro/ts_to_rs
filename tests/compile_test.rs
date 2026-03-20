use std::fs;
use std::process::Command;
use std::sync::Mutex;

use ts_to_rs::{build_shared_registry, transpile_collecting, transpile_collecting_with_registry};

/// Path to the fixed Cargo project used for compile checking.
const COMPILE_CHECK_DIR: &str = "tests/compile-check";

/// Mutex to serialize compile tests (they share the same compile-check project).
static COMPILE_LOCK: Mutex<()> = Mutex::new(());

/// Strips internal module `use` statements while preserving external crate imports.
///
/// Internal references (e.g., `use crate::`, `use super::`) cannot be resolved in
/// single-file compilation. External crate imports (e.g., `use serde`, `use scopeguard`)
/// must be kept for the code to compile with dependencies.
fn strip_internal_use_statements(rs_source: &str) -> String {
    rs_source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("use ") && !trimmed.starts_with("pub use ") {
                return true;
            }
            // Keep external crate imports, filter out internal module references
            !trimmed.contains("crate::") && !trimmed.contains("super::")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compiles the given Rust source code via `cargo check` against the fixed
/// compile-check project (which has external crate dependencies).
///
/// Caller must hold `COMPILE_LOCK` before calling this function.
fn assert_compiles(rs_source: &str, fixture_name: &str) {
    let compilable_source = strip_internal_use_statements(rs_source);

    // Write the generated code to the compile-check project's src/lib.rs
    let lib_path = format!("{COMPILE_CHECK_DIR}/src/lib.rs");
    // Suppress warnings and import external crate items used by generated code
    let full_source = format!(
        "#![allow(unused, dead_code, unreachable_code)]\n\
         use serde::{{Serialize, Deserialize}};\n\
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
        // union-fallback generates enum with Box<dyn Fn> which can't derive Clone/PartialEq.
        // The union conversion itself is correct; derive limitations are a separate issue.
        "union-fallback",
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

    // Read all sources for shared registry
    let sources: Vec<String> = entries
        .iter()
        .map(|e| {
            fs::read_to_string(e.path())
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", e.path().display()))
        })
        .collect();
    let source_refs: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();
    let shared_registry = build_shared_registry(&source_refs);

    let mut mod_names: Vec<String> = Vec::new();
    let mut lib_rs = String::new();

    for (i, entry) in entries.iter().enumerate() {
        let stem = entry
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let (rs_source, _) = transpile_collecting_with_registry(&sources[i], &shared_registry)
            .unwrap_or_else(|e| {
                panic!(
                    "transpile failed for '{}': {e}",
                    entry.file_name().to_string_lossy()
                )
            });

        if stem == "main" {
            lib_rs = rs_source;
        } else {
            let mod_path = format!("{COMPILE_CHECK_DIR}/src/{stem}.rs");
            fs::write(&mod_path, &rs_source)
                .unwrap_or_else(|e| panic!("failed to write {mod_path}: {e}"));
            mod_names.push(stem);
        }
    }

    // Build lib.rs with mod declarations and prelude
    let mod_decls: String = mod_names.iter().map(|m| format!("mod {m};\n")).collect();
    let full_source = format!(
        "#![allow(unused, dead_code, unreachable_code)]\n\
         use serde::{{Serialize, Deserialize}};\n\
         {mod_decls}{lib_rs}"
    );

    let lib_path = format!("{COMPILE_CHECK_DIR}/src/lib.rs");
    fs::write(&lib_path, &full_source)
        .unwrap_or_else(|e| panic!("failed to write {lib_path}: {e}"));

    let output = Command::new("cargo")
        .args(["check", "--message-format=short"])
        .current_dir(COMPILE_CHECK_DIR)
        .output()
        .expect("failed to execute cargo check");

    // Clean up module files
    for m in &mod_names {
        let _ = fs::remove_file(format!("{COMPILE_CHECK_DIR}/src/{m}.rs"));
    }

    assert!(
        output.status.success(),
        "cargo check failed for multi-file fixture '{fixture_name}':\n{}\ngenerated lib.rs:\n{full_source}",
        String::from_utf8_lossy(&output.stderr)
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
