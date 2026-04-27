# I-184: CI fresh-clone defect — gitignored template files referenced by `fs::copy` / `fs::write`

## Stage Judgment (Step 0a)

**Non-matrix-driven** (CI infra / build system fix)。conversion 機能ではないため `spec-first-prd.md` 2-stage workflow + 10-rule adversarial checklist は適用外。Step 0b problem space analysis + Step 3 impact area review + 4-layer review (Layer 1 + Layer 4 必須、Layer 2-3 optional) は実施。

## Background

CI run `2026-04-26T14:33:53` (job-logs `867df6b5-52cd-5751-98d9-0bf40e8851a7`) が `tests/compile_test.rs` 3 test 全 fail で exit 101。

```
thread 'test_all_fixtures_compile_with_builtins' (6204) panicked at tests/compile_test.rs:90:29:
failed to write tests/compile-check/src/lib.rs: No such file or directory (os error 2)
```

### 歴史的経緯 (なぜ git untrack だったか)

- **commit 4a52fd4 (Pre-I-144 cleanup, 2026-04-19)** で I-145 + I-161 が並行適用され、`tests/compile-check/src/lib.rs` および `tests/e2e/rust-runner/src/main.rs` を **「write-only artifact」分類で gitignored 化**。当時の design では:
  - compile-check: `compile_test` が template `src/lib.rs` を毎 test `fs::write` で上書き → working tree pollution 防止のため gitignore は **正しい判断**
  - e2e/rust-runner (当時): `RUST_RUNNER_DIR` 1 個の単一 directory を全 E2E test で共有、`static E2E_LOCK: Mutex<()>` で serialize、各 test が template `src/main.rs` を直接 `write_with_advancing_mtime` (内部 `fs::write`) で上書き → 同様に gitignore は **正しい判断**
- **commit 24a14b7 (テスト基盤強化、後続)** で **e2e/rust-runner のみ pool refactor が入った**:
  - `RUST_RUNNER_DIR` → `RUST_RUNNER_TEMPLATE_DIR` (rename signal)
  - `static E2E_LOCK` 廃止、`E2eRunnerPool` 導入
  - 各 test は **per-runner temp dir** に write、template は **`fs::copy` 読込み元** に変化
  - **template `src/main.rs` は test 実行で不変化** = I-161 当時の「write-only artifact」根拠は **消滅**
  - しかし pool refactor で doc comment は更新された一方、**`.gitignore` entry は更新されず** stale 化
