//! I-399 INV-T1 / INV-T2 / INV-T3 structural lock-in via integration tests.
//!
//! These tests turn the empirical observations recorded in
//! `backlog/I-399-e2e-test-isolation-defect.md` § Implementation T3 into
//! automated regression guards. They are split into two cohorts:
//!
//! - **Heavyweight invariants (T1 / T2 / T3)**: each test invokes
//!   `cargo test --test e2e_test` as a subprocess (≥1 round of the full E2E
//!   suite, ~120s per round). They are gated behind `#[ignore]` so they do
//!   not run on every CI invocation. Opt in with
//!   `cargo test --test i399_isolation_test -- --ignored` (each round does
//!   the minimum necessary work to verify the named invariant) or set
//!   `I399_DEEP_VERIFY=1` to run the deeper variants:
//!     - `test_invariant_t1_test_execution_determinism`: 1 round (smoke)
//!       by default, 10 rounds with `I399_DEEP_VERIFY=1` (= determinism
//!       across rounds).
//!     - `test_invariant_t2_cross_mode_invariance`: 4 modes × 1 round.
//!     - `test_invariant_t3_performance_regression_bound`: 5 rounds with
//!       round 0 excluded as warm-up, mean compared against the PRD pre-fix
//!       baseline (override via `I399_INV_T3_BASELINE_SECS`).
//!
//! - **Lightweight invariants (T4)**: pure-function and minimal-cargo
//!   probes that always run on CI and finish in seconds.
//!     - `test_content_hash_bin_name_invariants`: determinism / distinctness /
//!       format / 1000-input distribution of `content_hash_bin_name`.
//!     - `test_per_test_content_hash_isolation`: end-to-end empirical
//!       verification that cargo's per-bin fingerprint isolates two bins
//!       in the same package (= the architectural assumption that the
//!       I-399 structural fix relies on).
//!
//! Recursive self-invocation safeguard: the heavyweight tests pass
//! `--test e2e_test -- --skip i399_isolation_test` to the subprocess.
//! `--test e2e_test` already constrains the run to the e2e_test binary
//! (so `i399_isolation_test` itself never re-enters), but the explicit
//! `--skip i399_isolation_test` documents the safeguard and keeps the
//! invariant intact even if the subprocess invocation is ever generalised.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[path = "i399_runner_mech.rs"]
mod i399_runner_mech;
use i399_runner_mech::content_hash_bin_name;

/// Pre-fix baseline (seconds) for INV-T3, taken from PRD § INV-T3 (c)
/// Verification method (Iteration v3): mean of rounds 2-5 on commit
/// `80d9df1` with cold-compile rounds excluded = 165.72s. The baseline is
/// hardware-specific; override with `I399_INV_T3_BASELINE_SECS=<float>`
/// when measuring on slower CI hardware.
const PRE_FIX_BASELINE_SECS: f64 = 165.72;

/// Tolerance bound for INV-T3 (= ±10% per PRD acceptance criterion).
const INV_T3_TOLERANCE: f64 = 0.10;

/// Number of rounds for INV-T3 performance measurement (round 0 is warm-up,
/// rounds 1..N contribute to the mean).
const INV_T3_ROUNDS: usize = 5;

/// `cargo test --test e2e_test` summary parsed from the subprocess stdout
/// (= the line `test result: ok. N passed; M failed; K ignored; ...`).
#[derive(Clone, Eq, PartialEq, Debug)]
struct ResultSummary {
    passed: usize,
    failed: usize,
    ignored: usize,
}

struct SuiteResult {
    summary: ResultSummary,
    elapsed: Duration,
    success: bool,
    stdout: String,
    stderr: String,
}

