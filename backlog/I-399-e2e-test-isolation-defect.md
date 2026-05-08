# I-399: E2E Test Isolation Defect (stale runner-pool binary leak)

## Background

`cargo test --test e2e_test` の実行が **test 実行順序により異なる cell set で fail する非決定性** を持つ test infra defect (= I-224 T6a 完了時 2026-05-08 empirical 発覚、`/check_problem` で root cause hypothesis 確立後新規起票)。

### Empirical evidence (2026-05-08、4 度 cargo test 実行で観測)

| 実行 mode | 失敗 cell 数 | 失敗 cell set |
|----------|-------------|--------------|
| parallel default (run 1) | 9 | i144_f4 / i144_r5 / i144_i025 / i050a / **i224_75** / **i224_77** / prd27_10 / switch_nonliteral / string_literal_enum |
| serial (--test-threads=1) | 8 | i142bc / i153 / i154 / i171_c7 / i205_09 / mixed_features / multi_import_basic / multivar_decl |
| parallel CARGO_INCREMENTAL=0 | 9 | i144_14 / i142bc / i050a / i171_c5 / i171_c5b / i171_c5c / i153 / method_args / method_chain |
| parallel default (run 4) | 9 | i144_i025 / i205_09 / i224_02 / i224_12 / i224_09 / nullish_coalescing / number_api / object_literal_inference / object_ops |

4 run 全てで **失敗 cell set が異なる** = test 実行順序 / scheduler timing に依存した非決定性。**1 cell も pass / fail が安定しない** = 全 e2e 結果が transient working-tree state、CI / dev 双方の信頼性に直接 impact。

### 直接症状 (cell-77 fail で empirical verify 2026-05-08)

cell-77 fixture の transpile output (= **生成 Rust source 本体は correct**):
```rust
struct main { id: f64 }
const LIT_VAL: f64 = 100.0;
fn compute() -> f64 { 42.0 }
fn main() {
    let m = main { id: 77.0 };
    let n = compute();
    println!("{} {} {} {}", "got", m.id, LIT_VAL, n);
}
```

Expected stdout (TS oracle): `got 77 100 42`  
Actual Rust stdout: `got 100 42` (cell-75 expected stdout の頭部、`from async main` 不在)

**Generated Rust と actual stdout が無関係** = test runner が `cargo run` で **stale binary を実行**。**isolation run (cell-77 単独 + --test-threads=1) では PASS** = full suite 実行時のみ発火 = MTBF 系 defect。

### 投資調査済み non-causes (empirical 2026-05-08)

| 候補 root cause | 検証方法 | 結果 |
|----------------|---------|------|
| (a) cargo incremental cache stale on slot reuse | `CARGO_INCREMENTAL=0 cargo test` で実行 | **REJECTED** (= 失敗 cell set 変化のみ、count 同じ ~9 件、incremental cache は root cause ではない) |
| (b) mtime granularity 不足 (= mtime collision で cargo がファイル変化を miss) | `stat --format` で mtime 解像度確認 | **REJECTED** (= ext4 nanosecond 解像度、`fs::write` で mtime advance 確認) |
| (c) naive sequential cargo run on same path | `/tmp/probe-cargo-cache.sh` で 3 round 同 path 上書き | **REJECTED** (= cargo は naive case で正しく rebuild、3 rounds 全て新出力検出) |
| (d) test isolation run | `cargo test --test e2e_test test_e2e_cell_i050a -- --test-threads=1` | **PASS** (= isolation 実行では失敗再現せず、full suite 実行時のみ発火) |

### Root cause hypothesis (Spec stage で empirical 確定 — multiple plausible mechanisms)

empirical investigation で simple cargo cache hypothesis は REJECT。残る hypothesis:

| 候補 | 詳細 | Plausibility |
|------|------|-------------|
| (H1) Cargo build invocation の rare race condition | 4 slots concurrent cargo invocations が global cargo cache (`~/.cargo/registry` lock 等) で contention、特定 timing で fingerprint 検出失敗 | Medium (= concurrent invocation は確かに global lock 経由するが、stale binary に至る具体 mechanism 不明) |
| (H2) cargo run --quiet が build error を suppress、stale binary execute | `--quiet` flag が cargo build の compile error を表示せず、build 失敗時に既存 stale binary を exec | Medium (= cargo 仕様上 build fail なら run も fail すべきだが、empirical で再現する余地あり) |
| (H3) fs::write 後の cargo source read race | Test A の cargo build が source read 中、Test A の lease drop + Test B の lease acquire + Test B の fs::write 介入 → cargo build が partial / stale source を read | Low (= Drop semantics 上、lease return は cargo run 完了後のみ、race 不発生) |
| (H4) cargo の incremental fingerprint algorithm bug under specific source content + size + path triple | 特定 content+path 組合せで fingerprint hash collision、stale fingerprint reuse | Low (= cargo bug の固有事例) |

