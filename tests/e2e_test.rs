use std::fs;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use ts_to_rs::transpile;

#[path = "test_helpers.rs"]
mod test_helpers;
use test_helpers::{strip_internal_use_statements, TempFile};

/// Path to the E2E scripts directory.
const SCRIPTS_DIR: &str = "tests/e2e/scripts";

/// Path to the Rust runner Cargo project.
const RUST_RUNNER_DIR: &str = "tests/e2e/rust-runner";

/// Path to the locally-installed tsx binary.
const TSX_BIN: &str = "tests/e2e/node_modules/.bin/tsx";

/// Mutex to serialize E2E tests (they share the same rust-runner project).
static E2E_LOCK: Mutex<()> = Mutex::new(());

/// Tracks the last mtime set on rust-runner source files.
///
/// Cargo detects source changes via mtime comparison. On WSL2's ext4, rapid
/// consecutive writes can share the same mtime (nanosecond resolution but
/// batched updates), causing cargo to skip rebuilds. This tracks the last
/// mtime we set, ensuring each write gets a strictly later mtime.
static LAST_MTIME: Mutex<Option<SystemTime>> = Mutex::new(None);

/// Writes content to a file and ensures its mtime is strictly newer than any
/// previous call, so cargo's fingerprint check always detects the change.
fn write_with_advancing_mtime(path: &str, content: &str) {
    fs::write(path, content).unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
    let mut last = LAST_MTIME.lock().unwrap();
    let prev = last.unwrap_or(SystemTime::UNIX_EPOCH);
    // Use the later of "now" or "previous mtime + 1s" to guarantee monotonic increase
    // without accumulating unbounded future offsets.
    let next = SystemTime::now().max(prev + Duration::from_secs(1));
    *last = Some(next);
    let file = fs::File::options()
        .write(true)
        .open(path)
        .unwrap_or_else(|e| panic!("failed to open {path} for mtime update: {e}"));
    file.set_modified(next)
        .unwrap_or_else(|e| panic!("failed to set mtime on {path}: {e}"));
}

/// Result of running a single E2E script on both TS and Rust sides.
struct E2eResult {
    rs_source: String,
    rust_stdout: String,
    rust_stderr: String,
    ts_stdout: String,
    ts_stderr: String,
}

/// Options for customizing E2E test execution.
#[derive(Default)]
struct E2eOptions<'a> {
    /// Data to pipe to stdin (None = no stdin)
    stdin: Option<&'a str>,
    /// Extra environment variables to set for both TS and Rust
    env: Vec<(&'a str, &'a str)>,
}

/// Transpiles and executes a single TS script, returning both TS and Rust outputs.
fn execute_e2e(name: &str) -> E2eResult {
    execute_e2e_with_options(name, &E2eOptions::default())
}

/// Transpiles and executes a single TS script with custom options.
fn execute_e2e_with_options(name: &str, opts: &E2eOptions) -> E2eResult {
    let script_path = format!("{SCRIPTS_DIR}/{name}.ts");
    let ts_source = fs::read_to_string(&script_path)
        .unwrap_or_else(|e| panic!("failed to read {script_path}: {e}"));

    // Step 1: Transpile TS → Rust
    let rs_source =
        transpile(&ts_source).unwrap_or_else(|e| panic!("transpile failed for '{name}': {e}"));
    let rs_source = strip_internal_use_statements(&rs_source);

    // Step 2: Write Rust source and run
    let main_path = format!("{RUST_RUNNER_DIR}/src/main.rs");
    write_with_advancing_mtime(&main_path, &rs_source);

    let mut rust_cmd = Command::new("cargo");
    rust_cmd
        .args(["run", "--quiet"])
        .current_dir(RUST_RUNNER_DIR);
    for (k, v) in &opts.env {
        rust_cmd.env(k, v);
    }
    let rust_output = if let Some(stdin_data) = opts.stdin {
        rust_cmd.stdin(std::process::Stdio::piped());
        let mut child = rust_cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn cargo run");
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin_data.as_bytes())
            .expect("failed to write stdin");
        child
            .wait_with_output()
            .expect("failed to wait for cargo run")
    } else {
        rust_cmd.output().expect("failed to execute cargo run")
    };

    assert!(
        rust_output.status.success(),
        "cargo run failed for '{name}':\nstderr: {}\ngenerated Rust:\n{}",
        String::from_utf8_lossy(&rust_output.stderr),
        rs_source
    );

    // Step 3: Run TS via locally-installed tsx
    let ts_exec_source = format!("{ts_source}\nmain();\n");
    let ts_exec_guard = TempFile::new(format!("{SCRIPTS_DIR}/{name}_exec.ts"), &ts_exec_source);

    let mut ts_cmd = Command::new(TSX_BIN);
    ts_cmd.arg(ts_exec_guard.path());
    for (k, v) in &opts.env {
        ts_cmd.env(k, v);
    }
    let ts_output = if let Some(stdin_data) = opts.stdin {
        ts_cmd.stdin(std::process::Stdio::piped());
        let mut child = ts_cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn tsx");
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin_data.as_bytes())
            .expect("failed to write stdin");
        child.wait_with_output().expect("failed to wait for tsx")
    } else {
        ts_cmd
            .output()
            .expect("failed to execute tsx — run `npm install` in tests/e2e/")
    };

    assert!(
        ts_output.status.success(),
        "npx tsx failed for '{name}':\n{}",
        String::from_utf8_lossy(&ts_output.stderr)
    );

    E2eResult {
        rs_source,
        rust_stdout: String::from_utf8_lossy(&rust_output.stdout).into_owned(),
        rust_stderr: String::from_utf8_lossy(&rust_output.stderr).into_owned(),
        ts_stdout: String::from_utf8_lossy(&ts_output.stdout).into_owned(),
        ts_stderr: String::from_utf8_lossy(&ts_output.stderr).into_owned(),
    }
}

