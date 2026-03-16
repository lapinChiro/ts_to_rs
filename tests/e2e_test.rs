use std::fs;
use std::process::Command;
use std::sync::Mutex;

use ts_to_rs::transpile;

/// Path to the E2E scripts directory.
const SCRIPTS_DIR: &str = "tests/e2e/scripts";

/// Path to the Rust runner Cargo project.
const RUST_RUNNER_DIR: &str = "tests/e2e/rust-runner";

/// Path to the locally-installed tsx binary.
const TSX_BIN: &str = "tests/e2e/node_modules/.bin/tsx";

/// Mutex to serialize E2E tests (they share the same rust-runner project).
static E2E_LOCK: Mutex<()> = Mutex::new(());

/// Strips internal module `use` statements while preserving external crate imports.
fn strip_internal_use_statements(rs_source: &str) -> String {
    rs_source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("use ") && !trimmed.starts_with("pub use ") {
                return true;
            }
            !trimmed.contains("crate::") && !trimmed.contains("super::")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Runs an E2E test for the given script name.
///
/// 1. Reads the TS script from `tests/e2e/scripts/{name}.ts`
/// 2. Transpiles TS → Rust
/// 3. Writes Rust to `tests/e2e/rust-runner/src/main.rs` and runs `cargo run`
/// 4. Runs TS via locally-installed `tsx`
/// 5. Compares stdout line-by-line
fn run_e2e_test(name: &str) {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let script_path = format!("{SCRIPTS_DIR}/{name}.ts");
    let ts_source = fs::read_to_string(&script_path)
        .unwrap_or_else(|e| panic!("failed to read {script_path}: {e}"));

    // Step 1: Transpile TS → Rust directly. Scripts must define `function main()`.
    // In Rust, `fn main()` becomes the entry point automatically.
    let rs_source =
        transpile(&ts_source).unwrap_or_else(|e| panic!("transpile failed for '{name}': {e}"));

    let rs_source = strip_internal_use_statements(&rs_source);

    // Step 2: Write Rust source and run
    let main_path = format!("{RUST_RUNNER_DIR}/src/main.rs");
    fs::write(&main_path, &rs_source)
        .unwrap_or_else(|e| panic!("failed to write {main_path}: {e}"));

    let rust_output = Command::new("cargo")
        .args(["run", "--quiet"])
        .current_dir(RUST_RUNNER_DIR)
        .output()
        .expect("failed to execute cargo run");

    assert!(
        rust_output.status.success(),
        "cargo run failed for '{name}':\nstderr: {}\ngenerated Rust:\n{}",
        String::from_utf8_lossy(&rust_output.stderr),
        rs_source
    );

    let rust_stdout = String::from_utf8_lossy(&rust_output.stdout);

    // Step 3: Run TS via locally-installed tsx (append main() call for execution)
    let ts_exec_path = format!("{SCRIPTS_DIR}/{name}_exec.ts");
    let ts_exec_source = format!("{ts_source}\nmain();\n");
    fs::write(&ts_exec_path, &ts_exec_source)
        .unwrap_or_else(|e| panic!("failed to write {ts_exec_path}: {e}"));

    let ts_output = Command::new(TSX_BIN)
        .arg(&ts_exec_path)
        .output()
        .expect("failed to execute tsx — run `npm install` in tests/e2e/");

    // Clean up temp file
    let _ = fs::remove_file(&ts_exec_path);

    assert!(
        ts_output.status.success(),
        "npx tsx failed for '{name}':\n{}",
        String::from_utf8_lossy(&ts_output.stderr)
    );

    let ts_stdout = String::from_utf8_lossy(&ts_output.stdout);

    // Step 4: Compare stdout line-by-line
    let rust_lines: Vec<&str> = rust_stdout.lines().collect();
    let ts_lines: Vec<&str> = ts_stdout.lines().collect();

    if rust_lines != ts_lines {
        let mut diff = String::new();
        let max_lines = rust_lines.len().max(ts_lines.len());
        for i in 0..max_lines {
            let rs_line = rust_lines.get(i).unwrap_or(&"<missing>");
            let ts_line = ts_lines.get(i).unwrap_or(&"<missing>");
            if rs_line != ts_line {
                diff.push_str(&format!(
                    "  line {}: TS={:?}  Rust={:?}\n",
                    i + 1,
                    ts_line,
                    rs_line
                ));
            }
        }
        panic!(
            "stdout mismatch for '{name}':\n{diff}\nTS output:\n{ts_stdout}\nRust output:\n{rust_stdout}\nGenerated Rust:\n{rs_source}"
        );
    }
}

#[test]
fn test_e2e_hello_ts_rust_stdout_match() {
    run_e2e_test("hello");
}

#[test]
fn test_e2e_arithmetic_ts_rust_stdout_match() {
    run_e2e_test("arithmetic");
}

#[test]
fn test_e2e_string_ops_ts_rust_stdout_match() {
    run_e2e_test("string_ops");
}

#[test]
fn test_e2e_array_ops_ts_rust_stdout_match() {
    run_e2e_test("array_ops");
}

#[test]
fn test_e2e_control_flow_ts_rust_stdout_match() {
    run_e2e_test("control_flow");
}

#[test]
fn test_e2e_loops_ts_rust_stdout_match() {
    run_e2e_test("loops");
}

#[test]
fn test_e2e_functions_ts_rust_stdout_match() {
    run_e2e_test("functions");
}

#[test]
fn test_e2e_error_handling_ts_rust_stdout_match() {
    run_e2e_test("error_handling");
}

#[test]
fn test_e2e_classes_ts_rust_stdout_match() {
    run_e2e_test("classes");
}