**Spec stage 中の deep investigation で root cause を 1 つに絞り込み**、structural fix が全 hypothesis に対して bulletproof な design となることを verify する。

### 影響範囲 (= 本 entry を I-224 T7-T9 prerequisite に promote する根拠)

- **I-224 T9-1** が明示的に並列 e2e suite green を要求 (= 全 Axis C1 fixtures `cargo test --test e2e_test` で green)。本 defect 残存 = T9 review iteration が false-failure noise 汚染、structural prevention 必須
- **全 future PRD** の e2e empirical verification 信頼性に impact (= conversion correctness verification の base infra defect、universal infra leverage)
- I-224 T6a 完了時 plan.md baseline assertion「全 green / 0 fail」は実は本 defect 由来の偶発的成立、structurally false claim

## Problem Space (必須・最上位セクション)

### 入力次元 (Dimensions)

機能の出力 (= test pass/fail result) を決定する独立次元を列挙:

- **次元 A (test concurrency mode)**: parallel-default / parallel-with-CARGO_INCREMENTAL=0 / serial (--test-threads=1) / parallel-with-arbitrary-thread-count
- **次元 B (runner pool slot 再利用 pattern)**: slot 0 のみ reuse (single-thread or pool-size=1) / slot 0/1 round-robin / slot 0-3 hash-distributed (4-slot pool default) / slot 全空 → 全埋 sequential
- **次元 C (cargo build incremental cache state)**: warm cache (= 同一 source 既 build 済) / cold cache (= 新 source、deps cached) / fully-cold (= deps 含む新規 target dir)
- **次元 D (test の cargo invocation 形態)**: `execute_e2e` (= 1 lease 1 cargo run) / `run_cell_e2e_tests` (= 1 lease 連続 N cargo run、batched cells)
- **次元 E (test source content uniqueness)**: cell A と cell B の source byte-distinct (= 通常) / cell A と cell B の source byte-equal (= 同一 fixture 多重実行) / cell A と cell B の content same-size-different-bytes (= cargo's mtime+size fingerprint で確率的 collision 候補)

### 組合せマトリクス (= test infra defect 識別空間)

ideal output = **全 cell で test pass/fail が deterministic に source content に対応**。non-determinism 不在 = ideal。

| # | A (concurrency) | B (slot pattern) | C (cache state) | D (invocation 形態) | E (source uniqueness) | Ideal 出力 | 現状 | 判定 | Scope |
|---|---|---|---|---|---|-----------|------|------|-------|
| 1 | serial | single slot reuse | warm cache | execute_e2e | byte-distinct | source-A → output-A、source-B → output-B (deterministic) | sometimes stale (~3% rate) | ✗ | 本 PRD |
| 2 | serial | single slot reuse | warm cache | run_cell_e2e_tests | byte-distinct | 同上 | sometimes stale (i050a 再現) | ✗ | 本 PRD |
| 3 | serial | single slot reuse | cold cache | execute_e2e | byte-distinct | 同上 | deterministic (= 初回 cold compile が必ず rebuild trigger) | ✓ | regression lock-in |
| 4 | parallel-default | 4-slot round-robin | warm cache | execute_e2e | byte-distinct | 同上 | sometimes stale (~3% rate) | ✗ | 本 PRD |
| 5 | parallel-default | 4-slot round-robin | warm cache | run_cell_e2e_tests | byte-distinct | 同上 | sometimes stale | ✗ | 本 PRD |
| 6 | parallel-CARGO_INCREMENTAL=0 | 4-slot round-robin | warm cache (non-incremental) | execute_e2e | byte-distinct | 同上 | sometimes stale (= cargo incremental は root cause ではない empirical) | ✗ | 本 PRD |
| 7 | parallel-default | 4-slot round-robin | cold cache | execute_e2e | byte-distinct | 同上 | deterministic | ✓ | regression lock-in |
| 8 | parallel-default | 4-slot round-robin | warm cache | execute_e2e | byte-equal (= 同一 cell 二重実行) | source-A → output-A 二重 | deterministic (cache hit で同一 binary 実行 = 正しい) | ✓ | regression lock-in |
| 9 | parallel-default | 4-slot round-robin | warm cache | execute_e2e | same-size-different-bytes | content 不同 → output 不同 | candidate for cargo fingerprint stale (要 empirical) | 要調査 | 本 PRD scope (deep investigation で confirm) |
| 10 | parallel-default | 4-slot round-robin | warm cache | execute_e2e + previous test panic | byte-distinct | source-A → output-A | sometimes stale (= panic で lease drop が異常パスを通る場合 race risk あり) | 要調査 | 本 PRD scope |

判定凡例: ✓ (現状 OK) / ✗ (修正必要) / NA (unreachable, 理由付き) / 要調査 (Spec stage で empirical 確定)

判定 ✗ cells (1, 2, 4, 5, 6) = ideal output (deterministic) と現状 (~3% stale-binary rate) の divergence、本 PRD で structural fix。要調査 cells (9, 10) は Spec stage Self-Review TS-1 で empirical 確定。

### Spec-Stage Adversarial Review Checklist

`spec-stage-adversarial-checklist.md` 13-rule を `## Spec Review Iteration Log` section に転記して全項目 verification する。

## Oracle Observations (matrix-driven 不要、本 PRD は test infra defect)

本 PRD は **test infra defect** であり tsc / tsx の oracle 観察対象ではない (= conversion 機能ではなく test runner の決定性が問題)。代わりに **e2e_test.rs runner pool の empirical observation** を本 section に embed:

### Probe 1: cell-75 isolation vs full-suite (2026-05-08)

- **Isolation**: `cargo test --test e2e_test test_e2e_cell_i224_75 -- --test-threads=1` → **PASS** (35.32s)
- **Full suite parallel**: `cargo test --test e2e_test` (run 1) → **FAIL** (Rust stdout `1 2` instead of `got 100 42`)
- **Conclusion**: cell-75 の transpile output は correct、defect は full suite execution context で発火

### Probe 2: cargo cache naive case (2026-05-08、`/tmp/probe-cargo-cache.sh`)

```text
=== Round 1 (write v1, run) ===
v1 stdout: version-1
=== Round 2 (overwrite v2, run) ===
v2 stdout: version-2
=== Round 3 (overwrite v3 same-length-as-v1, run) ===
v3 stdout: version-X
=== Verdict ===
ALL PASS: cargo correctly rebuilt on each change. Stale-binary hypothesis REJECTED for naive case.
```

**Conclusion**: naive sequential cargo run with rapid main.rs overwrite IS correctly handled by cargo (= **naive case のみで REJECTED**、broad rejection ではない)。Defect mechanism は単純な cargo cache miss ではなく、より subtle な condition (= concurrent multi-thread cargo invocations / panic-recovery race / fingerprint algorithm edge case under specific source content + size + path triple / cargo's per-package fingerprint behavior under shared CARGO_TARGET_DIR not tested) で発火、TS-1 deep investigation で hypothesis を 1 つに絞り込む。

### Probe 3: TS-1 で deep investigation 予定の追加 probes

Spec stage で以下を実施し、root cause を確定:

- (TS-1-a) cargo build verbose output capture (= --verbose flag で "Compiling" message の有無を full suite 中で sample)
- (TS-1-b) instrumented runner: source SHA-256 + 実行 binary SHA-256 を log、divergence 検出
- (TS-1-c) panic-recovery race probe: 意図的に test panic 発生させ、lease drop + 次 test acquire の binary state 検証

## SWC Parser Empirical Lock-ins

本 PRD は SWC parser を touch しない (= test infra defect、parsing 経路と無関係) ため SWC parser empirical lock-in は **N/A**。

## Impact Area Audit Findings

### Pre-draft `_ => ` arm audit (Rule 11 (d-5))

```bash
python3 scripts/audit-ast-variant-coverage.py --files tests/e2e_test.rs tests/e2e/rust-runner/src/main.rs
```

**Note**: 上記 audit 対象 file は production code (= `src/`) ではなく test infra (= `tests/`)。`audit-ast-variant-coverage.py` の codebase scan は本 PRD scope (= test infra) と orthogonal、Rule 11 (d-1〜d-4) は production code SWC AST dispatch 限定 (= test code は対象外)。

**Decision**: Rule 11 audit は本 PRD scope と orthogonal、N/A 判定。`## Impact Area Audit Findings` section embed は本 PRD では空 (= violations 不在の意味で record)。

## Rule 10 Application

```yaml
Matrix-driven: no
Rule 10 axes enumerated:
  - test concurrency mode (parallel / parallel-CARGO_INCREMENTAL=0 / serial / arbitrary-thread-count)
  - runner pool slot 再利用 pattern (single-slot reuse / round-robin / hash-distributed)
  - cargo build incremental cache state (warm / cold / fully-cold)
  - test cargo invocation 形態 (execute_e2e / run_cell_e2e_tests batched)
  - test source content uniqueness (byte-distinct / byte-equal / same-size-different-bytes)
Cross-axis orthogonal direction enumerated: yes
Structural reason for matrix absence: infra で AST input dimension irrelevant
```

本 PRD は test infra defect であり、conversion 機能の AST input matrix ではなく **test infra failure mode matrix** (= 上記 5 axes Cartesian) を構築。Permitted reasons の `infra で AST input dimension irrelevant` 適用、Anti-pattern keyword 不在 (audit pass 想定)。

## Goal

`cargo test --test e2e_test` 実行が **test 実行順序 / concurrency mode / cargo cache state によらず deterministic に同一 result を返す** ことを保証する。

具体 verification:
1. **Level A determinism**: 同一 HEAD + 同一 source で `cargo test --test e2e_test` を 10 回連続実行、全 10 回が **完全に同一 result** (全 pass or 全 same fail)
2. **Cross-mode invariance**: parallel-default / serial / parallel-CARGO_INCREMENTAL=0 全 mode で同一 result
3. **Performance regression 0**: pre-fix `~150s` (full suite) と post-fix で structural に同等の実行時間 (= ±10% acceptable variance)

## Scope (3-tier 形式 hard-code)

### In Scope

- `tests/e2e_test.rs` の `E2eRunnerPool` / `E2eRunnerInstance` / `reset_single_file_main` / `execute_e2e_with_runner` を test isolation 保証する design に refactor
- 並列 / 直列 / cargo cache state 全 mode で deterministic test result 達成
- `tests/e2e/rust-runner/Cargo.toml` の structural change (= per-test unique bin / package name に対応する Cargo.toml mechanism)
- 既存 277 e2e tests が新 framework で動作することを verify (backward compatible regression lock-in)

### Out of Scope

- conversion logic の修正 (= production code `src/` は不変、本 PRD は test infra defect 専用)
- I-180 (E2E harness async-main multi-execution) の解消 — 別 PRD scope
- I-172 (Hono bench non-determinism) — 別 axis、別 PRD scope
- I-397 (auto-append detection edge cases) — 別 PRD scope
- cargo upstream の incremental cache algorithm 改修 — out of project scope

### Tier 2 honest error reclassify

本 PRD は test infra defect 解消 PRD であり、Tier 2 honest error reclassify 対象 features 不在。**N/A**。

## Invariants (test infra defect specific)

### INV-T1: Test execution determinism

- **(a) Property statement**: `cargo test --test e2e_test` を同一 HEAD + 同一 source で N 回連続実行、全 N 回が **完全に同一 result set** (全 pass / 全 same fail) を返す。
- **(b) Justification**: test 結果が non-deterministic な状態では、conversion regression と test infra defect を区別不能 = 全 PRD review iteration の信頼性 0、`ideal-implementation-primacy` 違反 (= 偽陽性/偽陰性 mixed test signal は理想実装の障害)。
- **(c) Verification method**: post-fix で `for i in {1..10}; do cargo test --test e2e_test 2>&1 | grep "test result"; done` を実行、全 10 行が同一 (pass count + fail count + ignored count + failed test name set 完全一致)。
- **(d) Failure detectability**: post-fix で N 回実行間に diff 検出 = invariant 違反、CI fail で detect。

### INV-T2: Cross-mode invariance

- **(a) Property statement**: parallel-default / parallel-with-CARGO_INCREMENTAL=0 / serial (--test-threads=1) / 任意 thread count の全 mode で同一 test result。
- **(b) Justification**: mode 差で結果が変わるなら test isolation が破綻 = test 結果が runtime context に依存 = empirical verification が不能。
- **(c) Verification method**: 上記 4 mode を順に実行、全 4 mode で同一 result set。
- **(d) Failure detectability**: mode 間 result diff 検出 → invariant 違反。

### INV-T3: Performance regression bound

- **(a) Property statement**: post-fix の `cargo test --test e2e_test` 実行時間が pre-fix 平均値 (~150s) から ±10% 以内 (= 135s〜165s)。
- **(b) Justification**: structural fix が performance regression を導入すると CI / dev workflow 体験が悪化、bug fix の structural 意義が損なわれる。
- **(c) Verification method**: pre-fix 5 回 + post-fix 5 回実行、平均値 diff ≤ 10%。
- **(d) Failure detectability**: 実行時間 diff > 10% → INV-T3 違反、design rework。

## Design

### Technical Approach

**Structural fix design (Option H = per-test content-derived binary identity)**:

current design の根本 fragility = 「複数 test が **同一 path (`src/main.rs`)** に source を上書き」 = cargo's fingerprint cache key (path-based) で path collision = 様々な mechanism で stale fingerprint reuse 可能性。

fix の structural insight: **path collision を構造的に排除** することで、cargo の incremental cache mechanism が何であろうと cache reuse は **content-equivalent な場合のみ発生** (= 正しい binary が再利用、stale binary は混入不能)。

#### Mechanism: per-test content-hash-derived bin name

```rust
// Per-test execution flow:
// 1. Compute hash = sha256(rs_source) truncated to first 12 chars (e.g., "a3f9c2b8e4d1")
// 2. Write rs_source to runner_manifest_dir/src/<hash>.rs
// 3. Cargo.toml has dynamic [[bin]] entry: [[bin]] name="<hash>" path="src/<hash>.rs"
//    - 既存 [[bin]] 一覧 (per slot) を session 中蓄積
// 4. cargo run --bin <hash> with shared CARGO_TARGET_DIR
//    - cargo はその bin name の fingerprint を独立に track、別 bin と collision 不能
//    - 同一 hash = byte-equal source = 既 build 済 binary 再利用 (correct)
//    - 異 hash = 新 source = cargo は構造的に新 binary を build (stale 不能)
// 5. parse stdout/stderr from cargo run output
```

cargo's per-bin fingerprint key: `(manifest_path, bin_name, profile, ...)`. 同 manifest 内に複数 [[bin]] = 各 bin 独立 fingerprint。Path-based collision を構造的に排除。

### Design Integrity Review

Per `.claude/rules/design-integrity.md` checklist:

- **Higher-level consistency**: 既存 E2eRunnerPool の slot 概念は維持 (= 並列性確保)、各 slot 内で per-test content-hash-derived bin に refactor。pipeline integrity (parser → transformer → generator) は test layer のみ touch、production code 無関係。
- **DRY / Orthogonality / Coupling**: per-test bin 管理を `RunnerInstance::run_with_source(rs_source)` 1 method に集約 (= source hash 計算 + Cargo.toml [[bin]] append + src/<hash>.rs write + cargo run の atomic operation)、外部 caller (`execute_e2e_with_runner`) からは hash 詳細隠蔽。Coupling 増加なし、抽象 cohesion 向上。
- **Broken windows**: 現状 `reset_single_file_main` は cleanup_generated_runner_sources で他 .rs files を削除する logic を持つが、本 fix では複数 .rs files が共存する design に切替、cleanup logic は session end (E2eRunnerPool drop) に移動。`Cargo.toml` mutation logic は新規必要 (= existing が静的 read-only).

Verified, structural improvements only, regression cell 0。

### Impact Area

```bash
# Empirical file path verify (Rule 3-pre):
# - tests/e2e_test.rs:307-374 (E2eRunnerPool / E2eRunnerInstance / acquire / Drop)
# - tests/e2e_test.rs:99-117 (reset_single_file_main / reset_multi_file_sources)
# - tests/e2e_test.rs:431-475 (execute_e2e_with_runner cargo run invocation)
# - tests/e2e/rust-runner/Cargo.toml (per-test [[bin]] mechanism サポート用 structural change)
# - tests/e2e/rust-runner/src/main.rs (template stub、本 PRD で per-cell-derived path に置換)
```

すべて test layer (`tests/`)、production code (`src/`) 不変。

### Semantic Safety Analysis

本 PRD は test infra defect 解消であり、type fallback / type approximation / type resolution behavior 改修を含まない。

**N/A — no type fallback changes**

## Spec Stage Tasks

### TS-0: Cartesian product matrix completeness verification

- **Work**: Problem Space matrix の 5 axes Cartesian product (= 4 × 4 × 3 × 2 × 3 = 288 cells) の reachable subset を完全 enumerate、abbreviation pattern 排除 (現 matrix table 10 cells は representative subset = Rule 1 (1-2) 違反)、各 cell に判定付与
- **Completion criteria**: matrix table 全 reachable cell 独立 row、`audit-prd-rule10-compliance.py` PASS

### TS-1: Deep investigation で root cause 確定

- **Work**: 以下 3 probes を実施し empirical で root cause 確定:
  - **(TS-1-a)** cargo build verbose probe: `RUSTC_LOG=info cargo run --verbose` を runner で代替実行、failing test sequence 中の "Compiling" message 有無を log
  - **(TS-1-b)** instrumented runner probe: source SHA-256 + 実行 binary SHA-256 を log、divergence 検出 (= source change but binary same → cache miss = stale-binary mechanism specific)
  - **(TS-1-c)** panic-recovery race probe: 意図的 panic 発生 + 直後の test を観察、lease drop の異常 path verify
- **Completion criteria**: 4 候補 hypothesis (H1〜H4) のうち 1 つ確定 or 「multi-mechanism (複数 root cause 同時発火)」確定、Spec doc に embed
- **Deliverable**: `report/I-399-root-cause-investigation.md` (root cause 確定 + structural fix design rationale)

### TS-2: Structural fix design empirical verify

- **Work**: per-test content-hash-derived bin design の prototype (`/tmp/i399-prototype/`) を構築、以下を verify:
  - 同一 hash の bin (= byte-equal source) は cargo cache 再利用 (= 2 nd 実行が 1st と同一 binary path 指す)
  - 異 hash の bin は cargo 必ず fresh build
  - shared CARGO_TARGET_DIR 内で複数 bin が orthogonal に共存 (= 1 bin の build が他 bin に影響しない)
  - 4 slots concurrent invocation で test isolation 維持
- **Completion criteria**: prototype が 100 round 連続実行で deterministic (= INV-T1 prototype 検証)、performance pre-fix 同等 (INV-T3 prototype 検証)

### TS-3: E2E fixture creation (red 状態) — N/A for test infra PRD

本 PRD は test infra defect、e2e fixture (= TS source) 不要。代わりに **integration test** で structural fix を verify:

- `tests/i399_isolation_test.rs` (新規) — pool 動作の deterministic 性を direct verify する integration test (10 round same-suite 実行 + result diff 検出)

### TS-4: Impact Area audit findings record

- **Work**: 上記 3-pre-2 で record 済 (= Rule 11 audit は test infra PRD で N/A、`## Impact Area Audit Findings` section に明記)
- **Completion criteria**: section 既 embed、TS-4 verify は本 doc 内 audit cross-check で完了

## Implementation Stage Tasks

(TDD 順、Spec stage 完了 + user 承認後着手)

### T1: per-test content-hash-derived bin design 実装

- **Work**: `tests/e2e_test.rs::E2eRunnerInstance` に `run_with_source(rs_source: &str) -> RunnerOutput` method 新設:
  - SHA-256 hash 計算 + first 12 chars truncate
  - Cargo.toml `[[bin]]` entry 動的 append (atomic + idempotent)
  - `src/<hash>.rs` write
  - `cargo run --bin <hash>` invoke
  - stdout / stderr capture
- 旧 `reset_single_file_main` API を deprecate / remove (= caller 全 site を `run_with_source` に migrate)
- **Completion criteria**: 単独 unit test pass + integration test (= TS-3 で起票) 1 round green、cargo build pass

### T2: Cargo.toml mechanism 改修

- **Work**: `tests/e2e/rust-runner/Cargo.toml` から default `[[bin]]` 削除、initial state は `[package]` + `[dependencies]` のみ。Runner pool initialization で `[[bin]]` を session 中動的 append (slot ごと)。
- **Completion criteria**: cargo build pass (initial empty Cargo.toml + 0 [[bin]] entries で warning 不発)

### T3: Backward compatibility verify (= 既存 277 tests が全 pass) + INV-T1/T2/T3 lock-in

- **Work**: `cargo test --test e2e_test` を以下 4 mode で各 5 round 実行:
  - parallel-default
  - parallel-CARGO_INCREMENTAL=0
  - serial (--test-threads=1)
  - parallel-with-thread-count=8
- 全 mode × 5 round で同一 result (INV-T1 + INV-T2 lock-in) + 平均実行時間が pre-fix ±10% (INV-T3 lock-in)
- INV-T1/T2/T3 を `tests/i399_isolation_test.rs` 内 invariants test として fill-in (= scripted 10-round test)
- **Completion criteria**: 全 4 mode × 5 round で deterministic、INV invariants tests green、`/check_job` 4-layer review pass

### T4: PRD close / chain 更新

- **Work**: `/check_job` 4-layer review final pass + plan.md / TODO update + I-224 T7 chain 再開準備
- **Completion criteria**: I-399 PRD close、I-224 T7 が信頼可能 e2e empirical で進行可能

## Spec Review Iteration Log

### Iteration v1 (2026-05-08、本 PRD draft 初版 self-applied verify)

skill workflow Step 4.5 で 13-rule self-applied verify 実施:

| Rule | Sub-rule check | Verdict | Notes |
|---|---|---|---|
| 1 | (1-1) 全 cell ideal output | partial | 10 cells subset enumerate、TS-0 で全 reachable 288 cells enumerate 必要 |
| 1 | (1-2) abbreviation pattern 不在 | partial | 現 matrix で representative subset = Rule 1 (1-2) 違反、TS-0 で完全 enumerate |
| 1 | (1-3) audit script PASS | TS-4 で実施 | 本 draft commit 後 `audit-prd-rule10-compliance.py` |
| 1 | (1-4) Orthogonality merge | N/A | 5 axes 全 independent dispatch、merge 不在 |
| 2 | (2-1) Oracle grounding | ✓ | empirical evidence section に 4 run の result + probe 1/2 結果 embed |
| 2 | (2-2) `## Oracle Observations` section | ✓ | section 存在、test infra empirical observations を embed |
| 2 | (2-3) audit script verify | TS-4 で実施 | section 不在は audit fail |
| 3 | (3-1) NA spec-traceable | N/A | NA cells 不在 (matrix 5 axes Cartesian で physical reachable = all) |
| 3 | (3-2) SWC parser empirical | N/A | 本 PRD は SWC parser non-touch |
| 3 | (3-3) SWC accept reclassify | N/A | 同上 |
| 4 | (4-1) reference doc 整合 | N/A | conversion grammar 改修なし |
| 4 | (4-2) doc-first dependency | N/A | doc 改修不要 |
| 4 | (4-3) audit verify | N/A | 同上 |
| 5 | (5-1) E2E fixture 準備 | partial | TS-3 で integration test 起票、e2e fixture (= TS source) 不要 |
| 5 | (5-2) Spec/Implementation stage 2-section split | ✓ | `## Spec Stage Tasks` + `## Implementation Stage Tasks` 両 section 存在 |
| 6 | (6-1) Matrix Ideal output ↔ Design 一致 | ✓ | matrix Ideal = "deterministic" を Design Approach の per-test content-hash-derived bin 設計と token-level 一致 |
| 6 | (6-2) Scope 3-tier hard-code | ✓ | `In Scope` / `Out of Scope` / `Tier 2 honest error reclassify` 3 sub-section、Tier 2 = N/A explicit |
| 7 | Control-flow exit sub-case | N/A | 本 PRD は test infra、control-flow body shape dimension 不在 |
| 8 | (8-5) `## Invariants` 独立 section | ✓ | INV-T1 / INV-T2 / INV-T3 各 4 項目 (a)(b)(c)(d) 記載 |
| 9 | (a) Spec→Impl Dispatch Arm Mapping | partial | 本 PRD は dispatch tree 不在 (test infra refactor)、N/A 候補だが TS-1 root cause 確定後 mapping 表 verify |
| 10 | Cross-axis matrix completeness | partial | 5 axes enumerate 済 (= 上記 Rule 10 Application yaml)、TS-0 で Cartesian completeness verify |
| 11 | (d-1〜d-4) AST node enumerate | N/A | test infra、production code SWC AST 改修なし |
| 11 | (d-5) `## Impact Area Audit Findings` section | ✓ | section 存在、Rule 11 N/A の structural reason 明記 |
| 12 | (e-1〜e-8) Mandatory + structural | ✓ | `## Rule 10 Application` section 記入 + Permitted reasons 適用 + Anti-pattern keyword 不在 |
| 13 | (13-1) Self-Review skill workflow Step 4.5 | ✓ | 本 section 自身 |
| 13 | (13-2) `## Spec Review Iteration Log` record | ✓ | 本 section |

**Iteration v1 findings count (self-claim、本 draft commit 後 audit script で empirical verify)**: ~~Critical = 0 (Implementation block するもの不在、partial verify items は TS-0/TS-1/TS-2/TS-4 で完成予定)、High = 4~~ → **/check_job 2nd-round adversarial review 2026-05-08 で false-positive classification と判定、re-classify**: Critical = 1 (TS-0 未済 = matrix 10 cells representative subset = Rule 1 (1-2) abbreviation pattern 違反 = Spec stage 移行 block) + High = 1 (TS-3 integration test recursive invocation 回避設計不在) + Medium = 4 (F2/F3/F6/F8 = TS-0〜TS-2 で empirical verify する conditional risks、Iteration v2 で resolve 予定) + Low = 4 (F5/F10/F11/F12 = wording 訂正 + cleanup strategy session boundary spec 改修)。

**Iteration v1 Spec stage 完了判定**: ~~Spec stage approved~~ → **Spec stage 移行 block** (= Critical 1 + High 1 が解消されるまで Implementation stage 着手不可、TS-0〜TS-4 完了後 Iteration v2 で 13-rule self-applied verify 全項目 ✓ 達成判定)。

**Self-review false-positive lesson (= I-D batch v11-8 candidate へ feedback、本 PRD self-applied integration)**: Iteration v1 が "partial" verify result を High と classify したが、rule per Rule 1 (1-2) abbreviation pattern 違反は Critical = Spec stage 移行 block 該当 = severity default が unclear。framework rule 改善候補として `spec-stage-adversarial-checklist.md` Rule 13 に "**Pending verdict severity default = Critical**" sub-rule 追加 (= "TS-X 後 verify" pending state は Implementation stage 移行 block する severity を default 適用、partial classification を High と claim する false-positive を構造的に prevent)。本 finding は I-D batch v11-8 candidate として正式 record 予定。

## Test Plan

### Integration tests (新規 `tests/i399_isolation_test.rs`)

**Recursive invocation guard (F7 fix、2nd-round adversarial review 由来 2026-05-08)**: tests 1-3 は subprocess で `cargo test --test e2e_test` を invoke するが、`tests/i399_isolation_test.rs` 自身が含まれる test binary を再帰呼び出しする risk あり。**回避設計**:

- subprocess invocation は `cargo test --test e2e_test -- --skip i399_isolation_test` flag を必須化 (= 自己再帰防止、`tests/e2e_test.rs` のみを実行)
- 10 round → **CI 上では 1 round 削減** (= dev local のみ deep 10 round 実行可能、CI 実行時間爆発回避)、env var `I399_DEEP_VERIFY=1` で deep mode opt-in
- 各 round 実行時間 ~150s + overhead = CI 上 1 round = ~3 min、dev deep mode 10 round = ~25 min (= dev only acceptable)
- INV-T3 performance bound verify は別 invocation pattern (= 別 test fn) で 5 round subprocess、平均値計算

#### Test list

1. `test_invariant_t1_test_execution_determinism` — `cargo test --test e2e_test -- --skip i399_isolation_test` を subprocess で N round 実行 (CI: 1 round / dev deep: 10 round)、全 result 完全一致 assert
2. `test_invariant_t2_cross_mode_invariance` — parallel-default / serial / CARGO_INCREMENTAL=0 / thread-count=8 各 mode 1 round 実行 (= mode 4 × 1 round = ~10 min)、cross-mode result 完全一致 assert
3. `test_invariant_t3_performance_regression_bound` — pre-fix baseline (= TS-2 prototype empirical 確定、現状 150s 仮値) ±10% 以内に post-fix が収まること、5 round 平均値 assert (TS-2 で baseline 確定後 INV-T3 fill-in)
4. `test_per_test_content_hash_isolation` — same hash → cargo cache reuse (binary mtime 不変)、different hash → fresh build (binary mtime 更新) を direct verify (= subprocess `cargo test` を invoke せず、`tests/e2e_test.rs::E2eRunnerInstance::run_with_source` を直接 unit test 形式で probe)

### Backward compatibility tests (existing 277 e2e tests)

`cargo test --test e2e_test` 全 pass を Implementation T3 で verify、INV-T1/T2/T3 lock-in。

## Completion Criteria

1. **INV-T1 (Test execution determinism)** lock-in: `cargo test --test e2e_test` 10 round 連続実行が deterministic
2. **INV-T2 (Cross-mode invariance)** lock-in: 4 mode 全て deterministic + cross-mode 同一 result
3. **INV-T3 (Performance regression bound)** lock-in: pre-fix ±10% 以内
4. **既存 277 e2e tests 全 pass** (regression 0)
5. `/check_job` 4-layer review で Layer 1-4 全 0 findings
6. `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
7. `cargo fmt --all --check` 0 diffs
8. `./scripts/check-file-lines.sh` 全 file < 1000 行
9. plan.md / TODO chain order update (= 案 β Phase 1-A の I-224 T7 prerequisite に I-399 を確定 insert)

**Hono bench impact**: 本 PRD は test infra 専用、production code (`src/`) 0 行変更、Hono bench への conversion 影響不在 = **Preservation classification** per `prd-completion.md` Tier-transition compliance (= Tier-transition compliance wording の 1 classification として Preservation を適用、production code unchanged で Hono bench 数値 / errors count いずれも pre/post 完全一致 expected)。

## References

- 関連 PRD: I-224 (T9 prerequisite block 由来、本 PRD direct beneficiary)、I-180 (E2E harness async-main multi-execution、test infra cluster sister concern)、I-172 (bench non-determinism、別 axis の同 type defect)、I-397 (e2e harness `should_auto_append_main_call` detection edge cases、test infra cluster sister)
- 関連 rule: `.claude/rules/spec-first-prd.md` (matrix-driven 適用判定で non-matrix 確定、ただし全 PRD で問題空間 enumerate / Rule 10 application 必須) / `.claude/rules/spec-stage-adversarial-checklist.md` (13-rule) / `.claude/rules/check-job-review-layers.md` (4-layer review) / `.claude/rules/problem-space-analysis.md` / `.claude/rules/ideal-implementation-primacy.md` / `.claude/rules/prd-completion.md`
- 関連 doc: tests/e2e_test.rs (本 PRD scope file、structural refactor target) / tests/e2e/rust-runner/Cargo.toml (mechanism 改修)
- 関連 handoff: なし (新 PRD、handoff doc 未生成)
- discovery date: 2026-05-08 (PRD I-224 T6a 完了時 Quality Gate 確認で empirical 観測、`/check_problem` で root cause hypothesis 確立 + I-180 entry 関連 PRD list 内未起票 I-173 を replacement で正式起票)
- iteration history: **v1 (initial draft 2026-05-08、Spec stage 移行 block = Critical 1 件 (TS-0 未済 abbreviation pattern 違反) + High 1 件 (TS-3 recursive invocation 回避設計不在) + Medium 4 件 (TS-0〜TS-2 で empirical verify する conditional risks) + Low 4 件 (wording 訂正類)、2nd-round /check_job adversarial review 2026-05-08 で false-positive 再 classify 完了、Iteration v2 で 13-rule self-applied verify 全 ✓ 達成判定予定)**
- user 承認: 2026-05-08 (Option A: I-399 を I-224 T7 前 prerequisite として先行 + Level A determinism + regression 許容なし + Option 1+2 cohesive batch + 深い investigation 全推奨採択)
