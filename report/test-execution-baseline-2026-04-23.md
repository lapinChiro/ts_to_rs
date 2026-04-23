# Test Execution Baseline (2026-04-23)

## Summary

- Base commit: `4bc50c9` (`[WIP] I-171 T4 完了 ...`)
- Working tree: clean (`git status --short` returned no output)
- Host: `x86_64-unknown-linux-gnu`, `rustc 1.96.0-nightly (3b1b0ef4d 2026-03-11)`, `nproc = 8`
- Measurement mode: warm/incremental baseline. `target/` and `tests/e2e/node_modules/` already existed; no `cargo clean` was run.
- Current end-to-end baseline: `cargo test` completes in `184.22s` real time.
- Dominant cost is not Rust unit tests. The bottlenecks are `compile_test` and `e2e_test`.

## Detailed Findings

### 1. Test inventory

`cargo test -- --list | rg ': test$|: benchmark$' | wc -l` reported `3415` discovered tests/benches.

Observed executed/ignored counts from the current suite:

| Scope | Executed | Ignored |
| --- | ---: | ---: |
| `src/lib.rs` unit tests | 3085 | 0 |
| `tests/cli_test.rs` | 3 | 0 |
| `tests/compile_test.rs` | 3 | 0 |
| `tests/e2e_test.rs` | 132 | 42 |
| Other integration tests | 146 | 0 |
| Doc-tests | 0 | 4 |
| Total | 3369 | 46 |

The large raw count is misleading: `3085` unit tests finish quickly, while the few `compile_test` / `e2e_test` cases dominate wall-clock time.

### 2. Measured timings

Measured with `/usr/bin/time -p`.

| Command | Result | real | user | sys |
| --- | --- | ---: | ---: | ---: |
| `cargo test` | pass | `184.22s` | `114.16s` | `53.97s` |
| `cargo test --lib` | pass | `4.49s` | `5.45s` | `1.24s` |
| `cargo test --test integration_test` | pass | `2.05s` | `2.40s` | `0.35s` |
| `cargo test --test compile_test` | pass | `106.12s` | `66.25s` | `29.89s` |
| `cargo test --test e2e_test -- --test-threads=1` | pass | `145.61s` | `96.22s` | `46.52s` |

Important note: isolated timings are not additive. `cargo test` reuses compilation/artifacts across binaries, so `106.12s + 145.61s` overstates the full-run total. Even so, these two binaries are clearly the only first-order bottlenecks.

### 3. `compile_test` is structurally expensive

`tests/compile_test.rs` serializes the entire binary with `COMPILE_LOCK` and then runs `cargo check` repeatedly against the shared `tests/compile-check` project.

Relevant code:

- Shared project + lock: `tests/compile_test.rs:18-21`
- Per-fixture `cargo check`: `tests/compile_test.rs:69-97`
- Fixture loops: `tests/compile_test.rs:179-206`, `tests/compile_test.rs:261-289`, `tests/compile_test.rs:400-403`

Current workload:

- `tests/fixtures/*.input.ts`: `97` fixture files total
- `test_all_fixtures_compile`: `83` fixtures
- `test_all_fixtures_compile_with_builtins`: `85` fixtures
- `test_multi_file_fixtures_compile`: `1` multi-file fixture directory
- Total compile-check invocations per `cargo test --test compile_test`: `169`

Measured per-test durations:

| Test | Time |
| --- | ---: |
| `test_all_fixtures_compile_with_builtins` | `57.003s` |
| `test_all_fixtures_compile` | `105.094s` |
| `test_multi_file_fixtures_compile` | `105.450s` |

The key point is that only `3` Rust test cases exist here, but each one embeds a large internal loop that shells out to Cargo many times.

### 4. `e2e_test` is also structurally serialized

`tests/e2e_test.rs` serializes all E2E cases with `E2E_LOCK`, writes a fresh Rust runner source, executes `cargo run --quiet`, then executes `tsx` for the TypeScript oracle.

Relevant code:

- Shared runner + lock: `tests/e2e_test.rs:24-30`
- Forced mtime advancement to make Cargo notice each generated source write: `tests/e2e_test.rs:32-55`
- Per-test flow: transpile -> write -> `cargo run` -> `tsx`: `tests/e2e_test.rs:84-167`

Current workload:

- `tests/e2e_test.rs`: `174` test functions total
- Executed in normal runs: `132`
- Ignored in normal runs: `42`
- `cargo test --test e2e_test -- --test-threads=1`: `145.21s` test time, `145.61s` wall time

This means the E2E binary spends roughly `1.10s` wall time per executed E2E case on average (`145.61 / 132`), before any optimization work.

### 5. Unit and snapshot-heavy Rust tests are not the bottleneck

`cargo test --lib` finishes in `4.49s` despite running `3085` tests. The slowest unit tests are concentrated around builtin loading / real builtin fixtures, not general test-framework overhead.

Top unit-test outliers from `cargo test --lib`:

| Test | Time |
| --- | ---: |
| `pipeline::type_resolver::tests::expected_types::vec_methods::test_remapped_method_optional_param_is_not_propagated_as_expected` | `0.794s` |
| `external_types::tests::test_builtin_response_has_status_field` | `0.726s` |
| `pipeline::type_resolver::tests::expected_types::vec_methods::test_vec_filter_callback_with_real_builtins` | `0.666s` |
| `pipeline::type_resolver::tests::expected_types::vec_methods::test_remapped_method_required_fn_param_still_propagated` | `0.650s` |
| `pipeline::type_resolver::tests::expected_types::vec_methods::test_vec_push_expected_type_with_real_builtins` | `0.640s` |

Top integration-test outliers from `cargo test --test integration_test`:

| Test | Time |
| --- | ---: |
| `test_external_type_struct` | `0.825s` |
| `test_instanceof_builtin_with_builtins` | `0.785s` |
| `test_throw_new_error_string_literal_no_double_to_string` | `0.743s` |
| `test_string_methods_with_builtins` | `0.731s` |
| `test_vec_method_expected_type` | `0.639s` |

These are useful local targets, but even eliminating them entirely would not move the total suite nearly as much as fixing `compile_test` or `e2e_test`.

### 6. Tooling / environment notes

- `hyperfine` was not installed.
- `cargo-nextest` was not installed.
- `TODO` already contains `cargo nextest` as a future investigation item (`TODO:752`).
- E2E timing in the sandbox failed because `tsx` tried to create an IPC socket and hit `EPERM` on `/tmp/tsx-1000/*.pipe`; the final E2E timing above was collected with escalation to avoid that sandbox-only artifact.

### 7. Immediate implication for optimization work

Based on the current baseline, the first optimization pass should target only these two paths:

1. `tests/compile_test.rs`
2. `tests/e2e_test.rs`

Improving libtest scheduling alone will have limited payoff, because the dominant binaries are internally serialized and spend most of their time in repeated external process execution (`cargo check`, `cargo run`, `tsx`).

## References

- `Cargo.toml`
- `tests/compile_test.rs:18-21`
- `tests/compile_test.rs:69-97`
- `tests/compile_test.rs:179-206`
- `tests/compile_test.rs:220-289`
- `tests/compile_test.rs:304-403`
- `tests/e2e_test.rs:24-30`
- `tests/e2e_test.rs:32-55`
- `tests/e2e_test.rs:84-167`
- `tests/e2e_test.rs:221+`
- `TODO:752`