/// Compares two outputs line-by-line, panicking with a diff on mismatch.
fn assert_lines_match(
    name: &str,
    stream: &str,
    ts_output: &str,
    rust_output: &str,
    rs_source: &str,
) {
    let rust_lines: Vec<&str> = rust_output.lines().collect();
    let ts_lines: Vec<&str> = ts_output.lines().collect();

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
            "{stream} mismatch for '{name}':\n{diff}\nTS {stream}:\n{ts_output}\nRust {stream}:\n{rust_output}\nGenerated Rust:\n{rs_source}"
        );
    }
}

/// Runs an E2E test comparing stdout only (existing behavior).
fn run_e2e_test(name: &str) {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let result = execute_e2e(name);
    assert_lines_match(
        name,
        "stdout",
        &result.ts_stdout,
        &result.rust_stdout,
        &result.rs_source,
    );
}

/// Runs an E2E test with stdin input.
fn run_e2e_test_with_stdin(name: &str, stdin_input: &str) {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let opts = E2eOptions {
        stdin: Some(stdin_input),
        ..Default::default()
    };
    let result = execute_e2e_with_options(name, &opts);
    assert_lines_match(
        name,
        "stdout",
        &result.ts_stdout,
        &result.rust_stdout,
        &result.rs_source,
    );
}

/// Runs an E2E test with extra environment variables.
fn run_e2e_test_with_env(name: &str, env: &[(&str, &str)]) {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let opts = E2eOptions {
        env: env.to_vec(),
        ..Default::default()
    };
    let result = execute_e2e_with_options(name, &opts);
    assert_lines_match(
        name,
        "stdout",
        &result.ts_stdout,
        &result.rust_stdout,
        &result.rs_source,
    );
}

