use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Condvar, Mutex, OnceLock};

use tempfile::{Builder, TempDir};
use ts_to_rs::transpile;

#[path = "test_helpers.rs"]
mod test_helpers;
use test_helpers::{strip_internal_use_statements, TempFile};

/// Path to the E2E scripts directory.
const SCRIPTS_DIR: &str = "tests/e2e/scripts";

/// Template project copied into per-runner temp directories.
const RUST_RUNNER_TEMPLATE_DIR: &str = "tests/e2e/rust-runner";

/// Path to the locally-installed tsx binary.
const TSX_BIN: &str = "tests/e2e/node_modules/.bin/tsx";

const E2E_RUNNER_POOL_ENV: &str = "TS_TO_RS_E2E_RUNNERS";
static E2E_RUNNER_POOL: OnceLock<E2eRunnerPool> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunnerInstancePaths {
    manifest_dir: String,
    target_dir: String,
}

fn resolve_runner_pool_size(
    available_override: Option<usize>,
    env_override: Option<&str>,
) -> usize {
    let available = available_override.unwrap_or(1).max(1);
    let default_size = available.clamp(1, 4);
    match env_override {
        Some(raw) => raw
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .map(|n| n.min(available))
            .unwrap_or(default_size),
        None => default_size,
    }
}

fn runner_instance_paths_for(base_dir: &str, slot: usize) -> RunnerInstancePaths {
    let slot_root = Path::new(base_dir).join(format!("runner-{slot}"));
    RunnerInstancePaths {
        manifest_dir: slot_root.join("rust-runner").display().to_string(),
        target_dir: slot_root.join("target").display().to_string(),
    }
}

fn cleanup_generated_runner_sources(src_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(src_dir)?;
    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_generated_rs = path.extension().is_some_and(|ext| ext == "rs")
            && path.file_name().is_some_and(|name| name != "main.rs");
        if is_generated_rs {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn copy_runner_template_file(relative_path: &str, destination_dir: &Path) -> std::io::Result<()> {
    let source = Path::new(RUST_RUNNER_TEMPLATE_DIR).join(relative_path);
    let destination = destination_dir.join(relative_path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination)?;
    Ok(())
}

struct E2eRunnerInstance {
    manifest_dir: PathBuf,
    target_dir: PathBuf,
}

impl E2eRunnerInstance {
    fn src_dir(&self) -> PathBuf {
        self.manifest_dir.join("src")
    }

    fn ts_exec_dir(&self) -> PathBuf {
        self.manifest_dir.join("ts-exec")
    }

    fn main_rs_path(&self) -> PathBuf {
        self.src_dir().join("main.rs")
    }

    fn reset_single_file_main(&self, rs_source: &str) {
        cleanup_generated_runner_sources(&self.src_dir())
            .unwrap_or_else(|e| panic!("failed to cleanup runner src dir: {e}"));
        fs::write(self.main_rs_path(), rs_source)
            .unwrap_or_else(|e| panic!("failed to write runner main.rs: {e}"));
    }

    fn reset_multi_file_sources(&self, main_rs: &str, modules: &[(String, String)]) {
        let src_dir = self.src_dir();
        cleanup_generated_runner_sources(&src_dir)
            .unwrap_or_else(|e| panic!("failed to cleanup runner src dir: {e}"));
        for (stem, source) in modules {
            let mod_path = src_dir.join(format!("{stem}.rs"));
            fs::write(&mod_path, source)
                .unwrap_or_else(|e| panic!("failed to write {}: {e}", mod_path.display()));
        }
        fs::write(self.main_rs_path(), main_rs)
            .unwrap_or_else(|e| panic!("failed to write runner main.rs: {e}"));
    }

    fn prepare_single_file_ts_exec(&self, source_path: &Path, ts_source: &str) -> TempFile {
        let source_dir = source_path
            .parent()
            .unwrap_or_else(|| Path::new(SCRIPTS_DIR));
        let exec_dir = self.reset_ts_exec_dir("single");
        copy_ts_fixture_files(source_dir, &exec_dir)
            .unwrap_or_else(|e| panic!("failed to copy TS fixture files: {e}"));
        let stem = source_path
            .file_stem()
            .unwrap_or_else(|| {
                panic!(
                    "TS fixture path has no file stem: {}",
                    source_path.display()
                )
            })
            .to_string_lossy();
        let exec_path = exec_dir.join(format!("{stem}_exec.ts"));
        TempFile::new(
            exec_path.display().to_string(),
            &format!("{ts_source}\nmain();\n"),
        )
    }

    fn prepare_multi_file_ts_exec(&self, source_dir: &Path, main_ts: &str) -> TempFile {
        let exec_dir = self.reset_ts_exec_dir("multi");
        copy_ts_fixture_files(source_dir, &exec_dir)
            .unwrap_or_else(|e| panic!("failed to copy TS fixture files: {e}"));
        let exec_path = exec_dir.join("main_exec.ts");
        TempFile::new(
            exec_path.display().to_string(),
            &format!("{main_ts}\nmain();\n"),
        )
    }

    fn reset_ts_exec_dir(&self, scope: &str) -> PathBuf {
        let exec_dir = self.ts_exec_dir().join(scope);
        if exec_dir.exists() {
            fs::remove_dir_all(&exec_dir).unwrap_or_else(|e| {
                panic!("failed to cleanup TS exec dir {}: {e}", exec_dir.display())
            });
        }
        fs::create_dir_all(&exec_dir)
            .unwrap_or_else(|e| panic!("failed to create TS exec dir {}: {e}", exec_dir.display()));
        exec_dir
    }
}

fn copy_ts_fixture_files(source_dir: &Path, destination_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination_dir)?;
    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let source = entry.path();
        if source.extension().is_none_or(|ext| ext != "ts") {
            continue;
        }
        let destination = destination_dir.join(entry.file_name());
        fs::copy(source, destination)?;
    }
    Ok(())
}

