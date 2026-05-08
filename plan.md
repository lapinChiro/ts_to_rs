# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-05-08)

**進行中**: PRD α-1 (I-224 = B2 fn main mechanism) Implementation Stage、T6a 完了 (commit 80d9df1)、T7 prerequisite として **I-399 (e2e test isolation defect) を先行 PRD として起票** (案 β Phase 1-A 順序 update、user 承認 2026-05-08 Option A)。

**次着手**: I-399 Spec stage TS-0〜TS-4 完了 → Implementation T1-T4 → /check_job 4-layer review → I-399 close → I-224 T7 chain 再開。I-399 PRD doc = `backlog/I-399-e2e-test-isolation-defect.md` (Spec stage iteration v1 draft、TS tasks 4 件未済 = High findings)。

### Quality Gate (post T6a = I-154 doc 3-category 拡張 + scripts/audit-no-pub-fn-init.sh CI integration + 4-layer review = pre-existing line-ref drift 2 件 broken-window fix)

| 指標 | 値 |
|------|-----|
| cargo test (24 binaries) | lib **3546** (T5-2 baseline 完全一致、T6a は src/ 無変更) / integration 122 / compile 3 / i224_invariants 7 / i224_helper 5 / i205_invariants 2 + 5 ignored / i205_helper 4 / 全 green。e2e は test infra non-determinism (= 新 I-399 起票、parallel 9 fail / serial 8 fail (異なる cell set)、T6a 因果無関係 = code 0 行変更で baseline と一致、I-180 entry が誤って参照していた未起票 I-173 を I-399 として正式起票完了) のため本 commit では別 PRD scope と認識 |
| cargo clippy / fmt / file-line | 0 warnings / 0 diffs / 全 .rs file < 1000 行 (transformer/mod.rs 931 / main_synthesis/mod.rs 974 / expressions/mod.rs 935 / user_main.rs 389) |
| audit-prd-rule10-compliance.py / audit-no-pub-fn-init.sh (CI-integrated post T6a) / audit-no-init-call-site.sh | PASS / exit=0 (INV-4 + INV-7 + CI merge gate lock-in) |
| Hono bench | clean **107** / errors **72** at SHA-pinned 027e3df (= T6a doc+CI 変更で code 不変、Preservation classification 維持) |

**bench 非決定性**: ±1 clean / ±2 errors の noise variance を [I-172] として記録 (test/bench infra defect、別 PRD)。

### Implementation Stage 進捗 (T1〜T9 sub-commits 一覧)

| Phase | Status | 完了日 |
|-------|--------|--------|
| T1: `__ts_` namespace + collision detection (INV-5 fill-in) | ✓ 完了 | 2026-05-07 |
| T2: IR enums + helper (INV-3 partial / INV-6 fill-in) | ✓ 完了 | 2026-05-07 |
| T3: fn main synthesis + rename + substitute + Axis B/E probes | ✓ 完了 | 2026-05-07 |
| T4: transform_module refactor + `pub fn init` 廃止 (INV-4 fill-in) | ✓ 完了 | 2026-05-07 |
| **T5-1**: Existing C0 cells e2e green-ify + I-205 cell-09 unblock + INV-1 fill-in | ✓ 完了 | 2026-05-08 |
| **T5-2**: NEW C0 fixtures e2e green (cell-77 GREEN + cell-41 / cell-79 Tier 2 lock-in 確認) + B2 .await wrap fix (cells 11/23/75 unblock = Iteration v11 Spec への逆戻り) + INV-7 fill-in + 4-layer review | ✓ 完了 | 2026-05-08 |
| **T6a**: I-154 namespace doc 3-category 拡張 (labels + value bindings + fn rename target) + `scripts/audit-no-pub-fn-init.sh` CI integration (PR merge gate) + 4-layer review (Layer 1 で pre-existing line-ref drift 2 件 broken-window fix) | ✓ 完了 | 2026-05-08 |
| T7: Test harness ESM upgrade permanent integration | **次着手** | — |
| T8: Top-level await synthesis logic | 未着手 | — |
| T9: Axis C1 cells e2e green + Hono bench verify + `[CLOSE]` PRD 完了 | 未着手 | — |