/// Spawns `cargo test --test e2e_test` as a subprocess with the supplied
/// extra harness args (passed after the `--` separator) and extra env
/// vars. The `--skip i399_isolation_test` flag is always included as a
/// recursion safeguard (see module-level doc).
fn run_e2e_suite(extra_args: &[&str], extra_env: &[(&str, &str)]) -> SuiteResult {
    let start = Instant::now();
    let mut cmd = Command::new("cargo");
    cmd.args([
        "test",
        "--test",
        "e2e_test",
        "--",
        "--skip",
        "i399_isolation_test",
    ]);
    for arg in extra_args {
        cmd.arg(arg);
    }
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.env("CARGO_TERM_COLOR", "never");

    let output = cmd
        .output()
        .expect("failed to spawn `cargo test --test e2e_test`");
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let summary = parse_test_result(&stdout, &stderr);
    SuiteResult {
        summary,
        elapsed,
        success: output.status.success(),
        stdout,
        stderr,
    }
}

/// Extracts the `ResultSummary` from the last `test result:` line of the
/// `cargo test` stdout. Panics with a diagnostic if the line is missing or
/// malformed (= this happens only for compile failures / cargo errors,
/// which we want to surface loudly).
fn parse_test_result(stdout: &str, stderr: &str) -> ResultSummary {
    let line = stdout
        .lines()
        .rfind(|line| line.contains("test result:"))
        .unwrap_or_else(|| {
            panic!(
                "`test result:` line not found in cargo stdout (compile failure?):\n\
                 ----- stdout -----\n{stdout}\n----- stderr -----\n{stderr}"
            )
        });
    ResultSummary {
        passed: parse_count(line, "passed"),
        failed: parse_count(line, "failed"),
        ignored: parse_count(line, "ignored"),
    }
}