struct E2eRunnerPool {
    _root: TempDir,
    runners: Vec<E2eRunnerInstance>,
    available: Mutex<Vec<usize>>,
    condvar: Condvar,
}

impl E2eRunnerPool {
    fn global() -> &'static Self {
        E2E_RUNNER_POOL.get_or_init(Self::new)
    }

    fn new() -> Self {
        let root = Builder::new()
            .prefix("ts_to_rs_e2e_runner_pool_")
            .tempdir()
            .unwrap_or_else(|e| panic!("failed to create e2e runner pool dir: {e}"));
        let pool_size = resolve_runner_pool_size(
            std::thread::available_parallelism().ok().map(|n| n.get()),
            std::env::var(E2E_RUNNER_POOL_ENV).ok().as_deref(),
        );
        let mut runners = Vec::with_capacity(pool_size);
        let root_str = root.path().display().to_string();
        for slot in 0..pool_size {
            let paths = runner_instance_paths_for(&root_str, slot);
            let manifest_dir = PathBuf::from(&paths.manifest_dir);
            fs::create_dir_all(manifest_dir.join("src")).unwrap_or_else(|e| {
                panic!(
                    "failed to create runner manifest dir {}: {e}",
                    manifest_dir.display()
                )
            });
            copy_runner_template_file("Cargo.toml", &manifest_dir)
                .unwrap_or_else(|e| panic!("failed to copy runner Cargo.toml: {e}"));
            copy_runner_template_file("Cargo.lock", &manifest_dir)
                .unwrap_or_else(|e| panic!("failed to copy runner Cargo.lock: {e}"));
            copy_runner_template_file("src/main.rs", &manifest_dir)
                .unwrap_or_else(|e| panic!("failed to copy runner main.rs: {e}"));
            runners.push(E2eRunnerInstance {
                manifest_dir,
                target_dir: PathBuf::from(paths.target_dir),
            });
        }
        Self {
            _root: root,
            runners,
            available: Mutex::new((0..pool_size).rev().collect()),
            condvar: Condvar::new(),
        }
    }

    fn acquire(&'static self) -> E2eRunnerLease {
        let mut available = self.available.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(slot) = available.pop() {
                return E2eRunnerLease { pool: self, slot };
            }
            available = self
                .condvar
                .wait(available)
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    fn runner(&self, slot: usize) -> &E2eRunnerInstance {
        &self.runners[slot]
    }
}

struct E2eRunnerLease {
    pool: &'static E2eRunnerPool,
    slot: usize,
}

impl E2eRunnerLease {
    fn runner(&self) -> &E2eRunnerInstance {
        self.pool.runner(self.slot)
    }
}

impl Drop for E2eRunnerLease {
    fn drop(&mut self) {
        let mut available = self
            .pool
            .available
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        available.push(self.slot);
        self.pool.condvar.notify_one();
    }
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
    let runner = E2eRunnerPool::global().acquire();
    execute_e2e_with_runner(&runner, name, &E2eOptions::default())
}

/// Transpiles and executes a single TS script with custom options.
///
/// `name` can be a flat name (e.g. "hello") or a subdir path (e.g. "sdcdf-smoke/let-init-string-lit").
fn execute_e2e_with_options(name: &str, opts: &E2eOptions) -> E2eResult {
    let runner = E2eRunnerPool::global().acquire();
    execute_e2e_with_runner(&runner, name, opts)
}