各 T の詳細 task spec + completion criteria は [`backlog/I-224-top-level-fn-main-mechanism.md`](backlog/I-224-top-level-fn-main-mechanism.md) Sub-commits 一覧 table 参照。

### Spec への逆戻り Iteration log (PRD doc Iteration v1〜v10 = audit trail)

| Iteration | 日付 | 概要 |
|-----------|------|------|
| v1〜v7 | 2026-05-01 | Spec stage 5 rounds adversarial review + convention compliance、52 件 actions 全 resolve、Spec stage true closure 達成 |
| v8 | 2026-05-07 | T2 完了時、I-228 sub-entries 4 件 (recursive Await detection 等) Spec への逆戻り |
| v9 | 2026-05-08 | T5-1 着手中、cell-12/24 silent-drop Tier 1 fix 用 `InitKind` 4→5 variant split (NonTriggerDef + NonTriggerData) |
| v10 | 2026-05-08 | T5-1 完了後 `/check_job` 3 iteration + `/check_problem` 2 round の累積 structural fix 5 件 record |
| **v11** | 2026-05-08 | T5-2 着手時、B2 + executable-mode `__ts_main()` substitute call **Tier 1 silent semantic loss** 発見 (cells 11/23/75) → `UserMainSubstitution` enum + `from_dispatch` constructor DRY 解消 + `UserMainKind` / `UserMainSubstitution` / `detect_user_main` の `user_main.rs` 同居 cohesion 向上 + `.claude/rules/file-size-resolution.md` 新設 (= 機械的末尾切り出し禁止) |

詳細 audit trail 全文 = [`backlog/I-224-top-level-fn-main-mechanism.md`](backlog/I-224-top-level-fn-main-mechanism.md) `## Spec Review Iteration Log`。

---

## /start 再開時の手順

### Step 1: 状態確認
1. **本 plan.md** を読む = 現在の状態 + 次着手 = **I-399 Spec stage** (= I-224 T7 prerequisite block per 案 β Phase 1-A 順序 update)
2. **I-399 PRD doc を読む** = [`backlog/I-399-e2e-test-isolation-defect.md`](backlog/I-399-e2e-test-isolation-defect.md) (= 進行中 PRD、Spec stage iteration v1 draft 完了、TS-0〜TS-4 未済)
3. **I-224 PRD doc を参照** = [`backlog/I-224-top-level-fn-main-mechanism.md`](backlog/I-224-top-level-fn-main-mechanism.md) (= I-399 完了後再開 PRD、T7-T9 待機)
4. **TODO 確認** = [`TODO`](TODO) (関連 entries: I-172 / I-180 / I-395-398 / **I-399 (進行中)**)

### Step 2: I-399 Spec stage TS-0〜TS-4 完了

**Work**:
1. **TS-0**: Cartesian product matrix completeness (= 5 axes × 全 reachable cells、現 10 cells representative subset を全 enumerate に拡張)
2. **TS-1**: Deep investigation root cause 確定 (= 3 probes: cargo build verbose / instrumented runner / panic-recovery race) → `report/I-399-root-cause-investigation.md` 作成
3. **TS-2**: Structural fix design empirical verify (= prototype `/tmp/i399-prototype/` で 100 round determinism + performance ±10% 検証)
4. **TS-3**: Integration test 起票 (= `tests/i399_isolation_test.rs` 新規)
5. **TS-4**: Audit findings record (= `## Impact Area Audit Findings` section embed 済、cross-check のみ)

**Completion criteria**:
- 13-rule self-applied verify で Critical=0 / High=0 (Iteration v2 Spec stage approved)
- `audit-prd-rule10-compliance.py` PASS

### Step 3: I-399 Implementation stage T1-T4

