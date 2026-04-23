# E2E Runner Isolation Design (2026-04-23)

## Summary

`tests/e2e_test.rs` の現行ボトルネック兼不安定要因は、全 test case が単一の `tests/e2e/rust-runner` を共有し、その `src/main.rs` と `src/*.rs` を上書きしながら `cargo run` を直列実行している点にある。

この設計を、**isolated runner pool** に置き換える。

方針:

1. 共有 `rust-runner` の代わりに、同一 test process 内で複数の runner instance を持つ
2. 各 runner instance は独立した `manifest_dir` と `target_dir` を持つ
3. test case は pool から runner lease を取得して実行し、完了後に返却する
4. これにより `E2E_LOCK` と `LAST_MTIME` を不要化する

## Current Problem

現行コードの shared mutable state:

- `tests/e2e_test.rs:24-30` `RUST_RUNNER_DIR`, `E2E_LOCK`
- `tests/e2e_test.rs:32-55` `LAST_MTIME`, `write_with_advancing_mtime`
- `tests/e2e_test.rs:84-167` single-file E2E の共有 runner 書き換え
- `tests/e2e_test.rs:274-361` multi-file E2E の共有 runner 書き換え

failure mode:

- 並列 test 実行で `src/main.rs` / `src/*.rs` が競合
- mtime workaround が必要になる時点で Cargo incremental build に対して不自然
- `E2E_LOCK` により並列性が完全に殺される
- TODO `I-173` の不安定性の根本原因候補と一致

## Target Design

### Runner Instance

各 runner instance は以下を持つ:

- `root_dir`: temp directory root
- `manifest_dir`: `root_dir/rust-runner`
- `target_dir`: `root_dir/target`

`manifest_dir` にはテンプレート `tests/e2e/rust-runner/` から必要ファイルだけをコピーする:

- `Cargo.toml`
- `Cargo.lock`
- `src/main.rs` (placeholder として初期化)

重要点:

- `target/` はテンプレートからコピーしない
- 各 runner instance は独自の `target_dir` を使う
- これにより incremental artifact も runner 単位で分離される

### Runner Pool

test process 内に `OnceLock<RunnerPool>` を置く。

`RunnerPool` は:

- `Vec<RunnerInstance>`
- `Mutex<Vec<usize>>` の available queue
- `Condvar`

を持つ。

test case は lease を 1 つ取得し、その lease の runner だけを書き換える。

lease が drop されると available queue に返却される。

### Pool Size

default:

- `min(available_parallelism, 4)`

override:

- env `TS_TO_RS_E2E_RUNNERS`

理由:

- 8 runner 以上にすると初回 compile が重くなりすぎる可能性がある
- まずは conservative default を使い、必要なら env で増減させる

### Single-file Flow

single-file E2E は現行と同じく:

1. TS source 読み込み
2. `transpile`
3. generated Rust を runner-local `src/main.rs` に書く
4. runner-local `cargo run --quiet`
5. `tsx` 実行
6. stdout/stderr 比較

ただし書き込み先は shared path ではなく lease が持つ runner-local path。

### Multi-file Flow

multi-file E2E も同様に runner-local `src/main.rs`, `src/<module>.rs` を使う。

shared `TempFile` cleanup ではなく runner-local file を都度上書き / 不要 module を cleanup する。

### Required Cleanup

multi-file test で前回の module file が残ると汚染が起こるため、runner instance ごとに:

- 実行前に `src/` 内の generated module files を cleanup
- `main.rs` は常に上書き

を行う。

`src/main.rs` 以外の管理ファイルは:

- テンプレート起源の固定ファイルは温存
- generated `*.rs` のみ cleanup

## Invariants

この変更で test infra が壊れていないと論理的に言うための不変条件は次。