fn execute_e2e_with_runner(runner: &E2eRunnerLease, name: &str, opts: &E2eOptions) -> E2eResult {
    let script_path = format!("{SCRIPTS_DIR}/{name}.ts");
    let ts_source = fs::read_to_string(&script_path)
        .unwrap_or_else(|e| panic!("failed to read {script_path}: {e}"));

    // Step 1: Transpile TS → Rust
    let rs_source =
        transpile(&ts_source).unwrap_or_else(|e| panic!("transpile failed for '{name}': {e}"));
    let rs_source = strip_internal_use_statements(&rs_source);

    // Step 2: Write Rust source and run in a runner-local manifest/target dir.
    runner.runner().reset_single_file_main(&rs_source);

    let mut rust_cmd = Command::new("cargo");
    rust_cmd
        .args(["run", "--quiet"])
        .current_dir(&runner.runner().manifest_dir)
        .env("CARGO_TARGET_DIR", &runner.runner().target_dir);
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
    let ts_exec_guard = runner
        .runner()
        .prepare_single_file_ts_exec(Path::new(&script_path), &ts_source);

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

/// Compares two outputs line-by-line, returning a diff string on mismatch.
///
/// Returns `None` if outputs match, `Some(diff)` otherwise.
fn compare_lines(ts_output: &str, rust_output: &str) -> Option<String> {
    let rust_lines: Vec<&str> = rust_output.lines().collect();
    let ts_lines: Vec<&str> = ts_output.lines().collect();

    if rust_lines == ts_lines {
        return None;
    }

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
    Some(diff)
}

/// Compares two outputs line-by-line, panicking with a diff on mismatch.
fn assert_lines_match(
    name: &str,
    stream: &str,
    ts_output: &str,
    rust_output: &str,
    rs_source: &str,
) {
    if let Some(diff) = compare_lines(ts_output, rust_output) {
        panic!(
            "{stream} mismatch for '{name}':\n{diff}\nTS {stream}:\n{ts_output}\nRust {stream}:\n{rust_output}\nGenerated Rust:\n{rs_source}"
        );
    }
}

#[test]
fn test_resolve_runner_pool_size_caps_to_four_by_default() {
    assert_eq!(resolve_runner_pool_size(Some(8), None), 4);
    assert_eq!(resolve_runner_pool_size(Some(3), None), 3);
}

#[test]
fn test_runner_instance_paths_are_isolated_per_slot() {
    let p0 = runner_instance_paths_for("/tmp/e2e-root", 0);
    let p1 = runner_instance_paths_for("/tmp/e2e-root", 1);

    assert_ne!(p0.manifest_dir, p1.manifest_dir);
    assert_ne!(p0.target_dir, p1.target_dir);
    assert!(p0.manifest_dir.ends_with("runner-0/rust-runner"));
    assert!(p1.manifest_dir.ends_with("runner-1/rust-runner"));
}

#[test]
fn test_cleanup_generated_runner_sources_removes_stale_modules_but_keeps_main() {
    let temp = tempfile::tempdir().expect("tempdir");
    let src_dir = temp.path().join("src");
    fs::create_dir_all(&src_dir).expect("create src");
    fs::write(src_dir.join("main.rs"), "fn main() {}\n").expect("write main");
    fs::write(src_dir.join("alpha.rs"), "pub fn alpha() {}\n").expect("write alpha");
    fs::write(src_dir.join("beta.rs"), "pub fn beta() {}\n").expect("write beta");

    cleanup_generated_runner_sources(&src_dir).expect("cleanup generated sources");

    assert!(src_dir.join("main.rs").exists());
    assert!(!src_dir.join("alpha.rs").exists());
    assert!(!src_dir.join("beta.rs").exists());
}

#[test]
fn test_single_file_ts_exec_path_is_runner_local() {
    let temp = tempfile::tempdir().expect("tempdir");
    let fixture_dir = temp.path().join("fixtures");
    fs::create_dir_all(&fixture_dir).expect("create fixture dir");
    fs::write(fixture_dir.join("case.ts"), "function main() {}\n").expect("write fixture");
    let runner = E2eRunnerInstance {
        manifest_dir: temp.path().join("runner"),
        target_dir: temp.path().join("target"),
    };

    let exec_guard =
        runner.prepare_single_file_ts_exec(&fixture_dir.join("case.ts"), "function main() {}\n");
    let exec_path = Path::new(exec_guard.path());

    assert!(exec_path.starts_with(runner.manifest_dir.join("ts-exec")));
    assert!(!exec_path.starts_with(&fixture_dir));
}

#[test]
fn test_multi_file_ts_exec_copies_fixture_files_to_runner_local_dir() {
    let temp = tempfile::tempdir().expect("tempdir");
    let fixture_dir = temp.path().join("multi").join("case");
    fs::create_dir_all(&fixture_dir).expect("create fixture dir");
    fs::write(
        fixture_dir.join("main.ts"),
        "import { value } from './dep';\nfunction main() {}\n",
    )
    .expect("write main");
    fs::write(fixture_dir.join("dep.ts"), "export const value = 1;\n").expect("write dep");
    let runner = E2eRunnerInstance {
        manifest_dir: temp.path().join("runner"),
        target_dir: temp.path().join("target"),
    };

    let exec_guard = runner.prepare_multi_file_ts_exec(&fixture_dir, "function main() {}\n");
    let exec_path = Path::new(exec_guard.path());
    let exec_dir = exec_path.parent().expect("exec parent");

    assert!(exec_path.starts_with(runner.manifest_dir.join("ts-exec")));
    assert!(!exec_path.starts_with(&fixture_dir));
    assert!(exec_dir.join("dep.ts").exists());
}

#[test]
fn test_parallel_e2e_runner_isolation_smoke() {
    let (hello, arithmetic) = std::thread::scope(|scope| {
        let hello = scope.spawn(|| execute_e2e("hello"));
        let arithmetic = scope.spawn(|| execute_e2e("arithmetic"));
        (
            hello.join().expect("hello join"),
            arithmetic.join().expect("arithmetic join"),
        )
    });

    assert_lines_match(
        "hello",
        "stdout",
        &hello.ts_stdout,
        &hello.rust_stdout,
        &hello.rs_source,
    );
    assert_lines_match(
        "arithmetic",
        "stdout",
        &arithmetic.ts_stdout,
        &arithmetic.rust_stdout,
        &arithmetic.rs_source,
    );
}

/// Runs an E2E test comparing stdout only (existing behavior).
fn run_e2e_test(name: &str) {
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
    let runner = E2eRunnerPool::global().acquire();
    let dir = format!("{SCRIPTS_DIR}/multi/{name}");

    // Collect all .ts files
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("failed to read dir {dir}: {e}"))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ts"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut mod_names: Vec<String> = Vec::new();
    let mut generated_modules: Vec<(String, String)> = Vec::new();
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
            mod_names.push(stem.clone());
            generated_modules.push((stem, rs_source));
        }
    }

    // Prepend mod declarations to main.rs
    let mod_decls: String = mod_names.iter().map(|m| format!("mod {m};\n")).collect();
    let full_main = format!("{mod_decls}{main_rs}");

    runner
        .runner()
        .reset_multi_file_sources(&full_main, &generated_modules);

    // Run Rust
    let rust_output = Command::new("cargo")
        .args(["run", "--quiet"])
        .current_dir(&runner.runner().manifest_dir)
        .env("CARGO_TARGET_DIR", &runner.runner().target_dir)
        .output()
        .expect("failed to execute cargo run");

    assert!(
        rust_output.status.success(),
        "cargo run failed for multi-file '{name}':\nstderr: {}\ngenerated main.rs:\n{}",
        String::from_utf8_lossy(&rust_output.stderr),
        full_main
    );

    let rust_stdout = String::from_utf8_lossy(&rust_output.stdout);

    // Run TS (tsx resolves relative imports automatically)
    let main_ts = fs::read_to_string(format!("{dir}/main.ts")).unwrap();
    let ts_exec_guard = runner
        .runner()
        .prepare_multi_file_ts_exec(Path::new(&dir), &main_ts);

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

