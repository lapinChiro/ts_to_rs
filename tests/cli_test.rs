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