fn parse_count(line: &str, kind: &str) -> usize {
    let pattern = format!(" {kind};");
    let end = line
        .find(&pattern)
        .unwrap_or_else(|| panic!("`{kind};` token not found in `test result:` line `{line}`"));
    let prefix = &line[..end];
    let start = prefix
        .rfind(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .unwrap_or(0);
    prefix[start..].parse().unwrap_or_else(|e| {
        panic!("failed to parse `{kind}` count from `test result:` line `{line}`: {e}")
    })
}

/// Resolves the round count for INV-T1: 10 in deep mode, 1 in smoke mode.
fn deep_verify_rounds() -> usize {
    if std::env::var("I399_DEEP_VERIFY").is_ok() {
        10
    } else {
        1
    }
}

/// Resolves the INV-T3 baseline (seconds), allowing CI/hardware overrides.
fn inv_t3_baseline_secs() -> f64 {
    std::env::var("I399_INV_T3_BASELINE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(PRE_FIX_BASELINE_SECS)
}

/// One INV-T2 mode entry: `(label, harness args after `--`, env vars)`.
struct ModeSpec {
    label: &'static str,
    harness_args: &'static [&'static str],
    env: &'static [(&'static str, &'static str)],
}

// ===== Heavyweight invariants (gated behind `#[ignore]`) =====

#[test]
#[ignore = "I-399 INV-T1 deep verify; opt-in via `cargo test --test i399_isolation_test -- --ignored`. \
            Set `I399_DEEP_VERIFY=1` for 10-round determinism (~20 min); default 1-round smoke (~3 min)."]
fn test_invariant_t1_test_execution_determinism() {
    let rounds = deep_verify_rounds();
    let mut summaries: Vec<ResultSummary> = Vec::with_capacity(rounds);
    for round in 0..rounds {
        eprintln!("INV-T1 round {} of {}", round + 1, rounds);
        let result = run_e2e_suite(&[], &[]);
        assert!(
            result.success,
            "INV-T1 round {round} subprocess failed:\n----- stdout -----\n{}\n----- stderr -----\n{}",
            result.stdout, result.stderr
        );
        summaries.push(result.summary);
    }

    if rounds > 1 {
        let first = &summaries[0];
        for (i, summary) in summaries.iter().enumerate().skip(1) {
            assert_eq!(
                summary, first,
                "INV-T1 violation: round {i} result {summary:?} differs from round 0 {first:?}"
            );
        }
    }
}

#[test]
#[ignore = "I-399 INV-T2 cross-mode verify; opt-in via `cargo test --test i399_isolation_test -- --ignored`. \
            Runs the e2e suite in 4 modes (~10 min total)."]
fn test_invariant_t2_cross_mode_invariance() {
    const MODES: &[ModeSpec] = &[
        ModeSpec {
            label: "parallel-default",
            harness_args: &[],
            env: &[],
        },
        ModeSpec {
            label: "serial",
            harness_args: &["--test-threads=1"],
            env: &[],
        },
        ModeSpec {
            label: "thread-count=8",
            harness_args: &["--test-threads=8"],
            env: &[],
        },
        ModeSpec {
            label: "parallel-CARGO_INCREMENTAL=0",
            harness_args: &[],
            env: &[("CARGO_INCREMENTAL", "0")],
        },
    ];

    let mut summaries: Vec<(&'static str, ResultSummary)> = Vec::with_capacity(MODES.len());
    for mode in MODES {
        eprintln!("INV-T2 mode: {}", mode.label);
        let result = run_e2e_suite(mode.harness_args, mode.env);
        assert!(
            result.success,
            "INV-T2 mode {label} subprocess failed:\n----- stdout -----\n{}\n----- stderr -----\n{}",
            result.stdout,
            result.stderr,
            label = mode.label,
        );
        summaries.push((mode.label, result.summary));
    }

    let (first_label, first_summary) = summaries[0].clone();
    for (label, summary) in summaries.iter().skip(1) {
        assert_eq!(
            summary, &first_summary,
            "INV-T2 violation: mode `{label}` summary {summary:?} differs from \
             baseline mode `{first_label}` {first_summary:?}"
        );
    }
}

#[test]
#[ignore = "I-399 INV-T3 performance regression bound; opt-in via `cargo test --test i399_isolation_test -- --ignored`. \
            Runs the e2e suite × 5 rounds (~12 min). Override pre-fix baseline via `I399_INV_T3_BASELINE_SECS=<seconds>`."]
fn test_invariant_t3_performance_regression_bound() {
    let baseline = inv_t3_baseline_secs();
    let bound = baseline * (1.0 + INV_T3_TOLERANCE);

    let mut elapsed_secs: Vec<f64> = Vec::with_capacity(INV_T3_ROUNDS);
    for round in 0..INV_T3_ROUNDS {
        eprintln!("INV-T3 round {} of {}", round + 1, INV_T3_ROUNDS);
        let result = run_e2e_suite(&[], &[]);
        assert!(
            result.success,
            "INV-T3 round {round} subprocess failed:\n----- stdout -----\n{}\n----- stderr -----\n{}",
            result.stdout, result.stderr
        );
        elapsed_secs.push(result.elapsed.as_secs_f64());
    }

    // Per PRD INV-T3 (c): warm-up exclusion (round 0) + arithmetic mean of
    // rounds 1..N. Round 0 is dominated by cold-compile cost on the e2e_test
    // binary (post-`cargo clean` or post-source-edit), which is not part of
    // the steady-state measurement.
    let warm: Vec<f64> = elapsed_secs.iter().skip(1).copied().collect();
    let mean = warm.iter().sum::<f64>() / warm.len() as f64;

    eprintln!("INV-T3 elapsed (seconds, round-by-round): {elapsed_secs:?}");
    eprintln!(
        "INV-T3 warm-up-excluded mean = {mean:.2}s, baseline = {baseline:.2}s, \
         bound = {bound:.2}s (= baseline × {tol:.2})",
        tol = 1.0 + INV_T3_TOLERANCE
    );

    assert!(
        mean <= bound,
        "INV-T3 violation: post-fix mean {mean:.2}s exceeds bound {bound:.2}s \
         (= baseline {baseline:.2}s × {tol:.2}). Round-by-round elapsed: {elapsed_secs:?}. \
         If measuring on slower hardware, override the baseline via \
         I399_INV_T3_BASELINE_SECS=<seconds>.",
        tol = 1.0 + INV_T3_TOLERANCE
    );
}

// ===== Lightweight invariants (always on CI) =====

/// `content_hash_bin_name` invariants: determinism, distinctness, format,
/// and distribution-at-production-scale (= 1000 distinct inputs must
/// produce 1000 distinct bin names so the FNV-1a + 12-hex-char design
/// remains structurally collision-free for the suite's ≤1k-test scale).
#[test]
fn test_content_hash_bin_name_invariants() {
    // (a) Determinism: identical content yields identical bin name (=
    //     precondition for cargo's cache-reuse path).
    let a1 = content_hash_bin_name("source A");
    let a2 = content_hash_bin_name("source A");
    assert_eq!(
        a1, a2,
        "INV-T isolation (a): content_hash_bin_name must be deterministic for \
         the same input"
    );

    // (b) Distinctness: distinct content yields distinct bin names (=
    //     precondition for cargo's per-bin path-collision elimination).
    let b = content_hash_bin_name("source B");
    assert_ne!(
        a1, b,
        "INV-T isolation (b): content_hash_bin_name must distinguish distinct \
         inputs `source A` vs `source B` (otherwise the I-399 path-collision \
         elimination is broken)"
    );

    // (c) Format invariants: 'b' prefix + 12 ASCII hex chars (length 13).
    //     The 'b' prefix avoids the leading-digit ident violation; ASCII
    //     hex avoids collisions with the runner package name
    //     (`e2e-rust-runner`, contains `-`).
    assert_eq!(
        a1.len(),
        13,
        "INV-T isolation (c): bin name must be 'b' + 12 hex chars (length 13), got {a1:?}"
    );
    assert!(
        a1.starts_with('b'),
        "INV-T isolation (c): bin name must start with 'b' (got {a1:?})"
    );
    assert!(
        a1[1..].chars().all(|c| c.is_ascii_hexdigit()),
        "INV-T isolation (c): bin name suffix must be ASCII hex digits (got {a1:?})"
    );

    // (d) Distribution at production scale: 1000 distinct inputs yield 1000
    //     distinct outputs. This is the empirical floor that justifies the
    //     FNV-1a 12-char design choice (suite scale ≤1000 tests; collision
    //     probability ≈ N²/2^48 ≈ 3.6e-9 at N=1000). If the suite ever
    //     grows beyond ~10⁵ tests, revisit the truncation length.
    let mut seen = HashSet::with_capacity(1000);
    for i in 0..1000 {
        let source = format!("fn main() {{ println!(\"variant {i}\"); }}");
        let hash = content_hash_bin_name(&source);
        assert!(
            seen.insert(hash.clone()),
            "INV-T isolation (d): hash collision detected at i={i} (hash={hash}). \
             The FNV-1a + 12-hex truncation design assumes ≤1k-test scale; if \
             this fires, the suite has grown beyond design bound or the hash \
             function regressed."
        );
    }
    assert_eq!(seen.len(), 1000);
}

/// End-to-end probe of cargo's per-bin fingerprint behavior with two
/// content-hash-derived bin names in the same minimal package. This is the
/// architectural assumption underlying the I-399 structural fix:
/// `cargo build --bin <bin_y>` must not rebuild `<bin_x>`, and rebuilding
/// `<bin_x>` with identical source must hit the cache.
///
/// If this test ever fails, the I-399 fix's structural soundness is
/// compromised (= cargo cannot isolate per-bin fingerprints, the fix
/// degrades to "lower probability of stale binary leak" rather than
/// structurally eliminating it).
#[test]
fn test_per_test_content_hash_isolation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let proj = tmp.path();
    fs::create_dir_all(proj.join("src")).expect("create src dir");

    // Stub `src/main.rs` so cargo accepts the package skeleton; the actual
    // bins under test are appended via `[[bin]]` entries below.
    fs::write(proj.join("src/main.rs"), "fn main() {}\n").expect("write src/main.rs stub");

    let bin_x = content_hash_bin_name(
        "// I-399 isolation probe — source X\nfn main() { println!(\"x\"); }\n",
    );
    let bin_y = content_hash_bin_name(
        "// I-399 isolation probe — source Y\nfn main() { println!(\"y\"); }\n",
    );
    assert_ne!(
        bin_x, bin_y,
        "test setup invariant: distinct sources must produce distinct bin names"
    );

    let target = proj.join("target");

    // Step 1: write bin_x source, register [[bin]], build, capture mtime.
    fs::write(
        proj.join(format!("src/{bin_x}.rs")),
        "// I-399 isolation probe — source X\nfn main() { println!(\"x\"); }\n",
    )
    .expect("write bin_x source");
    write_cargo_toml_with_bins(
        &proj.join("Cargo.toml"),
        &[(&bin_x, &format!("src/{bin_x}.rs"))],
    );
    cargo_build_bin(proj, &target, &bin_x, "build bin_x");
    let bin_x_path = target.join("debug").join(&bin_x);
    let mtime_x_initial = fs::metadata(&bin_x_path)
        .expect("bin_x metadata after initial build")
        .modified()
        .expect("bin_x mtime");

    // Step 2: sleep > filesystem mtime granularity (ext4 nanosecond, but be
    // defensive against WSL2 / macOS HFS+ 1s-precision quirks).
    std::thread::sleep(Duration::from_secs(2));

    // Step 3: write bin_y source, register [[bin]], build only bin_y, then
    // verify bin_x is unchanged. This is the architectural assumption: a
    // build of bin_y must not touch bin_x's binary on disk.
    fs::write(
        proj.join(format!("src/{bin_y}.rs")),
        "// I-399 isolation probe — source Y\nfn main() { println!(\"y\"); }\n",
    )
    .expect("write bin_y source");
    write_cargo_toml_with_bins(
        &proj.join("Cargo.toml"),
        &[
            (&bin_x, &format!("src/{bin_x}.rs")),
            (&bin_y, &format!("src/{bin_y}.rs")),
        ],
    );
    cargo_build_bin(proj, &target, &bin_y, "build bin_y");

    let bin_y_path = target.join("debug").join(&bin_y);
    assert!(
        bin_y_path.exists(),
        "bin_y must exist after `cargo build --bin {bin_y}`"
    );

    let mtime_x_after_y = fs::metadata(&bin_x_path)
        .expect("bin_x metadata after bin_y build")
        .modified()
        .expect("bin_x mtime");
    assert_eq!(
        mtime_x_initial, mtime_x_after_y,
        "INV-T isolation: bin_x mtime must be unchanged after `cargo build --bin {bin_y}` \
         (= cargo's per-bin fingerprint must isolate bin_x from bin_y's build). \
         If this fires, the I-399 structural fix's correctness is compromised."
    );

    // Step 4: rebuild bin_x with identical source — verify cache hit
    // (= mtime unchanged). This is the second half of the architectural
    // assumption: same content → cache reuse.
    cargo_build_bin(proj, &target, &bin_x, "rebuild bin_x");
    let mtime_x_rebuild = fs::metadata(&bin_x_path)
        .expect("bin_x metadata after rebuild")
        .modified()
        .expect("bin_x mtime");
    assert_eq!(
        mtime_x_initial, mtime_x_rebuild,
        "INV-T isolation: bin_x mtime must be unchanged on rebuild with identical \
         source (= cargo cache hit). If this fires, cache reuse is broken and \
         the I-399 fix's performance characteristic is unsupported."
    );
}

/// Renders the package skeleton + the supplied `[[bin]]` entries to
/// `Cargo.toml`. Always written from scratch (not appended) so the test is
/// idempotent across steps.
fn write_cargo_toml_with_bins(path: &Path, bins: &[(&str, &str)]) {
    let mut content = String::from(
        r#"[package]
name = "i399-isolation-probe"
version = "0.1.0"
edition = "2021"
"#,
    );
    for (name, rel_path) in bins {
        content.push_str(&format!(
            "\n[[bin]]\nname = \"{name}\"\npath = \"{rel_path}\"\n"
        ));
    }
    fs::write(path, content).expect("write Cargo.toml");
}

fn cargo_build_bin(proj: &Path, target: &Path, bin_name: &str, step: &str) {
    let output = Command::new("cargo")
        .args(["build", "--bin", bin_name, "--quiet"])
        .current_dir(proj)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `cargo build --bin {bin_name}` ({step}): {e}"));
    assert!(
        output.status.success(),
        "`cargo build --bin {bin_name}` ({step}) failed:\n----- stdout -----\n{}\n----- stderr -----\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