/// Runs all per-cell E2E tests in a PRD subdirectory.
///
/// Discovers all `.ts` files in `tests/e2e/scripts/{prd_id}/`, runs each as
/// an independent E2E test (TS stdout vs Rust stdout comparison), and reports
/// all failures at the end.
///
/// This is the SDCDF per-cell E2E runner (Phase 2 artifact).
fn run_cell_e2e_tests(prd_id: &str) {
    let runner = E2eRunnerPool::global().acquire();
    let dir = format!("{SCRIPTS_DIR}/{prd_id}");

    let mut entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("failed to read dir {dir}: {e}"))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ts"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    assert!(!entries.is_empty(), "no .ts cell fixtures found in {dir}");

    let mut failures: Vec<String> = Vec::new();

    for entry in &entries {
        let stem = entry
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let cell_name = format!("{prd_id}/{stem}");

        let result = execute_e2e_with_runner(&runner, &cell_name, &E2eOptions::default());

        if let Some(diff) = compare_lines(&result.ts_stdout, &result.rust_stdout) {
            failures.push(format!(
                "FAIL {cell_name}:\n{diff}  TS stdout:\n{}\n  Rust stdout:\n{}\n  Generated Rust:\n{}",
                result.ts_stdout, result.rust_stdout, result.rs_source
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {} cell tests failed for PRD '{prd_id}':\n\n{}",
            failures.len(),
            entries.len(),
            failures.join("\n---\n")
        );
    }
}

/// Runs a single per-cell E2E test by PRD id and cell id.
///
/// Equivalent to `run_e2e_test` but for cell fixtures in PRD subdirectories.
#[allow(dead_code)]
fn run_cell_e2e_test(prd_id: &str, cell_id: &str) {
    let cell_name = format!("{prd_id}/{cell_id}");
    let result = execute_e2e(&cell_name);
    assert_lines_match(
        &cell_name,
        "stdout",
        &result.ts_stdout,
        &result.rust_stdout,
        &result.rs_source,
    );
}

/// Runs an E2E test comparing both stdout and stderr.
fn run_e2e_test_with_stderr(name: &str) {
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
fn test_e2e_vec_index_option_return_ts_rust_stdout_match() {
    // I-138: Vec index read `arr[0]` in Option<T> context emits `.get(0).cloned()`
    // so empty Vec yields None (TS `undefined`) instead of panicking via `.unwrap()`.
    run_e2e_test("vec_index_option_return");
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

#[test]
fn test_e2e_callable_interface_ts_rust_stdout_match() {
    run_e2e_test("callable_interface");
}

#[test]
fn test_e2e_optional_params_ts_rust_stdout_match() {
    run_e2e_test("optional_params");
}

// --- SDCDF per-cell E2E tests ---
// PRD subdirectory tests use `run_cell_e2e_tests` (batch) or `run_cell_e2e_test` (single).

#[test]
fn test_e2e_cell_sdcdf_smoke() {
    run_cell_e2e_tests("sdcdf-smoke");
}

#[test]
fn test_e2e_cell_i050a() {
    run_cell_e2e_tests("i050a");
}

#[test]
fn test_e2e_cell_step3() {
    run_cell_e2e_tests("step3");
}

#[test]
fn test_e2e_cell_i142bc() {
    run_cell_e2e_tests("i142bc");
}

#[test]
fn test_e2e_cell_i153() {
    // I-153: switch case body nested bare break rewrite + label hygiene.
    // Each cell fixture verifies TSX stdout = Rust stdout for a matrix cell.
    run_cell_e2e_tests("i153");
}

#[test]
fn test_e2e_cell_i154() {
    // I-154: user labels colliding with ts_to_rs internal label names
    // (`try_block`, `do_while`, etc.) must still work independently after rename.
    run_cell_e2e_tests("i154");
}

// -----------------------------------------------------------------------------
// I-144 per-cell E2E fixtures (`tests/e2e/scripts/i144/`).
//
// SDCDF Spec-Stage T1 artifact: one fixture per Problem Space matrix cell so
// phase-by-phase progress (T6-1 through T6-5) can un-ignore specific cells as
// the implementation lands. The aggregate harness `run_cell_e2e_tests("i144")`
// used in T1 was replaced by the per-cell functions below at T6-1 so that a
// fixture still RED in a later phase does not mask a cell the current phase
// made GREEN.
//
// Each function's `#[ignore]` reason (when present) names the phase that will
// turn it GREEN. `scripts/record-cell-oracle.sh` wrote `.expected` files
// alongside each `.ts` fixture for human review; the runner recomputes TS
// stdout live.
// -----------------------------------------------------------------------------

// Baseline GREEN fixtures: these lock in existing narrowing behavior and
// must not regress as the I-144 analyzer replaces the legacy scanner.
#[test]
fn test_e2e_cell_i144_closure_no_reassign_keeps_e1() {
    run_cell_e2e_test("i144", "cell-regression-closure-no-reassign-keeps-e1");
}
#[test]
fn test_e2e_cell_i144_f4_loop_body_narrow_preserves() {
    run_cell_e2e_test("i144", "cell-regression-f4-loop-body-narrow-preserves");
}
#[test]
fn test_e2e_cell_i144_null_check_narrow() {
    run_cell_e2e_test("i144", "cell-regression-null-check-narrow");
}
#[test]
fn test_e2e_cell_i144_r5_nullish_on_narrowed_is_noop() {
    run_cell_e2e_test("i144", "cell-regression-r5-nullish-on-narrowed-is-noop");
}
#[test]
fn test_e2e_cell_i144_rc_narrow_read_contexts() {
    run_cell_e2e_test("i144", "cell-regression-rc-narrow-read-contexts");
}

// I-169 T6-2 follow-up GREEN lockin (matrix cell #3 / P1 multi-fn isolation):
// `g`'s narrow must fire independently from `f`'s closure-reassign event.
#[test]
fn test_e2e_cell_i144_multifn_same_var_isolation() {
    run_cell_e2e_test("i144", "cell-regression-multifn-same-var-isolation");
}

// T6-1 GREEN fixtures: ??= EmissionHint dispatch + scanner retirement.
#[test]
fn test_e2e_cell_i144_14_narrowing_reset_structural() {
    run_cell_e2e_test("i144", "cell-14-narrowing-reset-structural");
}
#[test]
fn test_e2e_cell_i144_c1_compound_arith_preserves_narrow() {
    run_cell_e2e_test("i144", "cell-c1-compound-arith-preserves-narrow");
}
#[test]
fn test_e2e_cell_i144_c2a_nullish_assign_closure_capture() {
    run_cell_e2e_test("i144", "cell-c2a-nullish-assign-closure-capture");
}

// T6-2 GREEN (2026-04-20): closure-reassign suppresses narrow shadow-let,
// stale reads coerce via the `helpers::coerce_default` JS table
// (`null + 1 = 1`, `"v=" + null = "v=null"`).
#[test]
fn test_e2e_cell_i144_c2b_closure_reassign_arith_read() {
    run_cell_e2e_test("i144", "cell-c2b-closure-reassign-arith-read");
}
#[test]
fn test_e2e_cell_i144_c2c_closure_reassign_string_concat() {
    run_cell_e2e_test("i144", "cell-c2c-closure-reassign-string-concat");
}

// T6-3 complete: E10 truthy predicate (primitive NaN + composite
// `Option<Union<T, U>>` via consolidated match emission).
#[test]
fn test_e2e_cell_i144_t4d_truthy_number_nan() {
    run_cell_e2e_test("i144", "cell-t4d-truthy-number-nan");
}
#[test]
fn test_e2e_cell_i144_i024_truthy_option_complex() {
    run_cell_e2e_test("i144", "cell-i024-truthy-option-complex");
}
// T6-3 regression (T4c/T4e): primitive-String / primitive-Bool truthy emission
// via the same `try_generate_primitive_truthy_condition` path as cell-t4d.
#[test]
fn test_e2e_cell_i144_regression_t4c_truthy_primitive_string() {
    run_cell_e2e_test("i144", "cell-regression-t4c-truthy-primitive-string");
}
#[test]
fn test_e2e_cell_i144_regression_t4e_truthy_primitive_bool() {
    run_cell_e2e_test("i144", "cell-regression-t4e-truthy-primitive-bool");
}
// T6-3 regression (H-3): composite Option<Union> truthy where the union mixes
// primitive and non-primitive variants is locked in by a unit test on
// `build_union_variant_truthy_arms` (see control_flow tests module) because
// the E2E path is currently blocked by an unrelated call-arg / return Union
// coercion gap for non-literal expressions. Unit-level lock-in directly
// validates the match-arm shape independent of that separate defect.

// T6-4: compound OptChain narrow (`x?.v !== undefined` narrows x).
#[test]
fn test_e2e_cell_i144_t7_optchain_compound_narrow() {
    run_cell_e2e_test("i144", "cell-t7-optchain-compound-narrow");
}

// T6-5: multi-exit Option return implicit None tail injection.
#[test]
fn test_e2e_cell_i144_i025_option_return_implicit_none_complex() {
    run_cell_e2e_test("i144", "cell-i025-option-return-implicit-none-complex");
}

// -----------------------------------------------------------------------------
// I-161 + I-171 batch (backlog/I-161-I-171-truthy-emission-batch.md).
// SDCDF Spec-Stage T1 artifact: per-cell E2E fixtures derived from the Problem
// Space matrix. All ✗ cells start RED (`#[ignore]`) and become GREEN as the
// implementation in T2-T6 lands. The lock-in regression fixture for T6-3 (from
// I-144) must stay GREEN throughout the batch.
// -----------------------------------------------------------------------------

// T6-3 regression lock-in (Ident + Option<primitive> + always-exit): existing
// `try_generate_option_truthy_complement_match` must keep emitting the
// consolidated match.
#[test]
fn test_e2e_cell_i161_i171_regression_t6_3_ident_option() {
    run_cell_e2e_test("i161-i171", "cell-regression-t6-3-ident-option");
}

// Matrix A cells (I-161 `&&=` / `||=` structural fix). All RED until T3.
#[test]
#[ignore = "I-161 A-2 RED — unignore at T3 (AndAssign desugar for narrowed F64)"]
fn test_e2e_cell_i161_a2_and_f64_narrow() {
    run_cell_e2e_test("i161-i171", "cell-a2-and-f64-narrow");
}
#[test]
#[ignore = "I-161 A-5 RED — unignore at T3 (AndAssign desugar for Option<F64>)"]
fn test_e2e_cell_i161_a5_and_option_f64() {
    run_cell_e2e_test("i161-i171", "cell-a5-and-option-f64");
}
#[test]
#[ignore = "I-161 A-5s RED — unignore at T3 (AndAssign desugar for Option<String>)"]
fn test_e2e_cell_i161_a5s_and_option_string() {
    run_cell_e2e_test("i161-i171", "cell-a5s-and-option-string");
}
#[test]
#[ignore = "I-161 O-5 RED — unignore at T3 (OrAssign desugar for Option<F64>)"]
fn test_e2e_cell_i161_o5_or_option_f64() {
    run_cell_e2e_test("i161-i171", "cell-o5-or-option-f64");
}
#[test]
#[ignore = "I-161 A-3 RED — unignore at T3 (AndAssign desugar for String)"]
fn test_e2e_cell_i161_a3_and_string_empty() {
    run_cell_e2e_test("i161-i171", "cell-a3-and-string-empty");
}
#[test]
#[ignore = "I-161 A-Member RED — unignore at T3 (Member LHS desugar)"]
fn test_e2e_cell_i161_a_member_and() {
    run_cell_e2e_test("i161-i171", "cell-a-member-and");
}
#[test]
#[ignore = "I-161 A-Expr RED — unignore at T3 (expression-context AndAssign)"]
fn test_e2e_cell_i161_a_expr_context() {
    run_cell_e2e_test("i161-i171", "cell-a-expr-context");
}
#[test]
#[ignore = "I-161 A-6 RED — unignore at T3 (Option<synthetic union> per-variant)"]
fn test_e2e_cell_i161_a6_and_option_union() {
    run_cell_e2e_test("i161-i171", "cell-a6-and-option-union");
}

// Matrix B cells (I-171 Layer 1 `!<expr>` type-aware dispatch). All RED until T4.
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_f64_in_ret() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-f64-in-ret");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_string_in_ret() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-string-in-ret");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_option_number_in_ret() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-option-number-in-ret");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_bin_expr() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-bin-expr");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_double_option() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-double-option");
}
#[test]
#[ignore = "I-171 B.1.23 RED — T4 Bang-arm De Morgan emission is correct, but fixture \
           requires narrow materialisation of x/y after `if (!(x && y)) return;` for \
           post-return `${x}:${y}` usage. Narrow-scope blocker → I-177 (narrow emission v2)."]