- **結果 (latent defect)**: e2e/rust-runner template の `src/main.rs` および `Cargo.lock` は gitignored (= read source として absent 可能) だが test code が `fs::copy` で **読込み源** とするため、fresh-clone state で `os error 2` で panic
- **CI 顕在化遅延**: `compile_test` (cell #3 panic) が `e2e_test` より先に走行し exit 101 で同 process 終了するため、e2e_test 側 latent defect が CI log に surface しなかった

### 本 PRD scope

`/check_job` Layer 4 trade-off review で 4 件 ✗ cell を網羅 enumerate (cells #3, #4, #7, #8)。compile-check は **template-buffer pattern** (元の I-145 design 維持)、e2e/rust-runner は **pool pattern** (post-refactor の access pattern を正しく反映) として **asymmetric** な ideal design を採用。

## Problem Space (Step 0b)

CI fresh-clone state における「test code が template subproject 内 file を参照する箇所」の問題空間を網羅 enumerate する。

### 入力次元

- **次元 A (Subproject)**: `tests/compile-check/` / `tests/e2e/rust-runner/`
- **次元 B (File)**: `Cargo.toml` / `Cargo.lock` / `src/<entry>.rs` / `src/<module>.rs` (multi-file generated) / `target/`
- **次元 C (Test access pattern)**: `fs::write` to template (overwrite) / `fs::copy` from template (read) / `fs::write` to per-runner temp dir / cargo internal regen / not accessed
- **次元 D (Git tracking state)**: tracked / gitignored / auto-generated at runtime

### 組合せマトリクス

直積を全 enumerate (NA 含む)。

| # | Subproject | File | Access pattern | Git state (Pre I-184) | Fresh-clone behavior (Pre) | Ideal | Git state (Post I-184) | 判定 | Scope |
|---|-----------|------|----------------|----------------------|---------------------------|-------|------------------------|------|-------|
| 1 | compile-check | `Cargo.toml` | Read by `cargo check` (current_dir) | Tracked | exists | exists | Tracked (unchanged) | ✓ | — |
| 2 | compile-check | `Cargo.lock` | Auto-regen by `cargo check` | Gitignored | absent → cargo regen ok | tracked (reproducibility 保証) | **Tracked** | ✗ → ✓ | 本 PRD |
| 3 | compile-check | `src/lib.rs` | `fs::write` (template 直接 overwrite、毎 test) | Gitignored (entry 名指定) | absent → fs::write fails (parent dir 不在) | gitignored (毎 test 上書きで working tree 汚染) + parent dir 保持 | Gitignored (`*.rs`) | ✗ → ✓ | 本 PRD |
| 4 | compile-check | `src/` directory | parent dir for `fs::write` | Untracked | absent → fs::write fails | `.keep` で tracked | **Tracked (`.keep`)** | ✗ → ✓ | 本 PRD |
| 5 | compile-check | `target/` | cargo internal | Gitignored | absent → cargo creates | absent ok | Gitignored (unchanged) | ✓ | — |
| 6 | e2e/rust-runner | `Cargo.toml` | `fs::copy` (read source、pool init) | Tracked | exists | exists | Tracked (unchanged) | ✓ | — |
| 7 | e2e/rust-runner | `Cargo.lock` | `fs::copy` (read source、pool init) | Gitignored | absent → fs::copy fails (`os error 2`) | tracked (reproducibility + read source) | **Tracked** | ✗ → ✓ | 本 PRD |
| 8 | e2e/rust-runner | `src/main.rs` | `fs::copy` (read source、pool init) | Gitignored (entry 名指定) | absent → fs::copy fails | tracked stub (`fn main() {}`、template skeleton として valid Cargo project 化) | **Tracked stub** | ✗ → ✓ | 本 PRD |
| 9 | e2e/rust-runner | `src/<module>.rs` (multi-file generated) | `fs::write` to per-runner temp | NA (per-runner temp は git tracked 外) | per-runner temp dir で生成 | per-runner temp で生成 | NA | NA | — |
| 10 | e2e/rust-runner | `src/` directory (template) | dir のみ存在保証 (cell #8 main.rs が dir を non-empty 化) | Untracked | absent → fs::copy fails (cell #8 起因) | cell #8 の tracked main.rs が dir を保持 | Implicitly tracked | ✗ → ✓ | 本 PRD |
| 11 | e2e/rust-runner | `target/` | cargo internal (per-runner temp) | Gitignored | absent → cargo creates | absent ok | Gitignored (unchanged) | ✓ | — |
| 12 | e2e/rust-runner | `node_modules/` (tests/e2e/) | tsx 経由 | Gitignored | CI で `npm install` step 実行 | step で生成 | Gitignored (unchanged) | ✓ | — (workflow 既存対応) |

凡例: ✓ (現状 OK) / ✗ (修正必要) / NA (unreachable) / 要調査 (Discovery で解消)

### Matrix Completeness Audit

- [x] 全 subproject (2 件) 列挙済
- [x] 全 file 種別 (Cargo.toml / Cargo.lock / .rs / target / node_modules) 列挙済
- [x] 全 access pattern (write / copy / regen / 非参照) 列挙済
- [x] 全 git state (tracked / gitignored / NA) 列挙済
- [x] 全直積 cell に判定付与済 (空 cell 0)
- [x] NA cell に理由記載済 (cell #9: per-runner temp は git scope 外)
- [x] ✗ cell が `本 PRD scope` か `別 PRD` かの判定付与済
- [x] 各 subproject の **access pattern (template-buffer vs pool)** に応じて asymmetric な ideal を選定 (cells #3/#4 は template-buffer、cells #6-#10 は pool)

✗ cell 4 件 (`#3, #4, #7, #8`) を全て本 PRD で structural fix。

## Goal

`actions/checkout@v6` 直後の fresh-clone state で `cargo test --test compile_test --test e2e_test` を実行し、3 + 157 = 160 test 全 pass、ignored は既存 29 件 (本 PRD 対象外) のみとする。さらに以下も成立:
- `cargo generate-lockfile` を両 subproject template directory で **temp stub 作成なしで** 実行可能 (e2e/rust-runner template が valid Cargo project)
- IDE / rust-analyzer が両 subproject template を valid Cargo project として load 可能

## Scope

### In Scope

1. **Cells #2, #7 (両 Cargo.lock)** structural fix: 両 subproject で git tracked 化 (binary / library 区別を超えた reproducibility 保証 + e2e の `fs::copy` 読込み元として動作)
2. **Cells #3, #4 (compile-check src/lib.rs / src/ dir)** structural fix: template-buffer pattern を維持し `.keep` で `src/` dir tracked + `*.rs` ignore で生成 file 除外
3. **Cells #8, #10 (e2e/rust-runner src/main.rs / src/ dir)** structural fix: post-pool-refactor の access pattern (template = read-only skeleton) を反映し **`src/main.rs` を tracked stub 化** + `src/*.rs` ignore entry 削除 + pool init を `copy_runner_template_file("src/main.rs", ...)` 統一形に統一
4. `tests/e2e_test.rs:593` の outdated doc comment 修正 (post-pool-refactor で template `src/` に書込みしないため、現在の per-runner temp dir 記述に更新)
5. `.gitignore` のコメント更新: 各 subproject の access pattern (template-buffer vs pool) を明記し、なぜ asymmetric な handling が ideal なのかを future maintainer のために記録
6. fresh-clone state での verification protocol を Test Plan に明記

### Out of Scope

- `copy_runner_template_file` 関数の rename (現状 Cargo.toml / Cargo.lock / src/main.rs 3 file を copy する pool init helper として正しく機能)
- compile-check も pool pattern に refactor (test 数が 3 件で `Mutex<()>` serialize で十分、ROI 低い、別 PRD 化候補だが本 PRD scope 外)
- compile-check / e2e/rust-runner の Cargo.toml dependency unification (既存 NOTE で in-sync 運用、本 PRD 対象外)
- workflow `.github/workflows/ci.yml` の fresh-clone simulation step 追加 (既存 `actions/checkout@v6` 自体が fresh-clone 相当のため不要)

## Design

### Technical Approach

#### Core insight: 各 subproject の access pattern が異なるため asymmetric handling が ideal

Pre-PRD で gitignored 化されていた file 群を「symmetric に extension-ignore + `.keep`」で統一しようとすると、**post-pool-refactor で消滅した write-only artifact 前提を復活させる anti-pattern** に陥る。各 subproject の access pattern を honest に反映する asymmetric design が ideal。

| Subproject | Pattern | Template `src/<entry>.rs` の本質 | Ideal handling |
|------------|---------|---------------------------------|----------------|
| compile-check | **template-buffer pattern**: `compile_test` が template 直下 `src/lib.rs` を `fs::write` で毎 test 上書き (`Mutex<()>` で serialize) | 真の write-only artifact (毎 test 上書き) | gitignore (`*.rs`) + `.keep` で dir 保持 |
| e2e/rust-runner | **pool pattern** (`E2eRunnerPool`): 各 test は per-runner temp dir に write、template は pool init での `fs::copy` 読込み元のみ | read-only skeleton (test 実行で不変) | tracked stub (template が valid Cargo project) |

#### Approach 比較

| Approach | Cargo.lock | e2e src/main.rs | 評価 |
|----------|-----------|----------------|------|
| (現状 Pre-PRD) | Gitignored (両) | Gitignored (両) | latent defect: fresh clone で `fs::copy` fails |
| **A** (採用): tracked Cargo.lock + asymmetric src/ | **Tracked (両)** | **Tracked stub (e2e のみ)** + Gitignored (compile-check) | 各 subproject の access pattern を正しく反映、template を valid Cargo project 化、pool init code 統一 |
| B: tracked Cargo.lock + symmetric `.keep` | Tracked (両) | Gitignored (`*.rs`) + `.keep` (両) | symmetric だが e2e 側は **vestigial defensive coding** (post-pool-refactor で template src/ は read-only でしか使われず、`.keep` の機能的意義 = 0)、cargo workflow 操作性 / IDE 対応で 3 件 minor regression |
| C: `fs::copy("Cargo.lock", ...)` 削除 + per-runner regenerate | Gitignored (e2e のみ) | (no change) | 各 per-runner で独立 dep resolution → version drift の risk、pool init に `cargo generate-lockfile` 数秒追加 |
| D: pool init 前に template で `cargo generate-lockfile` | (no change) | (no change) | network 必須、Cargo.lock 内容が run 毎に変動、差分管理不能 |

**Approach A の根拠**:
1. **Reproducibility**: 両 subproject の dep version を tracked Cargo.lock で固定 → CI 実行間 / contributor 間の dep drift を防止 (`tests/compile-check/Cargo.toml` 内 "Keep dependencies in sync" NOTE と整合)
2. **Pool init code 統一**: e2e/rust-runner pool init が `Cargo.toml` / `Cargo.lock` / `src/main.rs` 3 file を **全て `copy_runner_template_file` 統一形** で扱える (現状 Cargo files = copy / src/main.rs = stub-write の不統一を解消)
3. **Template が valid Cargo project**: e2e/rust-runner template directory で `cargo generate-lockfile` / `cargo metadata` / IDE rust-analyzer が **temp stub 作成なしで** 動作。Cargo.toml dep 更新時の workflow が直感的
4. **`.keep` の vestigial 性解消**: e2e/rust-runner `.keep` は template `src/` への write が無いため機能的意義が皆無 (pool refactor 以降)。tracked main.rs が dir を保持するため `.keep` 不要
5. **compile-check は asymmetric な ideal を honest に反映**: template-buffer pattern は generation buffer として template src/ を使うため `*.rs` ignore は **現在も valid**。pool refactor が compile-check には未適用であり、本 PRD で pattern 変更しない (compile-check pool 化は別 PRD 候補、test 数 3 件で ROI 低い)

#### 本 PRD の最終変更内容

`compile-check` (template-buffer pattern):
1. `.gitignore` から `tests/compile-check/Cargo.lock` 削除
2. `tests/compile-check/Cargo.lock` を `cargo generate-lockfile` で再生成し tracked 化
3. `tests/compile-check/src/.keep` 新規 (空 file、`fs::write` の parent dir 保証)
4. `.gitignore` で `tests/compile-check/src/lib.rs` → `tests/compile-check/src/*.rs` 拡張 ignore に変更
5. `tests/compile_test.rs:14-21` doc comment を `.keep` + `*.rs` ignore + `fs::write` parent dir 非作成を明記する形に更新

`e2e/rust-runner` (pool pattern):
6. `.gitignore` から `tests/e2e/rust-runner/Cargo.lock` 削除
7. `.gitignore` から `tests/e2e/rust-runner/src/main.rs` (and 派生 `src/*.rs` ignore) 削除
8. `tests/e2e/rust-runner/Cargo.lock` を `cargo generate-lockfile` で再生成し tracked 化
9. `tests/e2e/rust-runner/src/main.rs` を **tracked stub** (`fn main() {}` + 役割を説明する doc comment) として新規 commit
10. `tests/e2e/rust-runner/src/.keep` は **配置しない** (cell #10 の `.keep` 案を Approach B として retire)
11. `tests/e2e_test.rs:215-217` pool init を **`copy_runner_template_file("src/main.rs", ...)` 統一形** に維持 (Cargo.toml / Cargo.lock / main.rs を 3 file 統一 fs::copy)
12. `tests/e2e_test.rs:593` doc comment "writes them to `tests/e2e/rust-runner/src/`" → "writes them to the per-runner temp dir's src/" に修正

`.gitignore` 全体:
13. comment を asymmetric handling の根拠 (template-buffer vs pool) を明記する形に書き直す

### Design Integrity Review

`.claude/rules/design-integrity.md` checklist:

- **Higher-level consistency**: ✓ pool refactor 後の access pattern (template = read-only skeleton) を honest に反映。pool init code が 3 file 統一 fs::copy になり pool init 自体の cohesion 向上。
- **DRY / Orthogonality**: ✓ DRY 観点で「symmetric に統一」は誤適用 — 各 subproject の access pattern が **本質的に異なる** ため asymmetric が正しい (false symmetry を避ける)。pool init 内 3 file の copy は完全に DRY 化。compile-check (template-buffer) と e2e/rust-runner (pool) は orthogonal な test infrastructure として並立。
- **Coupling**: ✓ Cargo.lock track により subproject Cargo.toml と Cargo.lock の couple が明示的 (Cargo 意図の coupling)。pool init が template の存在に dependency するが、これは pool pattern の constituent property。
- **Broken windows**:
  - 発見 (in scope fix 済): `.gitignore` 旧 I-161 comment "write_with_advancing_mtime" stale → 本 PRD で book 全体書き換え
  - 発見 (in scope fix 済): `tests/e2e_test.rs:593` doc が pool refactor 前の記述のまま → 本 PRD で fix
  - 発見 (out of scope): `copy_runner_template_file` 関数名 — Approach A で 3 file 統一 copy となるため、関数名が「全 template file copy」を正しく表現する状態に回復 (rename 不要)
  - 発見 (out of scope): compile-check pool 化候補 — test 数 3 件で `Mutex` serialize ROI 低、別 PRD 候補

判定: **問題なし** (broken window 全て本 PRD で解消 or 別 PRD 候補として記録)。

### Impact Area

- `.gitignore` (Cargo.lock x2 削除、e2e src/main.rs ignore 削除、comment 全体書き直し)
- `tests/compile-check/Cargo.lock` (新規 git track)
- `tests/compile-check/src/.keep` (新規)
- `tests/compile_test.rs` (doc comment 更新)
- `tests/e2e/rust-runner/Cargo.lock` (新規 git track)
- `tests/e2e/rust-runner/src/main.rs` (新規 git track、tracked stub)
- `tests/e2e_test.rs` (pool init を統一 copy 形に維持、line 593 doc 修正)
- `plan.md` (完了 row 追加)
- `backlog/I-184-ci-fresh-clone-template-file-fix.md` (本 PRD doc 新規)

### Semantic Safety Analysis

**Not applicable** — 型 fallback / approximation / type resolution 変更を伴わない CI infra fix。

### 3a. Production Code Quality Review

| Issue | Location | Category | Severity | Action |
|-------|----------|----------|----------|--------|
| P1 | `tests/e2e_test.rs:593` doc | Stale doc (pool refactor 前の記述) | Low | 本 PRD で fix |
| P2 | `.gitignore` 旧 I-161 comment "write_with_advancing_mtime" | Stale comment (関数自体撤廃) | Low | 本 PRD で書き直し |
| P3 | `tests/e2e_test.rs:71` `copy_runner_template_file` 関数名 vs 現状 | Approach A 採用で 3 file 統一 copy になり、関数名が機能を正しく表現する状態に回復 | (resolved) | (rename 不要に) |

### 3b. Test Coverage Review

| Gap | Pattern | Technique | Severity |
|-----|---------|-----------|----------|
| G1 | "Fresh-clone state で全 test pass" の CI level regression test | E2E-level | Medium |

G1 対応: 専用 unit/integration test 追加は invasive (subproject template state を mock するため)、CI 自体が `actions/checkout@v6` で fresh-clone state を実行 → 既存 CI workflow が de-facto regression test として機能。本 PRD 完了後、CI run 1 回 pass で G1 検証完了とする。

## Task List

### T1: `compile-check` を template-buffer pattern として ideal 化

- **Work**:
  1. `.gitignore` から `tests/compile-check/Cargo.lock` 削除
  2. `tests/compile-check/Cargo.lock` を `cargo generate-lockfile` で再生成し tracked 化
  3. `tests/compile-check/src/.keep` 新規 (空 file、`fs::write` parent dir 保証)
  4. `.gitignore` で `tests/compile-check/src/lib.rs` → `tests/compile-check/src/*.rs` (拡張 ignore)
  5. `tests/compile_test.rs:14-21` doc comment 更新 (`.keep` で dir tracked + `*.rs` ignore + `fs::write` parent dir 非作成 を明記)
- **Completion criteria**:
  - `git ls-files` に `tests/compile-check/Cargo.lock`、`tests/compile-check/src/.keep` 両方が tracked
  - `git check-ignore -v tests/compile-check/src/lib.rs` で `*.rs` pattern による ignore を確認
  - `cargo build --tests` 成功
- **Depends on**: なし

### T2: `e2e/rust-runner` を pool pattern として ideal 化

- **Work**:
  1. `.gitignore` から `tests/e2e/rust-runner/Cargo.lock` および `tests/e2e/rust-runner/src/main.rs` (派生 `src/*.rs`) entry を削除
  2. `tests/e2e/rust-runner/Cargo.lock` を `cargo generate-lockfile` で再生成し tracked 化
  3. `tests/e2e/rust-runner/src/main.rs` を `fn main() {}\n` + 役割を説明する doc comment で **tracked stub** として新規 commit
  4. `tests/e2e/rust-runner/src/.keep` は **配置しない** (Approach B retire)
  5. `tests/e2e_test.rs:215-217` pool init を **`copy_runner_template_file("src/main.rs", ...)` 統一形** に維持 (Cargo.toml / Cargo.lock / main.rs 3 file 統一)
  6. `tests/e2e_test.rs:593` doc comment "writes them to `tests/e2e/rust-runner/src/`" → "per-runner temp dir's src/" に修正
- **Completion criteria**:
  - `git ls-files` に `tests/e2e/rust-runner/Cargo.lock`、`tests/e2e/rust-runner/src/main.rs` 両方が tracked
  - `tests/e2e/rust-runner/src/.keep` が **存在しない**
  - `git check-ignore -v tests/e2e/rust-runner/src/main.rs` で **ignore されない** (exit 1 / no output)
  - `cargo build --tests` 成功
  - `tests/e2e/rust-runner/` directory 内で `cargo metadata > /dev/null` および `cargo generate-lockfile` が **temp stub 作成なしで** 動作 (template が valid Cargo project) — Approach A の design 主張の empirical 検証
- **Depends on**: なし (T1 と並列可)

### T3: `.gitignore` comment を asymmetric handling 根拠の説明に書き直す

- **Work**: `.gitignore` の I-145 / I-161 comment を、各 subproject の access pattern (template-buffer vs pool) と asymmetric handling の根拠を future maintainer のために記録する形に書き直す。
- **Completion criteria**:
  - comment に「compile-check = template-buffer pattern (毎 test fs::write 上書き → `*.rs` ignore + `.keep`)」「e2e/rust-runner = pool pattern (template = read-only skeleton → tracked stub)」の対比が明記
  - `write_with_advancing_mtime` への stale 言及が削除済
- **Depends on**: T1, T2

### T4: True fresh-clone state での verification

- **Work**:
  1. `rm tests/compile-check/Cargo.lock tests/e2e/rust-runner/Cargo.lock tests/compile-check/src/lib.rs 2>/dev/null || true` (Cargo.lock x2 + compile-check 生成 .rs を削除して fresh-clone 相当を再現。e2e/rust-runner src/main.rs は **tracked なので削除しない**)
  2. `cargo test --test compile_test --test e2e_test -- --test-threads=2` 実行
  3. `git status` で意図しない変更が無いことを確認 (再生成された Cargo.lock x2 と compile-check src/lib.rs は working tree 上に出るが、それぞれ tracked diff / `*.rs` ignored で問題なし)
  4. `cd tests/e2e/rust-runner && cargo generate-lockfile` を **temp stub 作成なしで** 実行可能か確認 (Approach A design 主張の empirical 検証)
  5. `cargo clippy --all-targets --all-features -- -D warnings` 実行
  6. `cargo fmt --all --check` 実行
- **Completion criteria**:
  - compile_test: 3 passed; 0 failed; 0 ignored
  - e2e_test: 157 passed; 0 failed; 29 ignored (既存)
  - cargo generate-lockfile in e2e template: 成功 (temp stub 不要)
  - clippy 0 warning, fmt 差分なし
- **Depends on**: T1, T2, T3

### T5: plan.md 更新 + commit message 提案

- **Work**:
  - `plan.md` 直近の完了作業 table に I-184 row 追加
  - commit message を提案 (実行は user)
- **Completion criteria**:
  - commit message に backlog/I-184 reference + Approach A 根拠 (asymmetric handling) + 歴史的経緯 (pool refactor で I-161 前提が消滅) を含む
  - changes: `.gitignore` / `tests/compile-check/Cargo.lock` (new tracked) / `tests/compile-check/src/.keep` (new) / `tests/compile_test.rs` / `tests/e2e/rust-runner/Cargo.lock` (new tracked) / `tests/e2e/rust-runner/src/main.rs` (new tracked stub) / `tests/e2e_test.rs` / `plan.md` / `backlog/I-184-ci-fresh-clone-template-file-fix.md` (new)
- **Depends on**: T4 完了 + user 承認

## Test Plan

### 既存 test の継続 pass

- `cargo test --test compile_test`: 3 passed (既存)
- `cargo test --test e2e_test`: 157 passed; 29 ignored (既存)
- `cargo test`: 全 binary pass

### Fresh-clone state regression verification (T3)

CI 自体が fresh-clone を毎回実行するため、本 PRD 完了後の CI run pass を最終 regression test として位置付け。ローカル verification protocol は T3 に明記。

### 新規 test 追加なし

理由: 「fresh-clone state で test fail しないこと」を unit test 化するには subproject template state を mock する必要があり invasive。CI は de-facto 毎 run fresh-clone simulation。費用対効果不釣合いのため新規 test 追加せず CI を regression source とする。

## Completion Criteria

**Matrix completeness requirement**: Problem Space matrix の全 ✗ cell (#3, #4, #7, #8) が ✓ に遷移、その他 cell は変化なし or 既 ✓。

具体条件:

1. ✅ `.gitignore` に `Cargo.lock` x2 entry が存在しない
2. ✅ `.gitignore` に `tests/e2e/rust-runner/src/main.rs` または派生 `src/*.rs` entry が存在しない
3. ✅ `.gitignore` に `tests/compile-check/src/*.rs` entry のみが残る (compile-check template-buffer pattern 用)
4. ✅ `git ls-files` で `tests/compile-check/Cargo.lock`、`tests/compile-check/src/.keep`、`tests/e2e/rust-runner/Cargo.lock`、`tests/e2e/rust-runner/src/main.rs` (stub) が tracked
5. ✅ `tests/e2e/rust-runner/src/.keep` が **存在しない** (Approach B retire の verify)
6. ✅ `git check-ignore -v tests/compile-check/src/lib.rs` で `*.rs` pattern による ignore (compile-check template-buffer)
7. ✅ `git check-ignore -v tests/e2e/rust-runner/src/main.rs` で **ignore されない** (e2e template-stub tracked) — exit 1 / no output
8. ✅ True fresh-clone state (`Cargo.lock` x2 + `tests/compile-check/src/lib.rs` 削除、e2e tracked stub は保持) で `cargo test --test compile_test --test e2e_test` 全 pass (compile_test 3 / e2e_test 157)
9. ✅ `tests/e2e/rust-runner/` directory 内で `cargo metadata > /dev/null` および `cargo generate-lockfile` が temp stub 作成なしで成功 (template が valid Cargo project)
10. ✅ `cargo clippy --all-targets --all-features -- -D warnings`: 0 warning
11. ✅ `cargo fmt --all --check`: 差分なし
12. ✅ `tests/e2e_test.rs:593` doc comment が pool refactor 後の動作に整合
13. ⏸️ 本 PRD 完了後の最初の CI run で `cargo llvm-cov` step が pass (post-commit verify)

**Impact estimate verification**: Affected ✗ cell 4 件を実 trace で confirm:
- cell #3 (compile-check src/lib.rs): CI log の `failed to write tests/compile-check/src/lib.rs: No such file or directory` を直接 reproduce
- cell #4 (compile-check src/ dir): cell #3 の root cause として trace 済
- cell #7 (e2e Cargo.lock): ローカル fresh-state verify で `failed to copy runner Cargo.lock: No such file or directory (os error 2)` を直接 reproduce
- cell #8 (e2e src/main.rs): pool refactor 経緯 + `copy_runner_template_file("src/main.rs", ...)` 復活で template stub 必要を trace 済

## Lesson

1. **Stale gitignore は latent CI defect の温床**: `write_with_advancing_mtime` 撤廃 + pool refactor で **「write-only artifact」前提が消滅** したのに gitignore entry のみ残された結果、e2e/rust-runner template `src/main.rs` および `Cargo.lock` は実体としては「pool init `fs::copy` の read source」であるにも関わらず gitignored の状態で stale 化していた。Refactor 時に **「access pattern が変わった file の git tracking state」を整合させる checklist** が test infrastructure refactor の標準手順として必要。

2. **False symmetry を避ける**: 表層的に類似する 2 subproject (compile-check / e2e/rust-runner) を `.keep` + `*.rs` ignore で symmetric に統一すると、access pattern の本質差 (template-buffer vs pool) を覆い隠して vestigial defensive coding を生む。symmetric な統一は **DRY 原則の表層的適用**であり、orthogonal な test infrastructure を強引に同型化する **false symmetry**。design integrity review で「2 つは本当に同じ問題か?」を verify する step が必要。

3. **PRD の `Background` セクションには歴史的経緯を記録する**: I-145 / I-161 当時の design 根拠 (write-only artifact) と、後続 commit `24a14b7` (pool refactor) で前提が消滅した経緯を本 PRD に記録することで、将来の maintainer が「なぜこの asymmetric handling か?」を再構築可能にする。

## Related Issues

- **I-145** (Pre-I-144 cleanup, 4a52fd4, 2026-04-19): compile-check `lib.rs` を gitignored 化した PRD。本 PRD は I-145 design (template-buffer pattern + gitignored write-only artifact) を **維持**、追加で `Cargo.lock` tracked + `.keep` で `src/` dir 保持。
- **I-161** (Pre-I-144 cleanup, 同 commit): e2e/rust-runner `main.rs` を gitignored 化した PRD。本 PRD は **後続 pool refactor (24a14b7) で I-161 の「write-only artifact」前提が消滅した** 事実を初めて文書化し、tracked stub design に correction。
- **24a14b7 (テスト基盤強化)**: e2e/rust-runner pool refactor commit。本 PRD で I-161 想定との整合 gap が検出される起源。