/// Runs a multi-file E2E test.
///
/// Transpiles all `.ts` files in `tests/e2e/scripts/multi/{name}/`,
/// writes them to `tests/e2e/rust-runner/src/`, and compares stdout.
/// `main.ts` → `src/main.rs`, other files → `src/<name>.rs` with `mod` declarations.
fn run_e2e_multi_file_test(name: &str) {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = format!("{SCRIPTS_DIR}/multi/{name}");

    // Collect all .ts files
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("failed to read dir {dir}: {e}"))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ts"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut mod_names: Vec<String> = Vec::new();
    let mut mod_guards: Vec<TempFile> = Vec::new();
    let mut main_rs = String::new();

    for entry in &entries {
        let file_name = entry.file_name();
        let stem = entry
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let ts_source = fs::read_to_string(entry.path())
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file_name.to_string_lossy()));

        let rs_source = transpile(&ts_source).unwrap_or_else(|e| {
            panic!(
                "transpile failed for '{}': {e}",
                file_name.to_string_lossy()
            )
        });
        // Multi-file tests need internal use statements for cross-module references
        // (unlike single-file tests where they are noise)

        if stem == "main" {
            main_rs = rs_source;
        } else {
            let mod_path = format!("{RUST_RUNNER_DIR}/src/{stem}.rs");
            write_with_advancing_mtime(&mod_path, &rs_source);
            mod_guards.push(TempFile::guard(mod_path));
            mod_names.push(stem);
        }
    }

    // Prepend mod declarations to main.rs
    let mod_decls: String = mod_names.iter().map(|m| format!("mod {m};\n")).collect();
    let full_main = format!("{mod_decls}{main_rs}");

    let main_path = format!("{RUST_RUNNER_DIR}/src/main.rs");
    write_with_advancing_mtime(&main_path, &full_main);

    // Run Rust
    let rust_output = Command::new("cargo")
        .args(["run", "--quiet"])
        .current_dir(RUST_RUNNER_DIR)
        .output()
        .expect("failed to execute cargo run");

    // Drop module guards before assert to clean up even on failure
    drop(mod_guards);

    assert!(
        rust_output.status.success(),
        "cargo run failed for multi-file '{name}':\nstderr: {}\ngenerated main.rs:\n{}",
        String::from_utf8_lossy(&rust_output.stderr),
        full_main
    );

    let rust_stdout = String::from_utf8_lossy(&rust_output.stdout);

    // Run TS (tsx resolves relative imports automatically)
    let main_ts = fs::read_to_string(format!("{dir}/main.ts")).unwrap();
    let ts_exec_guard = TempFile::new(
        format!("{dir}/main_exec.ts"),
        &format!("{main_ts}\nmain();\n"),
    );

    let ts_output = Command::new(TSX_BIN)
        .arg(ts_exec_guard.path())
        .output()
        .expect("failed to execute tsx");

    assert!(
        ts_output.status.success(),
        "tsx failed for multi-file '{name}':\n{}",
        String::from_utf8_lossy(&ts_output.stderr)
    );

    let ts_stdout = String::from_utf8_lossy(&ts_output.stdout);
    assert_lines_match(name, "stdout", &ts_stdout, &rust_stdout, &full_main);
}