fn test_e2e_cell_i171_b_bang_logical_and() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-logical-and");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_tsas() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-tsas");
}

// Matrix C cells (I-171 Layer 2 if-stmt narrow emission). All RED until T5.
#[test]
#[ignore = "I-171 C-4 RED — unignore at T5 (non-exit body predicate form)"]
fn test_e2e_cell_i171_c4_if_bang_non_exit() {
    run_cell_e2e_test("i161-i171", "cell-c4-if-bang-non-exit");
}
#[test]
#[ignore = "I-171 C-5 RED — unignore at T5 (else branch consolidated match)"]
fn test_e2e_cell_i171_c5_if_bang_else() {
    run_cell_e2e_test("i161-i171", "cell-c5-if-bang-else");
}
#[test]
#[ignore = "I-171 C-15 RED — unignore at T5 (Member LHS Layer 1 fix, narrow out-of-scope by I-165)"]
fn test_e2e_cell_i171_c15_if_bang_member_exit() {
    run_cell_e2e_test("i161-i171", "cell-c15-if-bang-member-exit");
}

// -----------------------------------------------------------------------------
// Supplementary Matrix coverage (spec-stage review gap closure 2026-04-22):
// Each cell listed as "runtime GREEN currently" exercises a current emission
// that happens to be valid Rust (e.g. `bool && bool`) but still undergoes
// structural desugar in T3 for uniformity. The E2E test locks in runtime
// equivalence; unit/snapshot tests in T3-T5 verify the new emission shape.
// -----------------------------------------------------------------------------

