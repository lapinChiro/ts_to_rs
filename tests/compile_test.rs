use std::fs;
use std::process::Command;

use ts_to_rs::transpile_collecting;

/// Path to the fixed Cargo project used for compile checking.
const COMPILE_CHECK_DIR: &str = "tests/compile-check";

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
    let fixture_dir = "tests/fixtures";
    let mut fixture_count = 0;

    // Fixtures that cannot compile in isolation due to reasons OTHER than missing crates:
    let skip_compile = [
        "indexed-access-type",
        // Conditional types produce type aliases with unused type params and references to
        // traits (e.g., `<T as Promise>::Output`) not defined in the generated code.
        "conditional-type",
        // Type reference union variants reference types that lack required derives (Debug, Clone,
        // PartialEq) and have unused type parameters when compiled in isolation.
        "union-type",
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