**Work**:
1. **T1**: `E2eRunnerInstance::run_with_source(rs_source) → RunnerOutput` 新設、SHA-256 hash + 動的 [[bin]] append + `cargo run --bin <hash>`
2. **T2**: `tests/e2e/rust-runner/Cargo.toml` から default [[bin]] 削除、initial state は [package] + [dependencies] のみ
3. **T3**: Backward compatibility verify (= 4 mode × 5 round で deterministic、INV-T1/T2/T3 lock-in)
4. **T4**: `/check_job` 4-layer review pass + plan.md/TODO update + I-399 PRD close

**Commit message**: `[WIP] I-399 完了: E2E test isolation defect structural fix (per-test content-hash-derived bin、INV-T1/T2/T3 lock-in、277 e2e tests deterministic)`

### Step 4: I-399 完了後の chain (= I-224 T7-T9 再開)
I-224 T7 (= ESM upgrade、tokio dependency 追加 + observe-tsc.sh --esm CI flow) → T8 (top-await synthesis logic) → T9 (Axis C1 cells e2e green-ify + Hono bench Tier-transition + `[CLOSE]` I-224 PRD 完了)。

---

## 実行順序 (prerequisite chain、案 β = Universal infra leverage first + L1 mid-priority)

**案 β 採用根拠** (2026-05-01 user 承認、星取表 20/24 で 4 案中最良判定): Leverage 最大化 (B2/B3 を最先で fix → 全 future PRD の e2e verification + class field 変換が構造的に正しくなる)、Methodology infra 早期 codify (I-D を I-205 close 直後に整備 → 後続 PRD spec stage が第 1 反復で完成可能)、L1 Tier 0 (I-177) 中盤投入 (I-D 完了直後 = framework v2.x 安定後で I-177 spec stage の re-iteration risk 圧縮)。

```
[完了] PRD 1〜2.7 (I-177-D / I-177-E / I-177-B / I-177-F / I-198+199+200 batch) — 2026-04-26〜27
   ↓
[完了] PRD 2.75 = I-205 (Class member access dispatch with getter/setter framework、T1-T13 完了) — 2026-05-01
   T14-T16 は案 β Phase 1-A 完了後に再開 (= I-224 / I-225 / I-162 universal infra prerequisite block)
   ↓
═════ 案 β Phase 1-A: I-205 T14 prerequisite (3 PRD 逐次起票 + I-399 prerequisite block) ═════
   ↓
[進行中] PRD α-0-prerequisite = **I-399 (E2E test isolation defect)** — Spec stage v1 draft 2026-05-08、TS-0〜TS-4 未済 (= I-224 T9 並列 e2e green-ify の universal infra prerequisite、user 承認 2026-05-08 Option A)
   ↓
[待機中] PRD α-1 = I-224 (B2 fn main mechanism) — T6a 完了 2026-05-08 (commit 80d9df1)、I-399 完了後 T7 chain 再開
   ↓
[次] PRD α-2 = I-225 (B3 class field literal-only initializer type inference)
   ↓
[次] PRD α-3 = I-162 (constructor synthesis `Self::new()` for no-explicit-constructor classes)
   ↓
═════ 案 β Phase 1-B: I-205 close (T14 → T15 → T16) ═════
   ↓
[次] I-205 T14: E2E fixtures green-ify (B2/B3/I-162 verified end-to-end、34 cells)
   ↓
[次] I-205 T15: /check_job 4-layer review + 13-rule self-applied verify
   ↓
[次] I-205 T16: Task-ID-based naming → semantic naming refactor + I-205 範囲内 unwrap() cleanup
   ↓
═════ 案 β Phase 1-C: Methodology infra codify ═════
   ↓
[次] 新 PRD I-D: Framework rule integration cohesive batch (D-1〜D-4 + B2/B3/I-162 lessons + I-224 v3〜v11 framework gap candidates + I-399 Spec stage 2nd/3rd-round candidates、計 21 件 = R-1 + R-5 + 改善 v2-1 + v3-4/5/6 + v4-1/2/3 + v5-1/2 + v6-1/2 + v11-1/3/4/5/6/7/8/9、9 rounds adversarial review 累積)
   ↓
═════ 案 β Phase 2: L1 Tier 0 priority ═════
   ↓
[次] PRD 3 = I-177 mutation propagation 本体 (Tier 0 silent semantic change、L1)
   ↓
═════ 案 β Phase 3: Class dispatch group → L1 silent decorator ═════
   ↓
[次] PRD 2.76 = I-A (Method static-ness IR field propagation、元 I-205 T11-b)
   ↓
[次] PRD 2.77 = I-B (Class TypeName context detection unification + Static field associated const emission、元 I-205 T11-d/f + I-214 統合)
   ↓
[次] PRD 2.8 = I-201-A (AutoAccessor 単体 Tier 1 化、decorator なし subset)
   ↓
[次] PRD 2.9 = I-202 (Object literal Prop::Method/Getter/Setter Tier 1 化)
   ↓
[次] PRD 7 = I-201-B (Decorator framework 完全変換、TC39 Stage 3、L1 silent semantic change)
   ↓
═════ 案 β Phase 4: Narrow refinements (post-L1 cleanup) ═════
   ↓
[次] PRD 4 = I-177-A (else_block_pattern Let-wrap + I-194 typeof if-block elision 拡張可)
   ↓
[次] PRD 5 = I-177-C (symmetric XOR early-return detection)
   ↓
[次] PRD 6 = I-048 (closure ownership 推論、T7-3 完全 GREEN-ify)
   ↓
Phase A Step 5 → I-015 → I-158+I-159 → Phase A Step 6 → ...
```