// Matrix A supplementary cells (A-1 / A-4 / A-7 / A-8).
#[test]
fn test_e2e_cell_i161_a1_and_bool() {
    // runtime GREEN (Rust `bool && bool` = valid, matches TS semantics).
    // T3 will change emission to `if x { x = y; }`; runtime remains GREEN.
    run_cell_e2e_test("i161-i171", "cell-a1-and-bool");
}
#[test]
#[ignore = "I-161 A-4 RED — unignore at T3 (int truthy predicate, `!arr.len()` semantic fix)"]
fn test_e2e_cell_i161_a4_and_int() {
    run_cell_e2e_test("i161-i171", "cell-a4-and-int");
}
#[test]
#[ignore = "I-161 A-7 RED — unignore at T3 (Option<Named> is_some predicate)"]
fn test_e2e_cell_i161_a7_and_option_named() {
    run_cell_e2e_test("i161-i171", "cell-a7-and-option-named");
}
#[test]
#[ignore = "I-161 A-8 RED — unignore at T3 (always-truthy const-fold)"]
fn test_e2e_cell_i161_a8_and_always_truthy() {
    run_cell_e2e_test("i161-i171", "cell-a8-and-always-truthy");
}

// Matrix O supplementary cells (O-1, O-2, O-3, O-5s, O-6, O-7, O-8).
#[test]
fn test_e2e_cell_i161_o1_or_bool() {
    // runtime GREEN (Rust `bool || bool` = valid); T3 desugars for uniformity.
    run_cell_e2e_test("i161-i171", "cell-o1-or-bool");
}
#[test]
#[ignore = "I-161 O-2 RED — unignore at T3 (F64 falsy predicate)"]
fn test_e2e_cell_i161_o2_or_f64() {
    run_cell_e2e_test("i161-i171", "cell-o2-or-f64");
}
#[test]
#[ignore = "I-161 O-3 RED — unignore at T3 (String is_empty falsy predicate)"]
fn test_e2e_cell_i161_o3_or_string() {
    run_cell_e2e_test("i161-i171", "cell-o3-or-string");
}
#[test]
#[ignore = "I-161 O-5s RED — unignore at T3 (Option<String> map_or falsy)"]
fn test_e2e_cell_i161_o5s_or_option_string() {
    run_cell_e2e_test("i161-i171", "cell-o5s-or-option-string");
}
#[test]
#[ignore = "I-161 O-6 RED — unignore at T3 (Option<synthetic union> per-variant falsy)"]
fn test_e2e_cell_i161_o6_or_option_union() {
    run_cell_e2e_test("i161-i171", "cell-o6-or-option-union");
}
#[test]
#[ignore = "I-161 O-7 RED — unignore at T3 (Option<Named> is_none predicate)"]
fn test_e2e_cell_i161_o7_or_option_named() {
    run_cell_e2e_test("i161-i171", "cell-o7-or-option-named");
}
#[test]
#[ignore = "I-161 O-8 RED — unignore at T3 (always-truthy const-fold no-op)"]
fn test_e2e_cell_i161_o8_or_always_truthy() {
    run_cell_e2e_test("i161-i171", "cell-o8-or-always-truthy");
}