/// Runs an E2E test comparing both stdout and stderr.
fn run_e2e_test_with_stderr(name: &str) {
    let _guard = E2E_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let result = execute_e2e(name);
    assert_lines_match(
        name,
        "stdout",
        &result.ts_stdout,
        &result.rust_stdout,
        &result.rs_source,
    );
    assert_lines_match(
        name,
        "stderr",
        &result.ts_stderr,
        &result.rust_stderr,
        &result.rs_source,
    );
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

#[test]
fn test_e2e_switch_match_ts_rust_stdout_match() {
    run_e2e_test("switch_match");
}

#[test]
fn test_e2e_loop_control_ts_rust_stdout_match() {
    run_e2e_test("loop_control");
}

#[test]
fn test_e2e_enum_basic_ts_rust_stdout_match() {
    run_e2e_test("enum_basic");
}

#[test]
fn test_e2e_optional_chaining_ts_rust_stdout_match() {
    run_e2e_test("optional_chaining");
}

#[test]
fn test_e2e_nullish_coalescing_ts_rust_stdout_match() {
    run_e2e_test("nullish_coalescing");
}

#[test]
fn test_e2e_null_undefined_value_ts_rust_stdout_match() {
    // I-379: `Expr::BuiltinVariantValue(None)` 構造化の runtime semantics 等価性検証。
    run_e2e_test("null_undefined_value");
}

#[test]
fn test_e2e_closures_ts_rust_stdout_match() {
    run_e2e_test("closures");
}

#[test]
fn test_e2e_default_params_ts_rust_stdout_match() {
    run_e2e_test("default_params");
}

#[test]
fn test_e2e_destructuring_ts_rust_stdout_match() {
    run_e2e_test("destructuring");
}

#[test]
fn test_e2e_nested_rest_destructuring_ts_rust_stdout_match() {
    run_e2e_test("nested_rest_destructuring");
}

#[test]
fn test_e2e_spread_ops_ts_rust_stdout_match() {
    run_e2e_test("spread_ops");
}

#[test]
fn test_e2e_class_inheritance_ts_rust_stdout_match() {
    run_e2e_test("class_inheritance");
}

#[test]
fn test_e2e_generics_ts_rust_stdout_match() {
    run_e2e_test("generics");
}

#[test]
fn test_e2e_template_literals_ts_rust_stdout_match() {
    run_e2e_test("template_literals");
}

#[test]
fn test_e2e_array_methods_ts_rust_stdout_match() {
    run_e2e_test("array_methods");
}

#[test]
fn test_e2e_object_ops_ts_rust_stdout_match() {
    run_e2e_test("object_ops");
}

#[test]
fn test_e2e_advanced_classes_ts_rust_stdout_match() {
    run_e2e_test("advanced_classes");
}

#[test]
fn test_e2e_number_api_ts_rust_stdout_match() {
    run_e2e_test("number_api");
}

#[test]
fn test_e2e_type_system_ts_rust_stdout_match() {
    run_e2e_test("type_system");
}

#[test]
fn test_e2e_nested_logic_ts_rust_stdout_match() {
    run_e2e_test("nested_logic");
}

#[test]
fn test_e2e_intersection_type_ts_rust_stdout_match() {
    run_e2e_test("intersection_type");
}

#[test]
fn test_e2e_switch_nonliteral_ts_rust_stdout_match() {
    run_e2e_test("switch_nonliteral");
}

#[test]
fn test_e2e_const_mutation_ts_rust_stdout_match() {
    run_e2e_test("const_mutation");
}

#[test]
fn test_e2e_to_string_calls_ts_rust_stdout_match() {
    run_e2e_test("to_string_calls");
}

#[test]
fn test_e2e_string_concat_ts_rust_stdout_match() {
    run_e2e_test("string_concat");
}

#[test]
fn test_e2e_local_type_decl_ts_rust_stdout_match() {
    run_e2e_test("local_type_decl");
}

#[test]
fn test_e2e_console_error_ts_rust_stdout_and_stderr_match() {
    run_e2e_test_with_stderr("console_error");
}

#[test]
fn test_e2e_method_args_ts_rust_stdout_match() {
    run_e2e_test("method_args");
}

#[test]
fn test_e2e_console_display_ts_rust_stdout_match() {
    run_e2e_test("console_display");
}

#[test]
fn test_e2e_discriminated_union_ts_rust_stdout_match() {
    run_e2e_test("discriminated_union");
}

#[test]
fn test_e2e_string_literal_enum_ts_rust_stdout_match() {
    run_e2e_test("string_literal_enum");
}

#[test]
fn test_e2e_async_await_ts_rust_stdout_match() {
    run_e2e_test("async_await");
}

#[test]
fn test_e2e_type_infer_ts_rust_stdout_match() {
    run_e2e_test("type_infer");
}

#[test]
fn test_e2e_in_operator_ts_rust_stdout_match() {
    run_e2e_test("in_operator");
}

#[test]
fn test_e2e_conditional_assignment_ts_rust_stdout_match() {
    run_e2e_test("conditional_assignment");
}

#[test]
fn test_e2e_param_properties_ts_rust_stdout_match() {
    run_e2e_test("param_properties");
}

#[test]
fn test_e2e_multi_import_basic_ts_rust_stdout_match() {
    run_e2e_multi_file_test("import_basic");
}

#[test]
fn test_e2e_update_expr_ts_rust_stdout_match() {
    run_e2e_test("update_expr");
}

#[test]
fn test_e2e_var_type_arrow_ts_rust_stdout_match() {
    run_e2e_test("var_type_arrow");
}

#[test]
fn test_e2e_bitwise_ops_ts_rust_stdout_match() {
    run_e2e_test("bitwise_ops");
}

#[test]
fn test_e2e_rest_params_ts_rust_stdout_match() {
    run_e2e_test("rest_params");
}

#[test]
fn test_e2e_null_option_ts_rust_stdout_match() {
    run_e2e_test("null_option");
}

#[test]
fn test_e2e_tuple_literals_ts_rust_stdout_match() {
    run_e2e_test("tuple_literals");
}

#[test]
fn test_e2e_param_type_infer_ts_rust_stdout_match() {
    run_e2e_test("param_type_infer");
}

#[test]
fn test_e2e_update_expr_semantics_ts_rust_stdout_match() {
    run_e2e_test("update_expr_semantics");
}

#[test]
fn test_e2e_typeof_check_ts_rust_stdout_match() {
    run_e2e_test("typeof_check");
}

#[test]
fn test_e2e_regex_replace_ts_rust_stdout_match() {
    run_e2e_test("regex_replace");
}

#[test]
fn test_e2e_bigint_basics_ts_rust_stdout_match() {
    run_e2e_test("bigint_basics");
}

#[test]
fn test_e2e_iife_ts_rust_stdout_match() {
    run_e2e_test("iife");
}

#[test]
fn test_e2e_readonly_param_ts_rust_stdout_match() {
    run_e2e_test("readonly_param");
}

#[test]
fn test_e2e_stdin_echo_ts_rust_stdout_match() {
    run_e2e_test_with_stdin("stdin_echo", "hello\nworld\nfoo\n");
}

#[test]
fn test_e2e_interface_traits_ts_rust_stdout_match() {
    run_e2e_test("interface_traits");
}

#[test]
fn test_e2e_file_io_ts_rust_stdout_match() {
    let temp_dir = std::env::temp_dir().join("ts_to_rs_e2e_file_io");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("failed to create temp dir");
    let temp_dir_str = temp_dir.to_string_lossy().to_string();

    run_e2e_test_with_env("file_io", &[("TEST_DIR", &temp_dir_str)]);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_e2e_method_chain_ts_rust_stdout_match() {
    run_e2e_test("method_chain");
}

#[test]
fn test_e2e_object_literal_inference_ts_rust_stdout_match() {
    run_e2e_test("object_literal_inference");
}

#[test]
fn test_e2e_string_escape_ts_rust_stdout_match() {
    run_e2e_test("string_escape");
}

#[test]
fn test_e2e_object_spread_ts_rust_stdout_match() {
    run_e2e_test("object_spread");
}

#[test]
fn test_e2e_typeof_function_ts_rust_stdout_match() {
    run_e2e_test("typeof_function");
}

#[test]
fn test_e2e_typeof_const_ts_rust_stdout_match() {
    run_e2e_test("typeof_const");
}

#[test]
fn test_e2e_callback_type_inference_ts_rust_stdout_match() {
    run_e2e_test("callback_type_inference");
}

#[test]
fn test_e2e_do_while_ts_rust_stdout_match() {
    run_e2e_test("do_while");
}

#[test]
fn test_e2e_multivar_decl_ts_rust_stdout_match() {
    run_e2e_test("multivar_decl");
}

#[test]
fn test_e2e_unary_ops_ts_rust_stdout_match() {
    run_e2e_test("unary_ops");
}

#[test]
fn test_e2e_for_variations_ts_rust_stdout_match() {
    run_e2e_test("for_variations");
}

#[test]
fn test_e2e_type_alias_ts_rust_stdout_match() {
    run_e2e_test("type_alias");
}

#[test]
fn test_e2e_fn_expr_ts_rust_stdout_match() {
    run_e2e_test("fn_expr");
}

#[test]
fn test_e2e_class_methods_ts_rust_stdout_match() {
    run_e2e_test("class_methods");
}

#[test]
fn test_e2e_class_advanced_ts_rust_stdout_match() {
    run_e2e_test("class_advanced");
}

#[test]
fn test_e2e_interface_structs_ts_rust_stdout_match() {
    run_e2e_test("interface_structs");
}

#[test]
fn test_e2e_nullable_return_ts_rust_stdout_match() {
    run_e2e_test("nullable_return");
}

#[test]
fn test_e2e_explicit_type_args_ts_rust_stdout_match() {
    run_e2e_test("explicit_type_args");
}

#[test]
fn test_e2e_assignment_expected_ts_rust_stdout_match() {
    run_e2e_test("assignment_expected");
}

#[test]
fn test_e2e_interface_composition_ts_rust_stdout_match() {
    run_e2e_test("interface_composition");
}

#[test]
fn test_e2e_keyword_types_ts_rust_stdout_match() {
    run_e2e_test("keyword_types");
}

#[test]
fn test_e2e_mixed_features_ts_rust_stdout_match() {
    run_e2e_test("mixed_features");
}

#[test]
fn test_e2e_mutation_detection_ts_rust_stdout_match() {
    run_e2e_test("mutation_detection");
}

#[test]
fn test_e2e_narrowing_null_eq_ts_rust_stdout_match() {
    run_e2e_test("narrowing_null_eq");
}

#[test]
fn test_e2e_narrowing_compound_ternary_ts_rust_stdout_match() {
    run_e2e_test("narrowing_compound_ternary");
}

#[test]
fn test_e2e_to_string_method_ts_rust_stdout_match() {
    run_e2e_test("to_string_method");
}