**PRD 凝集度原則**: 凝集度高 + 適切な粒度。1 PRD = 1 architectural concern。各 PRD の architectural concern + 着手順 rationale は下記「次の作業 table」+ TODO 参照。

---

## 次の作業 table (priority order)

| 優先度 | レベル | PRD | architectural concern (= 1 PRD = 1 concern) |
|--------|-------|-----|---------------------------------------------|
| **進行中** | L3 (universal infra prerequisite に promote) | **I-399 (E2E test isolation defect)** Spec → Implementation | `cargo test --test e2e_test` の test 実行順序 / concurrency mode によらない deterministic test result 保証 (= per-test content-hash-derived bin による cargo cache collision 構造的排除) |
| 次優先 1 (I-399 完了後) | L3 | **I-224 (B2 fn main mechanism)** T7〜T9 | top-level executable script の Rust emission で `fn main()` 自動生成 + Option β (top-await Tier 1 + ESM harness) cohesive batch |
| 次優先 2 | L3 | **I-225 (B3)** | Class field の literal-only initializer (annotation 無) で type inference 完成 |
| 次優先 2 | L3 | **I-162** | Constructor synthesis `Self::new()` for no-explicit-constructor classes |
| 次優先 3 | L2 | **I-205 T14〜T16** | Class member access dispatch with getter/setter framework 完了 (e2e green-ify + naming refactor) |
| 次優先 4 (post-I-205 close) | L4 | **I-D Framework rule integration cohesive batch** | task-ID 命名禁止 + Iteration v18 改善 4 件 + T7/T8 framework gap + T5 lessons + Iteration v9/v10/v11 lessons + B2/B3/I-162 lessons + T6a 2nd-round adversarial lessons (3 candidates v11-5/6/7 = handoff doc cross-ref drift detection + double-source consistency axis + Layer 1 factual accuracy semantic check) + I-399 Spec stage 2nd-round candidate (v11-8 = Rule 13 pending verdict severity default = Critical) (cohesive batch、計 21 candidates) |
| L1 Tier 0 | L1 | **PRD 3 (I-177 mutation propagation)** | F1/F3 narrow body 内 mutation の outer Option<T> propagation (silent semantic change 解消) |
| Class group | L3 | **PRD 2.76 (I-A) + 2.77 (I-B) + 2.8 (I-201-A) + 2.9 (I-202)** | Method static-ness IR field / Class TypeName context detection / AutoAccessor / Object literal getter/setter |
| L1 silent | L1 | **PRD 7 (I-201-B)** | Decorator framework 完全変換 (TC39 Stage 3) |
| Narrow refinements | L3 | **PRD 4-6 (I-177-A / I-177-C / I-048)** | typeof Let-wrap / symmetric XOR / closure ownership 推論 |
| Phase A continuations | L3 | **I-162 → Step 5 → I-015 → I-158+I-159 → Step 6 → I-143 / Step 7 / Phase B** | compile_test skip 解消 chain |