// Matrix B supplementary cells (B-T4 / B-T6 / B-T7 / B-T8 Named / B-T8 Vec).
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_int() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-int");
}
#[test]
#[ignore = "I-171 B-T6 RED — T4 Bang-arm per-variant falsy match emission is correct, \
           but fixture `f(NaN)` call fails because `NaN` literal isn't wrapped as \
           `F64OrString::F64(f64::NAN)` at the call site (synthetic-union-constructor \
           coercion missing). Blocker → I-179 (synthetic union lit coercion at call args). \
           NaN handling inside the match is verified via unit tests."]
fn test_e2e_cell_i171_b_bang_option_union() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-option-union");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_option_named() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-option-named");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_named() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-named");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_vec() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-vec");
}

// Matrix C supplementary cells (C-7 const-fold, C-11-C-14 peek-through, C-16,
// C-17, C-18, C-19, C-23, C-24).
#[test]
#[ignore = "I-171 C-7 RED — unignore at T5 (const-fold `!null`)"]
fn test_e2e_cell_i171_c7_const_fold_null() {
    run_cell_e2e_test("i161-i171", "cell-c7-const-fold-null");
}
#[test]
#[ignore = "I-171 C-11 RED — unignore at T5 (Paren peek-through)"]
fn test_e2e_cell_i171_c11_peek_paren() {
    run_cell_e2e_test("i161-i171", "cell-c11-peek-paren");
}
#[test]
#[ignore = "I-171 C-12 RED — unignore at T5 (TsAs peek-through)"]
fn test_e2e_cell_i171_c12_peek_tsas() {
    run_cell_e2e_test("i161-i171", "cell-c12-peek-tsas");
}
#[test]
#[ignore = "I-171 C-13 RED — unignore at T5 (TsNonNull peek-through)"]
fn test_e2e_cell_i171_c13_peek_nonnull() {
    run_cell_e2e_test("i161-i171", "cell-c13-peek-nonnull");
}
#[test]
#[ignore = "I-171 C-14 RED — unignore at T5 (`!!x` double negation truthy fold)"]
fn test_e2e_cell_i171_c14_peek_unary() {
    run_cell_e2e_test("i161-i171", "cell-c14-peek-unary");
}
#[test]
#[ignore = "I-171 C-16 RED — unignore at T5 (OptChain Layer 1 only, narrow OOS by I-143-a+I-165)"]
fn test_e2e_cell_i171_c16_if_bang_optchain() {
    run_cell_e2e_test("i161-i171", "cell-c16-if-bang-optchain");
}
#[test]
#[ignore = "I-171 C-17 RED — unignore at T5 (Bin arith tmp-bind)"]
fn test_e2e_cell_i171_c17_if_bang_bin_arith() {
    run_cell_e2e_test("i161-i171", "cell-c17-if-bang-bin-arith");
}
#[test]
#[ignore = "I-171 C-18 RED — unignore at T5 (LogicalAnd De Morgan)"]
fn test_e2e_cell_i171_c18_if_bang_logical_and() {
    run_cell_e2e_test("i161-i171", "cell-c18-if-bang-logical-and");
}
#[test]
#[ignore = "I-171 C-19 RED — unignore at T5 (Call tmp-bind)"]
fn test_e2e_cell_i171_c19_if_bang_call() {
    run_cell_e2e_test("i161-i171", "cell-c19-if-bang-call");
}
#[test]
#[ignore = "I-171 C-23 RED — unignore at T5 (LogicalOr De Morgan)"]
fn test_e2e_cell_i171_c23_if_bang_logical_or() {
    run_cell_e2e_test("i161-i171", "cell-c23-if-bang-logical-or");
}
#[test]
#[ignore = "I-171 C-24 RED — unignore at T5 (always-truthy operand const-fold)"]
fn test_e2e_cell_i171_c24_if_bang_always_truthy() {
    run_cell_e2e_test("i161-i171", "cell-c24-if-bang-always-truthy");
}

