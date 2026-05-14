use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn cargo_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ts_to_rs"))
}

#[test]
fn test_cli_report_unsupported_outputs_json() {
    let tmp = TempDir::new().unwrap();
    let output_path = tmp.path().join("output.rs");

    let output = cargo_bin()
        .args([
            "tests/fixtures/unsupported-syntax.input.ts",
            "--output",
            output_path.to_str().unwrap(),
            "--report-unsupported",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "should succeed with --report-unsupported: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout should be valid JSON: {e}\nstdout: {stdout}"));
    assert!(!parsed.is_empty(), "should report unsupported syntax items");

    // Verify each entry has kind and location
    for entry in &parsed {
        assert!(entry.get("kind").is_some(), "entry should have 'kind'");
        assert!(
            entry.get("location").is_some(),
            "entry should have 'location'"
        );
    }

    // Rust output file should still be written
    assert!(output_path.exists(), "output .rs file should be written");
}

#[test]
fn test_cli_report_unsupported_preserves_file_line_col_location_contract() {
    let tmp = TempDir::new().unwrap();
    let input_path = tmp.path().join("location-check.ts");
    let output_path = tmp.path().join("location-check.rs");
    fs::write(&input_path, "\nexport default 42;\n").unwrap();

    let output = cargo_bin()
        .args([
            "--output",
            output_path.to_str().unwrap(),
            "--report-unsupported",
        ])
        .arg(&input_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "should succeed with exact location reporting: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));
    assert_eq!(parsed.len(), 1, "expected exactly one unsupported item");
    let expected_location = format!("{}:2:1", input_path.display());
    assert_eq!(
        parsed[0].get("location").and_then(|v| v.as_str()),
        Some(expected_location.as_str())
    );
    assert!(output_path.exists(), "output file should still be written");
}

#[test]
fn test_cli_report_unsupported_preserves_utf8_byte_based_location_contract() {
    let tmp = TempDir::new().unwrap();
    let input_path = tmp.path().join("utf8-location-check.ts");
    let output_path = tmp.path().join("utf8-location-check.rs");
    fs::write(&input_path, "const café = 1; export default 42;\n").unwrap();

    let output = cargo_bin()
        .args([
            "--output",
            output_path.to_str().unwrap(),
            "--report-unsupported",
        ])
        .arg(&input_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "should succeed with UTF-8 location reporting: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));
    assert_eq!(parsed.len(), 1, "expected exactly one unsupported item");
    let expected_location = format!("{}:1:18", input_path.display());
    assert_eq!(
        parsed[0].get("location").and_then(|v| v.as_str()),
        Some(expected_location.as_str())
    );
    assert!(output_path.exists(), "output file should still be written");
}

#[test]
fn test_cli_report_unsupported_all_supported_outputs_empty_array() {
    let tmp = TempDir::new().unwrap();
    let output_path = tmp.path().join("output.rs");

    let output = cargo_bin()
        .args([
            "tests/fixtures/basic-types.input.ts",
            "--output",
            output_path.to_str().unwrap(),
            "--report-unsupported",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout should be valid JSON: {e}\nstdout: {stdout}"));
    assert!(
        parsed.is_empty(),
        "should be empty array for fully supported file"
    );
}

#[test]
fn test_cli_default_errors_on_unsupported() {
    let tmp = TempDir::new().unwrap();
    let output_path = tmp.path().join("output.rs");

    let output = cargo_bin()
        .args([
            "tests/fixtures/unsupported-syntax.input.ts",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "should fail without --report-unsupported when unsupported syntax exists"
    );
}

#[test]
fn test_cli_help_mentions_core_flags_and_subcommand() {
    let output = cargo_bin().arg("--help").output().unwrap();

    assert!(
        output.status.success(),
        "--help should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    for needle in [
        "--report-unsupported",
        "--no-builtin-types",
        "resolve-types",
        "Transpile TypeScript source code to Rust",
    ] {
        assert!(
            stdout.contains(needle),
            "help output should contain `{needle}`, got:\n{stdout}"
        );
    }
}

#[test]
fn test_cli_without_input_reports_missing_input_path() {
    let output = cargo_bin().output().unwrap();

    assert!(
        !output.status.success(),
        "invoking CLI without input or subcommand should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("input path is required for transpilation"),
        "expected missing-input diagnostic, got:\n{stderr}"
    );
}

#[test]
fn test_cli_single_file_default_output_writes_input_rs() {
    let tmp = TempDir::new().unwrap();
    let input_path = tmp.path().join("sample.input.ts");
    fs::write(
        &input_path,
        "interface User { name: string; }\nfunction greet(user: User): string { return user.name; }\n",
    )
    .unwrap();

    let output = cargo_bin().arg(&input_path).output().unwrap();

    assert!(
        output.status.success(),
        "single-file transpilation should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected_output = input_path.with_extension("rs");
    assert!(
        expected_output.exists(),
        "default single-file output should be written to {}",
        expected_output.display()
    );
}

#[test]
fn test_cli_directory_default_output_writes_input_dir_rs() {
    let tmp = TempDir::new().unwrap();
    let input_dir = tmp.path().join("src");
    fs::create_dir(&input_dir).unwrap();
    fs::write(
        input_dir.join("main.ts"),
        "function greet(name: string): string { return name; }\n",
    )
    .unwrap();

    let output = cargo_bin().arg(&input_dir).output().unwrap();

    assert!(
        output.status.success(),
        "directory transpilation should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output_dir = tmp.path().join("src_rs");
    assert!(
        output_dir.exists(),
        "default directory output should be written to {}",
        output_dir.display()
    );
    assert!(
        output_dir.join("main.rs").exists(),
        "converted Rust file should exist in {}",
        output_dir.display()
    );
}

#[test]
fn test_cli_directory_report_unsupported_sorts_files_and_preserves_locations() {
    let tmp = TempDir::new().unwrap();
    let input_dir = tmp.path().join("src");
    fs::create_dir(&input_dir).unwrap();
    fs::write(input_dir.join("b.ts"), "export default 42;\n").unwrap();
    fs::write(input_dir.join("a.ts"), "export default 7;\n").unwrap();
    let output_dir = tmp.path().join("out_rs");

    let output = cargo_bin()
        .args([
            input_dir.to_str().unwrap(),
            "--output",
            output_dir.to_str().unwrap(),
            "--report-unsupported",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "directory report-unsupported mode should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));
    assert_eq!(parsed.len(), 2, "expected one unsupported entry per file");

    let a_location = format!("{}:1:1", input_dir.join("a.ts").display());
    let b_location = format!("{}:1:1", input_dir.join("b.ts").display());
    assert_eq!(
        parsed[0].get("location").and_then(|v| v.as_str()),
        Some(a_location.as_str()),
        "entries should follow collect_ts_files() sorted order"
    );
    assert_eq!(
        parsed[1].get("location").and_then(|v| v.as_str()),
        Some(b_location.as_str()),
        "entries should follow collect_ts_files() sorted order"
    );
    assert!(
        output_dir.join("a.rs").exists(),
        "a.ts should be transpiled"
    );
    assert!(
        output_dir.join("b.rs").exists(),
        "b.ts should be transpiled"
    );
}

#[test]
fn test_cli_resolve_types_missing_tsconfig_reports_not_found() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("missing-tsconfig.json");

    let output = cargo_bin()
        .args(["resolve-types", "--tsconfig"])
        .arg(&missing)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "resolve-types with missing tsconfig should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("tsconfig not found"),
        "expected missing-tsconfig diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains(missing.to_string_lossy().as_ref()),
        "diagnostic should include missing path, got:\n{stderr}"
    );
}