詳細 architectural concern + 着手順 rationale + completion criteria は各 PRD の TODO entry / backlog/ doc 参照。

### 次点 / L4 deferred (上記 table 外)
- I-013 + I-014 batch (RC-5 abstract class 変換パス)、I-140 (TypeDef::Alias)、I-050 umbrella (Any coercion)、I-146 (`return undefined` on void fn)、I-074 / I-160 / I-165〜I-170 / I-168 / I-172 / I-177-G (= 各 L4 latent items、TODO 参照)
- **I-395** = Class expression conversion (anonymous class lifting、I-201-A / I-201-B 系 cohesive batch 候補)
- **I-396** = Module-level destructuring pattern proper conversion (I-016 silent drop family、5-axis matrix-driven PRD 候補)
- **I-397** = e2e harness `should_auto_append_main_call` detection edge cases (low priority infra)

---

## 直近の完了作業 (audit trail summary)

実装詳細は git log、設計判断 archive は [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD / Phase | 日付 | 後続への影響 |
|-------------|------|-------------|
| **I-224 T6a 完了 + adversarial 2nd-round /check_job 反映** (I-154 namespace handoff doc 3-category 拡張 = 1-a labels (4 entries) + 1-b value bindings (`__ts_old`/`__ts_new`/`__ts_recv`、I-205 setter dispatch desugar) + 1-c function rename target (`__ts_main`、I-224 T1、INV-5) + canonical 単一 source of truth pointer (`src/transformer/statements/mod.rs:27-47`) で固定 list duplication 回避 + 1-d lint enforcement 2 axis (label-side 4 sites + module-level identifier-side `scan_for_ts_namespace_collisions`) + 1-e reservation rationale + 1-f CI invariant lock-in section / `scripts/audit-no-pub-fn-init.sh` を `.github/workflows/ci.yml` に CI step "Audit no `pub fn init` (I-224 INV-4 lock-in)" として integrate (PR merge gate、enforced paths = `src/` + `tools/` + `tests/e2e/rust-runner/`、advisory paths = `tests/e2e/scripts/**/*.rs` gitignored working-tree-only artefacts) / `/check_job` 4-layer review **2 round** pass: 1st round で line-ref drift 2 件 (`__ts_main` :130→:133、`__ts_do_while_loop` :346→:356) + `__ts_switch` row line ref 追加 / 2nd round adversarial で 4 件追加 fix = Iteration v11 ambiguity 排除 (1-b header から I-224 v11 と I-205 task ID conflate を解消) + "T8 INV-3" → "I-205 INV-3 source-side single-evaluation contract" 不曖昧化 + 1-f false claim 訂正 (= advisory paths は gitignored working-tree-only、I-224 T5 incomplete latent state ではない) + audit script advisory comment 整合 update (handoff doc と double-source) / Layer 2 empirical (lib 3546 PASS = T5-2 完全一致 / i224_invariants 7 PASS = INV-1〜INV-7 lock-in 維持 / audit-no-pub-fn-init exit=0 / CI yml 15 steps well-formed) / Layer 3 cross-axis (3 reservation categories × 2 lint axes × CI gate axis 全 enumerated、Spec gap 0、emission sites 8 sites all covered) / Layer 4 trade-off (pure addition + broken-window discipline、regression cell 0、structural fix not patch、interim 条件不問) / Defect classification = Implementation gap 6 件 (2 1st-round line-ref + 4 2nd-round factual accuracy) all fixed-in-commit + Review insight 2 件 = (a) **e2e test isolation defect** (= cargo test --test e2e_test で parallel 9 fail / serial 8 fail (異なる cell set)、stale binary leak 由来の test order 依存 = I-180 entry が "I-173 (E2E parallel flakiness)" として 関連 PRD 言及するが I-173 自体未起票、新 TODO I-399 候補、T6a 因果無関係 = code 0 行変更で baseline と一致 = I-172 (bench non-determinism、別 axis) とは別 defect) + (b) doc line-ref drift detection automation (= handoff doc grep `<file>:<line>` を verify する CI step 候補、I-D framework batch 候補) | 2026-05-08 | 全 future PRD で `__ts_*` namespace 拡張時の structural enforcement (canonical doc-first dependency)、INV-4 invariant CI merge gate lock-in (post-T4 0 hits、再混入を merge 段階で block)、advisory paths の gitignored 性質を構造的に文書化 (CI fresh clone = 0 advisory)、handoff doc cohesion 向上 (3 category symmetric coverage、token-level accuracy)、broken-window discipline (line-ref drift fix + factual accuracy fix)、e2e test isolation defect finding を I-399 候補として triage、doc-cross-ref drift detection を I-D framework batch 候補化。詳細 = git log + design-decisions.md `## Switch emission と label hygiene > 1` |
| **I-224 T5-2 + Iteration v11 (initial fix + deep review structural fix + 2nd review unit tests + /check_problem TODO 起票)** (`UserMainSubstitution` enum + `from_dispatch` constructor DRY 解消 / `UserMainKind` + `UserMainSubstitution` + `detect_user_main` の `user_main.rs` 同居 cohesion 向上 / cells 11/23/75 e2e green-ify (Tier 1 silent-loss fix) / **deep review structural fix** = double-await bug (cells 16/30/36 + nested `await main()` 用 `Transformer::suppress_main_await_wrap` flag + `convert_expr_in_await_context` helper による context-aware suppression、3 entry sites symmetric 適用) / INV-2 拡張 (B1 sync + B2 async + `await main();` patterns 4 sub-cases lock-in) / **2nd review unit tests** (`UserMainSubstitution::from_dispatch` 10-cell decision table + `is_active()` / `is_async()` predicates + cross-predicate invariant `is_async ⇒ is_active` の 4 direct unit tests) / INV-7 invariants test fill-in (subprocess audit + independent grep verifier) / `scripts/audit-no-init-call-site.sh` 新設 / `.claude/rules/file-size-resolution.md` 新設 (= 機械的末尾切り出し禁止 procedure) / PRD doc Iteration v11 entry (改善 candidates v11-1〜v11-4) / TODO `[I-398]` 起票 (out-of-matrix hypothetical scenarios)) | 2026-05-08 | cells 11/23/75 e2e green、cell-77 既存 GREEN 維持、cell-41 + cell-79 Tier 2 lock-in、cells 16/30/36 transpile 単一 `.await` 出力 (e2e は T7-T8 scope)、INV-7 lock-in (post-T4 0 hits)、Hono bench 107/72 (Preservation 0/0 vs T4)、framework 改善 4 件 (file-size-resolution.md NEW + I-D batch v11-1/v11-3/v11-4 candidates 追加)、TODO I-398 起票 = 全 Layer 1〜4 0 findings + /check_problem 0 unresolved。詳細 = PRD doc Iteration v11 |
| **I-224 T5-1 + /check_job (3 iter) + /check_problem (2 round) cumulative structural fixes** (NonTrigger split / convert_expr passthroughs / per-declarator routing / classify_decl_var_path 削除 / TsAs/TsSatisfies/TsTypeAssertion expected_type propagation / destructuring Tier 2 honest reject / `prd-completion.md` Tier-transition wording 拡張 / TODO I-395-397 起票 / PRD Iteration v10 entry) | 2026-05-08 | i-224 e2e harness 40 fixtures wiring (14 GREEN + 27 ignored)、I-205 cell-09 unblock、INV-1 fill-in、cell-12/24 silent-drop Tier 1 fix、Hono bench 107/72 (Tier-transition Improvement compliance)。詳細 = PRD doc Iteration v9/v10 |
| **I-224 T1〜T4** (`__ts_main` collision validator / IR enums + helper / fn main synthesis + rename + substitute + Axis B/E probes / transform_module refactor + pub fn init 廃止 + namespace_lint extraction) | 2026-05-06〜07 | INV-2/3/4/5/6 invariants test fill-in、Spec への逆戻り Iteration v8 (T2 完了時 I-228 sub-entries 4 件)、Hono Tier-transition Preservation。詳細 = PRD doc Iteration v8 + git log |
| **I-205 T13** (B6/B7 corner cells Tier 2 reclassify lock-in + INV-5 Option B audit + 5 NEW integration tests) | 2026-05-01 | INV-5 visibility consistency (Option B、production 0 LOC change)、cargo lib 3358 |
| **I-205 T12** (Class Method Getter body C1 `.clone()` 自動挿入、Iteration v18 + v19) | 2026-05-01 | Decision Table C 完全 cover、cell 78 NA reclassify、cell 74 fixture rename、T16 + 別 PRD I-D 切り出し |
| **I-205 T10** (Internal `this.x` dispatch、E2 context、Iteration v16 + v17 deep-deep review) | 2026-05-01 | INV-2 (External/Internal dispatch path symmetry) 構造的達成、`body_requires_mut_self_borrow` recursive walker、TODO I-219〜I-223 起票 |
| **I-205 T9** (Logical compound `??=` `&&=` `\|\|=` Member target setter dispatch、Iteration v14 + v15) | 2026-04-30 | `ReceiverCalls` enum refactor、TODO I-219〜I-221 起票 |
| **I-205 T8** (Compound assign Member target setter dispatch + INV-3 1-evaluate compliance + DRY refactor + member_dispatch.rs 6-file split、Iteration v12) | 2026-04-29 | TODO I-217 / I-218 起票 |
| **環境整備** (4 file 構造的分割 + DRY refactor、行数超過解消) | 2026-04-29 | 4 → 27 file split、TODO I-393 / I-394 起票 |
| **PRD 2.7 (I-198 + I-199 + I-200 batch)** framework Rule 改修 + TypeResolver coverage extension + ast-variants.md Prop section 追加 + audit scripts CI 化 | 2026-04-27 | framework Rule 3/4/10/11/12 拡張、Implementation Revision 1 (PropOrSpread Grammar gap) + Revision 2 (cell 15 Prop::Assign Spec gap) self-applied integration |
| **I-184** (CI fresh-clone defect: stale gitignored template files post pool refactor) | 2026-04-27 | `.gitignore` asymmetric handling + Cargo.lock tracked、PRD `Background` に歴史的経緯記録の lesson |
| **I-177-E + I-177-B + I-177-F batch** (Plan η Step 1.5 + Step 2 + Step 2.5) | 2026-04-26 | `synthetic fork inheritance` fix + `FileTypeResolution` canonical primitive + arrow/fn-expr `visit_block_stmt` 統一 |
| **I-177-D** (TypeResolver `narrowed_type` suppression scope refactor、案 C、Plan η Step 1) | 2026-04-26 | trigger-kind-based dispatch refactor、Plan η framework 初実戦投入 |
| **I-178 + I-183 + Rule corpus optimization batch** | 2026-04-25 | matrix-driven PRD framework 整備 (10-rule checklist + 4-layer review + 5-category defect classification) |
| **I-161 + I-171 batch** (`&&=`/`\|\|=` desugar + Bang truthy emission) | 2026-04-22〜04-25 | I-177 umbrella 起票 (Tier 0 L1) |

---

## 次の PRD 着手前の参照ポイント

- **I-224 (現 PRD)**: PRD doc + Iteration v8/v9/v10/v11 entries (Spec への逆戻り audit trail、T6a 完了後の 2nd-round adversarial review は doc/CI structural fix のみで Spec 逆戻り不在 = Iteration log 追加なし)
- **I-225 / I-162 (次 PRD)**: TODO 内 entry + 案 β chain
- **I-205 T14-T16 (post case-β-1A)**: `backlog/I-205-getter-setter-dispatch-framework.md` の T11 削除 + 新 PRD I-A/I-B migration 注記
- **PRD I-D (Framework rule integration cohesive batch)**: I-205 close 後 deferred、I-224 v3〜v11 framework gap candidates + I-399 Spec stage 2nd-round candidate を集約 = **計 21 件** (含 T6a 2nd-round v11-5/6/7 + I-399 v11-8)
- **PRD 3 (I-177 本体)**: matrix-driven、案 A (mutation-ref) vs 案 B (writeback) を spec stage で empirical 確定
- **Phase A Step 5/6/7**: 「開発ロードマップ」section + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) (削除禁止 — 過去判断は reference として保持、実装乖離時は最新化)

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。

**完了済 (Step 0〜4 + I-153/I-154 + pre-Step-3)**: 詳細は git log + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) 参照。