// -----------------------------------------------------------------------------
// v4 adversarial review gap closure (2026-04-22):
// 8 additional Matrix B shape cells + 1 C-16b OptChain base narrow +
// 5 T7 classifier interaction regression fixtures.
// -----------------------------------------------------------------------------

// Matrix B.1 additional shape cells.
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_nc() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-nc");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_cond() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-cond");
}
#[test]
#[ignore = "I-171 B.1.32 RED — T4 Bang-arm tmp-bind on awaited F64 is correct, but \
           fixture TS top-level `main();` call produces duplicated stdout under tsx \
           (4 lines vs fixture's 2). Blocker → I-180 (E2E harness async-main execution \
           semantics). Bang-arm await dispatch is covered by unit tests."]
fn test_e2e_cell_i171_b_bang_await() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-await");
}
#[test]
#[ignore = "I-171 B.1.33 RED — T4 Bang-arm Assign desugar emission is correct \
           (`{ let tmp = rhs; x = tmp; falsy(tmp) }`), but fixture exercises \
           pre-existing emission defects: (1) tuple destructuring `[l, x] = f()` \
           lowers to `f().get(N).cloned().unwrap()` (array syntax, not tuple), \
           (2) ternary with string literals returns `&str` where `(String, f64)` is \
           expected. Blocker → I-181 (tuple destructuring + ternary &str/String \
           coercion). Assign desugar itself covered by unit tests."]
fn test_e2e_cell_i171_b_bang_assign() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-assign");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_this() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-this");
}
#[test]
#[ignore = "I-171 B.1.36 RED — T4 Bang-arm tmp-bind on Update (postfix `i++`) is \
           correct (`{ let _old = i; i = i+1; _old }`), but fixture exercises the \
           same pre-existing tuple-destructuring + ternary `&str`/`String` emission \
           defects as cell-b-bang-assign. Blocker → I-181. Update dispatch itself \
           covered by unit tests."]
fn test_e2e_cell_i171_b_bang_update() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-update");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_tstypeassertion() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-tstypeassertion");
}
#[test]
// GREEN at T4
fn test_e2e_cell_i171_b_bang_tsconstassertion() {
    run_cell_e2e_test("i161-i171", "cell-b-bang-tsconstassertion");
}

// Matrix C-16b: OptChain base narrow (in-scope, T6 P3b extension).
#[test]
#[ignore = "I-171 C-16b RED — unignore at T6 P3b (guards.rs OptChain base narrow extension)"]
fn test_e2e_cell_i171_c16b_optchain_base_narrow() {
    run_cell_e2e_test("i161-i171", "cell-c16b-optchain-base-narrow");
}

// T7 regression cells (classifier × narrow × logical assign interaction).
#[test]
#[ignore = "I-161 T7-1 RED — unignore at T7 (&&= on narrowed F64, R4 re-host)"]
fn test_e2e_cell_i161_t7_1_and_narrow_f64() {
    run_cell_e2e_test("i161-i171", "cell-t7-1-and-narrow-f64");
}
#[test]
#[ignore = "I-161 T7-2 RED — unignore at T7 (||= on narrowed F64)"]
fn test_e2e_cell_i161_t7_2_or_narrow_f64() {
    run_cell_e2e_test("i161-i171", "cell-t7-2-or-narrow-f64");
}
#[test]
#[ignore = "I-161 T7-3 RED — unignore at T7 (&&= + closure reassign interaction)"]
fn test_e2e_cell_i161_t7_3_and_closure_reassign() {
    run_cell_e2e_test("i161-i171", "cell-t7-3-and-closure-reassign");
}
#[test]
#[ignore = "I-161 T7-4 RED — unignore at T7 (||= then ??= chain)"]
fn test_e2e_cell_i161_t7_4_or_then_nc() {
    run_cell_e2e_test("i161-i171", "cell-t7-4-or-then-nc");
}
#[test]
#[ignore = "I-161 T7-5 RED — unignore at T7 (&&= on narrowed synthetic union + string RHS)"]
fn test_e2e_cell_i161_t7_5_and_narrow_union_rhs() {
    run_cell_e2e_test("i161-i171", "cell-t7-5-and-narrow-union-rhs");
}