1. 同時実行される 2 test case が同じ `manifest_dir` を共有しない
2. 同時実行される 2 test case が同じ `target_dir` を共有しない
3. single-file / multi-file ともに generated Rust の書き込み先が lease に閉じる
4. `E2E_LOCK` と `LAST_MTIME` に依存しない
5. TS oracle 比較ロジック (`compare_lines`, stdout/stderr assertions) は不変
6. fixture 読み込み元 (`tests/e2e/scripts/**`) は不変

## Verification Plan

### A. RED/GREEN for infrastructure invariants

まず helper-level test を追加する。

1. pool size resolution
2. parallel acquire で distinct runner path が返る
3. runner reset/cleanup が stale generated module を残さない

これらは pure infra verification なので、変換挙動に依存しない。

### B. Parallel smoke verification

2 つの simple E2E fixture を同時に実行する integration test を追加する。

狙い:

- shared runner 汚染が起きないことを小さく検証する
- lease 分離が本当に機能していることを black-box でも確認する

### C. Behavioral equivalence verification

変更後に以下を実施する。

1. `cargo test --test e2e_test -- --test-threads=1`
2. `cargo test --test e2e_test`
3. `cargo test --test e2e_test` を複数回繰り返す

判定基準:

- serial で pass
- parallel default で pass
- repeated parallel runs でも pass

これにより「旧来の deterministic path は保ちつつ、parallel でも壊れない」を確認する。

### D. Repository quality gate

最後に:

- `cargo fix --allow-dirty --allow-staged`
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `./scripts/check-file-lines.sh`

を実施する。

## Non-Goals

今回は以下は行わない。

- TS oracle cache
- `tsx` 常駐 worker 化
- E2E shard build / dispatcher 化
- compile_test の shard 化

今回は **isolated runner pool** に限定する。

## Expected Outcome

この変更が成功すると:

- `E2E_LOCK` を除去できる
- `LAST_MTIME` workaround を除去できる
- `cargo test --test e2e_test` を直列固定ではなく並列実行可能にできる
- `I-173` の shared runner 競合を構造的に解消できる

## Verification Results

実装後に以下を確認した。

### Helper-level invariants

- `cargo test --test e2e_test test_resolve_runner_pool_size_caps_to_four_by_default`
- `cargo test --test e2e_test test_runner_instance_paths_are_isolated_per_slot`
- `cargo test --test e2e_test test_cleanup_generated_runner_sources_removes_stale_modules_but_keeps_main`

いずれも pass。

### Black-box isolation smoke

- `cargo test --test e2e_test test_parallel_e2e_runner_isolation_smoke`
- `cargo test --test e2e_test test_e2e_hello_ts_rust_stdout_match`
- `cargo test --test e2e_test test_e2e_multi_import_basic_ts_rust_stdout_match`

いずれも pass。single-file / multi-file の両方で runner-local manifest が成立していることを確認。

### Full-suite behavior + stability

- serial: `cargo test --test e2e_test -- --test-threads=1` → pass, `real 193.70s`
- parallel run 1: `cargo test --test e2e_test` → pass, `real 135.58s`
- parallel run 2: `cargo test --test e2e_test` → pass, `136 passed / 42 ignored`, `finished in 135.01s`
- parallel run 3: `cargo test --test e2e_test` → pass, `136 passed / 42 ignored`, `finished in 117.89s`
- full repository: `cargo test` → pass; embedded `tests/e2e_test.rs` run also pass (`136 passed / 42 ignored`, `finished in 107.05s`)

### Quality gate

- `cargo fix --allow-dirty --allow-staged` → pass
- `cargo fmt --all --check` → pass
- `cargo clippy --all-targets --all-features -- -D warnings` → pass
- `cargo test` → pass
- `./scripts/check-file-lines.sh` → pass

## References

- `tests/e2e_test.rs:24-30`
- `tests/e2e_test.rs:32-55`
- `tests/e2e_test.rs:84-167`
- `tests/e2e_test.rs:274-361`
- `report/test-execution-baseline-2026-04-23.md`
- `TODO:680`