**永続 skip (設計制約 4 件)**: `callable-interface-generic-arity-mismatch` / `indexed-access-type` / `vec-method-expected-type` / `external-type-struct`。

**effective residual (10 fixture)**: trait-coercion, any-type-narrowing, type-narrowing, instanceof-builtin, intersection-empty-object, closures, functions, keyword-types, string-methods, type-assertion。

**次の Step**:
```
Step 5 (型変換 + null)              I-026 + I-029 + I-030
Step 6 (string + intersection)     I-028 + I-033 + I-034
Step 7 (builtin impl)               I-071
```

| Step | 修正対象 | 主要 issue | unskip target |
|------|---------|-----------|---------------|
| 5 | 型 assertion / null/any 変換 / any-narrowing enum | I-026 / I-029 / I-030 | type-assertion, trait-coercion, any-type-narrowing |
| 6 | string method / intersection mapped type | I-028 / I-033 / I-034 | string-methods, intersection-empty-object, type-narrowing |
| 7 | builtin 型 impl 生成 (Date, RegExp 等) | I-071 | instanceof-builtin |

**残 fixture × 解消依存** (Step 経由不能): closures (I-048)、keyword-types (I-146)、functions (I-319)。

### Phase B: RC-11 expected type 伝播 (OBJECT_LITERAL_NO_TYPE)

Phase A 完了後、Hono ベンチマーク最大カテゴリ (全エラーの 45%) に着手。I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback)。

---

## リファレンス

- **最上位原則**: [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md)
- **優先度ルール**: [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md)
- **TODO 記載標準**: [`.claude/rules/todo-entry-standards.md`](.claude/rules/todo-entry-standards.md)
- **PRD workflow**: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md)
- **Spec stage 完了 verification**: [`.claude/rules/spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) (12-rule)
- **Implementation stage 完了 verification**: [`.claude/rules/check-job-review-layers.md`](.claude/rules/check-job-review-layers.md) (4-layer)
- **設計整合性**: [`.claude/rules/design-integrity.md`](.claude/rules/design-integrity.md) + [`.claude/rules/prd-design-review.md`](.claude/rules/prd-design-review.md)
- **完了基準**: [`.claude/rules/prd-completion.md`](.claude/rules/prd-completion.md) (Tier-transition compliance wording 含む)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **PRD handoff**: `doc/handoff/*.md`
- **Grammar reference**: `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md`
- **TODO 全体**: [`TODO`](TODO)
- **進行中 PRD doc**: [`backlog/I-224-top-level-fn-main-mechanism.md`](backlog/I-224-top-level-fn-main-mechanism.md)
- **ベンチマーク履歴**: `bench-history.jsonl`
- **エラー分析**: `scripts/inspect-errors.py`
- **実装調査 report**: `report/*.md`
