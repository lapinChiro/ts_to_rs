# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-23)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 111/158 (70.3%) |
| Hono bench errors | 63 |
| cargo test (lib) | 3085 pass (I-171 T4 で +60: bang_dispatch 46 + bang_assign_dispatch 14 に split、T4 /check_job deep deep + /check_problem review で IG-3/IG-4/IG-5/IG-6 critical bug 修正 + TG-2/3/4 test 拡張 込) |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 136 pass + 42 `#[ignore]` (existing 132 runtime E2E + 4 harness isolation/helper tests; default parallel `cargo test --test e2e_test` stable after isolated runner pool fix) |
| clippy | 0 warnings |
| fmt | 0 diffs |

**Note (2026-04-21)**: T6-4/T6-5 commit message は Hono bench 113/158 clean / 60 errors を報告したが、T6-6 empirical 再測 (clean rebuild × 複数 run) では 112/158 / 62 errors が stable な値。同一 HEAD + 同一ソースで bench に ±1 clean / ±2 errors の non-deterministic variance が発生。**I-144 前後の stable 値 net change = 0 errors**。当初 HashMap iteration order を疑ったが empirical 調査で否定 (`expr_types.get(&span)` 等は lookup only で emission 非影響)。候補 root cause は `std::fs::read_dir` の platform-dependent order / bench script の `find | xargs cp` / `module_graph` の cross-module resolution のいずれか (要調査)。pre-existing 非決定性を I-172 として TODO 起票、I-144 scope 外で別 PRD 扱い。

**Note (2026-04-22 T3)**: I-161 T3 完了時点で Hono bench 再測、clean 112/158 / errors 62 で pre-T3 と完全一致 (regression 0)。I-161 は narrow-related compile error (`&&=`/`||=` on non-bool LHS) の structural fix であり、Hono 現 bench の error category (OBJECT_LITERAL_NO_TYPE 28 + OTHER 15 + CALL_TARGET 4 + ...) には該当しないため数値無変動が ideal-implementation-primacy.md 通りの想定挙動。

**Note (2026-04-23 T4、I-172 再顕在化)**: I-171 T4 完了時点で Hono bench 再測、**T4 差分 empirical 検証** (pre-T4 binary.rs/truthy.rs を git show で取り出し release build) も含めて **111/158 clean / 63 errors** に stable 化。T3 commit (cba5f62) の 112/62 vs T4 commit の 111/63 は pre-T4 ソースでも同じ 111/63 を再現するため T4 regression 0 を empirical 確認済。I-172 の ±1 clean / ±2 errors variance が今回再顕在化した事実のみ記録。category diff (OBJECT_LITERAL_NO_TYPE 28→27 / OTHER 15→17) の 2 件 "compound logical assign on unresolved X" は T3 の `UnsupportedSyntaxError` categorization shift。

### 進行中作業

#### テスト基盤強化

- High: E2E の共有状態がまだ残っています。tests/e2e_test.rs:291 と tests/e2e_test.rs:558 で {name}_exec.ts / main_exec.ts を fixture directory に固定名で書いています。runner は分離されましたが、TS
    実行用一時ファイルは共有 path のままなので、同一 fixture の同時実行や aggregate/per-cell の重複有効化で削除・上書き race が起き得ます。check_job.md の「妥協なし」基準では、runner-local か unique
    temp path に寄せるべきです。
- Medium: tests/e2e_test.rs:1 が 1640 LOC まで増え、既知課題 TODO:687 の 1000 LOC policy 違反を悪化させています。現行 scripts/check-file-lines.sh:1 は src/ しか見ないため検出されませんが、
    CLAUDE.md 上の project-wide 方針とは不整合です。E2E harness と test cases を分割し、可能なら line check の対象を tests にも広げる必要があります。

#### 開発

**I-161 + I-171 batch PRD** (2026-04-22〜) — Spec stage 完了、T2 (共有 helper) + T3 (I-161 `&&=`/`||=` desugar) + **T4 (I-171 Layer 1 Bang arm)** 完了。T5-T8 着手待ち。`backlog/I-161-I-171-truthy-emission-batch.md` + `report/i161-i171-t1-red-state.md` (v6) + 26 tsc observations + 60 E2E fixtures + test harness 登録 60 test function。

**T4 完了範囲 (2026-04-23、/check_job deep deep review 後の最終状態)**:
- `convert_unary_expr` Bang arm を type-aware dispatch `convert_bang_expr` に分離 (`src/transformer/expressions/binary.rs`)
- 5 layer dispatch: (1) peek-through (Paren/TsAs/TsNonNull/TsTypeAssertion/TsConstAssertion), (2) `try_constant_fold_bang` literal + Arrow/Fn const-fold, (3) double-neg `!!<e>` → `truthy_predicate_for_expr` + **literal 経路 recursive const-fold** (IG-1 fix、`try_constant_fold_bang(inner) = Some(BoolLit(b))` なら `BoolLit(!b)` 返却、TypeResolver 非依存で decidable)、(3b) De Morgan on `Bin(LogicalAnd/LogicalOr)` at AST layer、(3c) Assign desugar `{ let tmp=rhs; x=tmp; <falsy(tmp)> }`、(4) general `falsy_predicate_for_expr`、(5) fallback raw `!<operand>` (Any/TypeVar explicit error surface)
- T2 helper structural fix: `predicate_primitive_with_tmp` を **ref-count-aware** に改修。`Bool`/`String`/`Primitive(int)` predicate は operand 1 回参照のみのため tmp bind 不要、**F64 のみ** (`<op> == 0.0 || <op>.is_nan()` 2 ref) tmp bind 発動。既存 snapshot regression (noise 削減)
- E2E empirical verify: I-171 B cell 20 un-ignore → **15 GREEN 化** (cell-b-bang-{f64-in-ret / string-in-ret / option-number-in-ret / bin-expr / double-option / tsas / int / option-named / named / vec / nc / cond / this / tstypeassertion / tsconstassertion})。残 5 cell は pre-existing defect blocker (cell-b-bang-logical-and: I-177 narrow / cell-b-bang-option-union: I-179 NaN→synthetic union coercion / cell-b-bang-assign/update: I-181 tuple destructuring + ternary `&str`/`String` / cell-b-bang-await: I-180 async-main e2e harness) として blocker annotation 付き re-ignore。T4 emission は全 5 cell で semantically correct (empirical 読み合わせ済)
- **Layer 3c Assign desugar structural 再設計 (IG-3 / IG-4 / IG-5 — deep deep review 後の追加 fix) + Layer 3 double-neg recurse (IG-6 — /check_problem で追加発見)**:
  - **IG-3**: tmp の type annotation を LHS 型に修正 (`rhs_ty` → `lhs_ty` via `assign_target_type(&AssignTarget)` helper)。TypeResolver の expected-type wrap で `Some(...)` 化された `value` IR と matching する `Option<T>` annotation が正しく emit される。Pre-fix: `let tmp: f64 = Some(5.0)` E0308 → Post-fix: `let tmp: Option<f64> = Some(5.0)` clean
  - **IG-4**: 非 Copy LHS で `x = tmp.clone()` を emit、tmp を predicate 用に保持 (`is_copy_type()` 判定)。Copy LHS (`f64`/`bool`/`int`/`Option<f64>`/Copy tuple) は bare `x = tmp`。Pre-fix: String LHS で E0382 use-after-move → Post-fix: `.clone()` で tmp 存続
  - **IG-5**: AST op check (`assign_expr.op != Assign`) を IR shape check に置換。arithmetic/bitwise compound (`+=`/`-=`/`*=`/`/=`/`%=`/bitwise) は `convert_assign_expr` で `Expr::Assign { target, BinaryOp(target, op, rhs) }` に normalise されるため Layer 3c で正しく desugar (`!(x += v) = !<new x>`)。`&&=`/`||=`/`??=` は non-Assign IR (If/Block) を emit するため destructure fail で自然 skip (conditional semantics 保持)
  - **TypeResolver 拡張 (structural 前提条件)**: arithmetic/bitwise compound assign の LHS ident を `record_assign_target_ident_type` で `expr_types` に記録 (従来は Logical compound のみ)。expected-type propagation は既存 set 維持、LHS 型 lookup のみ拡大。I-175 expected-type coercion gap は継続 (orthogonal)
  - **IG-6 (/check_problem で追加発見)**: Layer 3 double-neg で inner operand が `Assign` / `Bin(LogicalAnd/LogicalOr)` の場合、direct `truthy_predicate_for_expr` 経路は無効 Rust (`<Assign>.method()` / `<Option<T> && Option<U>>`) を emit する。`needs_bang_recurse` 判定を追加、該当 shape では `convert_bang_expr(&inner.arg)` を recurse して outer `Not` で wrap。Layer 3b De Morgan / Layer 3c Assign desugar が先に発動し、outer Not と合わさって正しい truthy 意味論 (`!<Block>` / `!(<a falsy> || <b falsy>)`) を emit
  - **Empirical compile verify**: `/tmp/bang_probe/{option_lhs,string_lhs,option_string_lhs,compound_add,double_neg_assign,double_neg_logical}.rs` で pre-fix 出力の E0308/E0382 + post-fix 出力の clean compile & runtime 正解 (TS と一致) を確認
- T2 helper structural fix: `predicate_primitive_with_tmp` を **ref-count-aware** に改修 (F64 のみ 2-ref で tmp bind)
- E2E empirical verify: I-171 B cell 20 un-ignore → **15 GREEN 化**。残 5 cell は pre-existing defect blocker (I-177/I-179/I-180/I-181) として blocker annotation 付き re-ignore。T4 emission は全 5 cell で semantically correct
- Unit test: **60 case** を 2 module に cohesion split:
  - `bang_dispatch.rs` (46 case): Layer 2 const-fold 12 + peek-through 6 + Layer 3 double-neg 5 (IG-1 literal fold 4 追加 + untyped fallback 1) + Layer 3b De Morgan 2 + Layer 5 fallback 1 + B.1 shape dispatch 18 (Member/OptChain/Unary -+typeof/Bin arith/comp/bitwise/InstanceOf/In/NC/Call/Cond/New/Await/Array/Tpl/This/Update) + その他 2
  - `bang_assign_dispatch.rs` (14 case): Layer 3c Assign desugar primitive f64 + arithmetic compound (IG-5 regression) + logical compound skip + Option<F64> (IG-3 regression) + String (IG-4 regression) + Option<String> (IG-3+IG-4 combo) + Named struct always-truthy + unresolved target fallback + typed double-neg Option<F64>/Option<String> (TG-4) + **IG-6 regression 4 case** (double-neg on Assign → Layer 3c inversion / LogicalAnd → De Morgan inversion / LogicalOr → De Morgan inversion / arithmetic compound `+=` × double-neg)
- 44 T2 truthy helper test と合わせ **104 dispatch case** (PRD 目標 ~95 以上を達成)
- SI-1 PRD Matrix 更新: backlog/I-161-I-171 Matrix B の 5 blocker cell (B.1.19 IG-1 / B.1.23 / B.1.32 / B.1.33 / B.1.36 / B-T6) に Dual verdict + B.1.8 NaN AST 到達不能 note + B.1.16 OptChain implementation equivalence note を追加
- Quality gate 全 pass: cargo test **3085** lib + 122 integration + 3 compile + 132 E2E + 42 ignored、clippy 0 warnings、fmt 0 diffs、file-lines OK (bang_dispatch.rs 857 + bang_assign_dispatch.rs 518 各 ≤ 1000)、Hono bench 111/158 / 63 errors (T4 full regression 0 empirical 検証、TypeResolver 拡張も既存処理 non-regression 確認)

**T3 完了範囲 (2026-04-22)**:
- T3-TR: TypeResolver `AndAssign`/`OrAssign` expected propagation 追加 (`src/pipeline/type_resolver/expressions.rs` + `rhs_expected_for_compound` helper + 7 unit test)
- T3 本体: `convert_assign_expr` AndAssign/OrAssign arm を conditional-assign desugar に置換、stmt-context 用 `try_convert_compound_logical_assign_stmt` intercept + expr-context block form + narrow-binding mutability post-pass (`mutability.rs::mark_mutated_narrow_bindings`)
- T2 helper: `truthy_predicate_for_expr` / `falsy_predicate_for_expr` / `TempBinder` / `is_always_truthy_type` / `try_constant_fold_bang` / `peek_through_type_assertions` (+ 44 helper unit test)
- Matrix A/O primary 84 + A.5 expr-context cross + Tier 2 NA + blocked LHS error-path = **95 unit test** (`src/transformer/statements/tests/compound_logical_assign/`)

**T3 scope 分割 (完全性保持のため新 PRD 分岐)**:
- **I-177 (新 PRD、narrow emission v2)**: I-144 T6-3 inherited の shadow-mutation-propagation 欠陥を structural fix する prerequisite PRD。I-161 narrow-alive cells + I-171 C-15n/C-16n narrow-scope cells + T4 で発見の cell-b-bang-logical-and の prerequisite。TODO 参照 🔗 I-177。
- **I-178 (新 PRD、spec-first-prd Checklist 拡張)**: `spec-first-prd.md` Spec-Stage Adversarial Review Checklist に 6 項目目「Matrix/Design integrity」を追加する framework 改善。I-161 SG-2 empirical lesson 由来。TODO 参照 🔗 I-178。
- **I-179 (新 TODO、synthetic union literal coercion at call args、T4 empirical 発見)**: `f(NaN)` call 時に `NaN` literal が `F64OrString::F64(f64::NAN)` として wrap されず raw `f64::NAN` で emit、Option<synthetic union> expected 型と mismatch。cell-b-bang-option-union E2E blocker。
- **I-180 (新 TODO、e2e harness async-main execution semantics、T4 empirical 発見)**: TS top-level `main();` + `async function main()` が tsx 実行で stdout 2 倍出力、fixture .expected と不一致。cell-b-bang-await E2E blocker。
- **I-181 (新 TODO、tuple destructuring `.get(N)` + ternary `&str`/`String`、T4 empirical 発見)**: `const [a,b] = fn()` が tuple return に対し `.get(0).cloned().unwrap()` (array indexing syntax) で emit + 三項演算子 `"falsy"`/`"truthy"` が `&str` のまま `(String, f64)` に渡り型不一致。cell-b-bang-assign/update E2E blocker。

**PRD spec 訂正 (2026-04-22)**: SG-1 (Matrix A-5 / O-5 / B.2 T5 の `*v` → `v` 訂正、`is_some_and(|v: T|...)` は T by-value ABI)、SG-2 (Matrix ideal column と Design section emission form の統一、Matrix A-6/O-5/O-5s/O-6 を predicate helper 形に更新)、SG-3 (Matrix A.4 narrow × compound assign sub-matrix 追加、I-177 依存で cell 別 deferred annotation)。

T5 (Layer 2 `try_generate_option_truthy_complement_match` 拡張) / T6 (broken window fix P1-P4 + E2E un-ignore) / T7 (classifier 相互検証) / T8 (全 quality gate + PRD 完了処理) の順で継続。narrow-scope 関連は I-177 完了後に T3-N / T7-N として回帰。

### 直近の完了作業

実装詳細は git log / `backlog/` (close 後 archive)、設計判断は
[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD | 日付 | サマリ (1-3 行) |
|-----|------|-----------------|
| **File line-count reduction refactor (8 files)** | 2026-04-21 | 1000 LOC 超過 8 file を cohesion-driven split (21 files changed, +1964 / −8767 LOC net)。Phase 1 test files (build_registry 1123→6 / control_flow 1095→7 / generator/tests 1068→8 / switch 1028→7 / generator/expressions/tests 1019→8) + Phase 2 production files (registry/collection 1524→8 sub-dir with placeholder/decl/class/resolvers/type_literals/const_values/callable / ts_type_info/mod 1045→3 files helpers+tests / transformer/expressions/methods 1267→3 sub-dir mod+closures+tests)。visibility `pub(in crate::registry)` で original `pub(super)` scope を厳密保持。`check-file-lines.sh` OK、quality gate 全 pass、Hono bench 非後退。post-review で `map_method_call` 411 LOC 単一 match decomposition を I-174 として起票 (L4)。計画詳細は git log 参照 |
| **I-144 (control-flow narrowing analyzer umbrella)** | 2026-04-19〜04-21 | CFG-based narrowing analyzer PRD (umbrella: I-024 / I-025 / I-142 Cell #14 / C-1 / C-2a-c / C-3 / C-4 / D-1 吸収) を 9 sub-phase (T0-T6-6) で完了。T0-T2 SDCDF Spec stage (matrix-driven + Dual verdict framework) + T3-T5 analyzer 基盤 (`pipeline/narrowing_analyzer/` + `NarrowEvent` enum + `NarrowTypeContext` trait) + T6-1〜T6-5 emission 実装 (EmissionHint dispatch / coerce_default / truthy E10 / OptChain compound narrow / implicit None tail) + T6-6 close で 7 連鎖 review 11 structural fix (IMPL-1〜7 YAGNI dead variant/field 除去 + `transformer/mod.rs` 1117→718 LOC cohesion 分割)。matrix 全 9 ✗ cell GREEN。設計判断は `doc/handoff/design-decisions.md` section「Control-flow narrowing analyzer (I-144)」8-section archive、sub-phase 実装詳細は git log 参照 |
| **I-153 + I-154 batch + 以前の完了** | 2026-04-19 以前 | I-153 / I-154: switch case body nested `break` silent redirect の structural 解消 + internal label `__ts_` prefix 統一 (`report/i153-switch-nested-break-empirical.md`)。以前: I-SDCDF (spec-first framework、beta)、I-050-a (SDCDF Pilot)、Phase A Step 3/4 (I-020 部分/I-023/I-021)、I-145 / I-150 batch、INV-Step4-1、I-142 (`??=`) / I-142-b+c、I-022 (`??`) / I-138 / I-040 / I-392 ほか。git log で参照可能 |

### 次の作業 (I-144 完了後 2026-04-21、spec-first workflow 適用)

**優先順位は `.claude/rules/todo-prioritization.md` (L1 > L2 > L3 > L4) および
`.claude/rules/ideal-implementation-primacy.md` (silent semantic change を最優先) に従う。**

**Tier 0 (L1 silent) 該当なし**。**Tier 1 (L2 Struct) 該当なし** (I-144 完了で解消)。

**着手順の導出原則**:
1. I-144 Dual verdict framework で `TS ✓ / Rust ✗` として分離された narrow-related compile error は I-144 context が fresh なうちに優先 (I-161 / I-162 / I-171)
2. Phase A roadmap (Step 5 → Step 6 → Step 7) で compile_test skip 直接削減
3. Phase B (RC-11 OBJECT_LITERAL_NO_TYPE 28件 = Hono 全 error の 45%) は Phase A 完了後
4. L4 latent items (runtime 同一 / reachability なし) は notes 欄に退避

| 優先度 | レベル | PRD | 内容 | 根拠 |
|--------|-------|-----|------|------|
| — | L3 | **I-161 + I-171 batch** | 進行中: T2 (helper) + T3 (`&&=`/`||=` desugar、non-narrow scope) + **T4 (Bang arm type-aware dispatch、2026-04-23 完了、15 E2E GREEN 化)** 完了。T5 (Layer 2 `try_generate_option_truthy_complement_match` 拡張) 着手待ち。narrow-scope cells は I-177 完了後に回帰 | 上記「進行中作業」参照 |
| 1 | **L2** | **I-177 (新、narrow emission v2)** | I-144 T6-3 inherited の shadow-mutation-propagation 欠陥を structural fix。`if let Some(x) = x { body }` 形式が body mutation を outer `Option<T>` に propagate しない pre-existing defect。I-161 narrow cells (A.4 / A-6 / O-6 / T7-*) の prerequisite | I-161 T3 実装で latent defect が runtime 誤動作として顕在化。Design Foundation (narrow emission 基盤) のため L2 格上げ |
| 2 | L3 | **I-162** | class without explicit constructor → `Self::new()` 自動合成 | I-144 T2 instanceof narrow の Rust 側 E2E lock-in が本 defect で block。`class Dog {}` → `struct Dog {}` 止まりで `Dog::new()` 不在で E0599 |
| 3 | L3 | **Phase A Step 5** (I-026 / I-029 / I-030) | 型 assertion / null as any / any-narrowing enum 変換 | `type-assertion`, `trait-coercion`, `any-type-narrowing` unskip (3 fixture 直接削減) |
| 4 | L3 | **I-178 (新、spec-first-prd Checklist 拡張)** | Spec-Stage Adversarial Review Checklist に 6 項目目「Matrix/Design integrity」追加 (framework 改善) | I-161 SG-2 empirical lesson 由来 (Matrix ideal column と Design section emission shape 乖離の silent PRD inconsistency 検出力不足) |
| 5 | L3 | **I-015** | Hono types.rs `Input['out']` indexed access 解決失敗 (E0405) | `src/ts_type_info/resolve/indexed_access.rs:271`。Hono types.rs で 1 件だが dir compile blocker |
| 6 | L3 | **I-158 + I-159 batch** | Non-loop labeled stmt + 内部 emission 変数 user namespace hygiene | I-154 変数版 + I-153 labeled block 対応。I-158 が I-153 emission と interaction のため I-158 先行推奨 |
| 7 | L3 | **Phase A Step 6** (I-028 / I-033 / I-034) | intersection 未使用型パラメータ (E0091) + charAt/repeat/toFixed method 変換 | `string-methods`, `intersection-empty-object`, `type-narrowing` unskip |
| 8 | L3 | **I-143 meta-PRD** | `??` 演算子の問題空間完全マトリクス + 8 未解決セル (a〜h) | I-143-a〜h 未着手。I-143-b (`any ?? T`) は I-050 依存、他は独立 |
| 9 | L3 | **I-142 Step 4 C-5 / C-6 + Phase A Step 7 (I-071)** | I-144 非吸収の small cleanup (C-7 は I-050 依存) + `instanceof-builtin` unskip 用 builtin 型 impl 生成 | C-5/C-6 は test quality 改善 (handoff doc)、I-071 は Phase A 最終 step (1 fixture unskip) |
| 10 | L3 | **Phase B (RC-11)** (I-003 / I-004 / I-005 / I-006) | expected type 伝播の不完全性 (OBJECT_LITERAL_NO_TYPE 28件) | Hono 全 error の 45%、Phase A 完了後の最大インパクト category |

**注**: 本テーブルは着手順。各 PRD で `prd-template` skill + `.claude/rules/problem-space-analysis.md`
+ `.claude/rules/spec-first-prd.md` を適用する。

### 次点 / L4 deferred (上記 table 外)

table に入らなかった L3 / L4 items:

- **I-013 + I-014 batch** (L3、RC-5 abstract class 変換パス欠陥) — class inheritance 系、抱え込み依存が強いため独立 PRD 着手時に整備
- **I-140** (L3、TypeDef::Alias variant 追加) — `type MaybeStr = string \| undefined` alias 経由の Option 認識。I-134 / I-056 と batch 可能
- **I-050 umbrella** (L3、Any coercion) — I-143-b + I-050-b + I-050-c が依存。structural 母体として設計維持
- **I-146** (L3、`return undefined` on void fn) — `keyword-types` unskip の残条件
- **I-048** (L3、所有権推論) — RC-2 根本解決、`closures` / `functions` unskip の残条件、修正規模大
- **I-074** (L4、`Item::StructInit` broken window) — pipeline-integrity 違反、PRD 化候補
- **I-160** (L4、Walker defense-in-depth Expr-embedded Stmt::Break) — 現時点 reachability なし
- **I-165 / I-166 / I-167 / I-170** (L4 narrow precision umbrella) — I-144 後の latent imprecision、runtime 動作同一、Rust 精度のみ向上
- **I-168** (L4、`NarrowEvent::Reset` event 未消費) — Hono で顕在化なし pre-existing imprecision
- **I-172** (L4、bench 非決定性) — test / bench infra、別 PRD

### Batching 検討 (2026-04-21)

- ✅ **完了**: I-144 + I-142 Step 4 C-1〜C-4+D-1 (I-144 で一括吸収)
- **I-161 + I-171**: narrow-related truthy compile error。`truthy_predicate_for_expr` 汎用 helper + `if (!x)` 経路拡張を共有基盤として構築 (新規 batch proposal)
- **I-158 + I-159**: namespace hygiene 系 (I-154 と同系)。I-158 先行推奨 (I-153 emission との interaction)
- **I-143 + I-050-b + I-050-c**: `??` / Any / Synthetic union coercion が共通 `resolve_expr` / `propagate_expected` 基盤を持つ
- **I-140 + I-134 + I-056**: type alias 関連、`TypeDef::Alias` variant 新設で DRY 可能
- **I-013 + I-014**: abstract class 変換パス (強依存、`generate_child_of_abstract()` 拡張)
- **I-165 / I-166 / I-167 / I-170**: narrow precision umbrella (`VarId` binding identity + CFG analysis の基盤を共有)
- **I-050 umbrella** (`backlog/I-050-any-coercion-umbrella.md`) は design 母体として存続

### INV 状態

- INV-Step4-1: ✅ 完了 (`report/i142-step4-inv1-closure-compile.md`)
- INV-Step4-2: ✅ **消失確認で close** (2026-04-19、observation 対象だった `utils/concurrent.ts:12` の OBJECT_LITERAL_NO_TYPE regression が現 bench で検出されず。bisection 不要、`doc/handoff/I-142-step4-followup.md` C-9 section に empirical 解消記録)
- I-153 問題空間: ✅ 完了 (`report/i153-switch-nested-break-empirical.md`)

---

## 次の PRD 着手前の参照ポイント

次期 PRD 着手時、以下を参照:

- **Phase A Step 5 / 6 / 7**: 下記「開発ロードマップ」 section + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **I-144 設計判断 (archive)**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) の CFG narrowing analyzer / NarrowTypeContext trait / EmissionHint dispatch / coerce_default table / closure reassign Policy A section
- **I-142 Step 4 残余 (C-5〜C-9)**: [`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md)
- **I-158 / I-159 (hygiene follow-ups)**: TODO 参照
- **I-143 meta-PRD (`??` 完全仕様)**: TODO I-143 本体 + a〜h 未解決セル

新規 PRD 着手時は `prd-template` skill + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md) + [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) を適用する。

---

## 設計判断の引継ぎ

後続 PRD 向けの設計判断アーカイブは **[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)** に集約。

含まれる topic (要約):

- **Type scope 管理**: `push_type_param_scope` の設計理由
- **Primitive type 9 variant YAGNI 例外**
- **Switch emission と label hygiene (I-153/I-154)**: `__ts_` prefix convention、walker 設計、conditional wrap、Block flatten、is_literal_match_pattern 微変化
- **Optional param 収束設計 (I-040)**: `wrap_if_optional` 単一ヘルパー、全 10 emission 経路
- **Conversion helpers (RC-2)**: remapped methods / `produces_option_result` / strictNullChecks / FieldAccess parens
- **Error handling emission**: TryBodyRewrite exhaustive capture / I-023 short-circuit / 協調 / union return 実行順序 (RC-13)
- **DU analysis (Phase A Step 4)**: walker single source of truth / Tpl children visit
- **Control-flow narrowing analyzer (I-144)**: 2-channel architecture (NarrowEvent via guards / EmissionHint dispatch / du_analysis) / `NarrowTypeContext` trait / 3-variant `NarrowEvent` enum + 2-layer `NarrowTrigger` / `coerce_default` table / closure reassign Policy A / Dual verdict framework / `ir_body_always_exits` / **YAGNI 厳守方針 (actually-populated のみ enum variant 化)** / `transformer/mod.rs` cohesion 分割 (helpers/option_builders / injections / ts_enum)
- **Lock-in テスト (削除禁止)**: 保護対象テスト一覧
- **残存 broken window**: Item::StructInit 等、`transformer/mod.rs` 以外の pre-existing file-size violation 8 件

新規 PRD 着手時は関連 section を事前レビュー。実装が設計判断と乖離していたら該当 section を
最新化 (削除は禁止 — 過去の設計判断は reference として保持)。

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。
skip 解消後は新たな skip 追加を原則禁止とし、回帰検出を自動化する。

**完了済み:**

- Step 0: `basic-types` unskip
- Step 1 (RC-13): `union-fallback`, `ternary`, `ternary-union` unskip + `external-type-struct` (with-builtins) unskip
- Step 2: `array-builtin-methods` unskip + `closures` の I-011 filter 参照セマンティクス解消
- **Pre-Step-3**: I-138 (Vec index Option) + I-022 (`??`) + I-142 (`??=` Ident LHS) — Tier 1 silent bug を pre-Step として解消、`nullish-coalescing` fixture unskip
- **Step 3** (2026-04-17): I-020 部分 + I-025、`void-type` unskip
- **Step 4** (2026-04-17): I-023 + I-021、`async-await` + `discriminated-union` unskip
- **I-153 + I-154 batch** (2026-04-19): switch case body silent redirect + label hygiene structural fix + A-fix (Block stmt support)

**永続 skip (設計制約 4件):**

- `callable-interface-generic-arity-mismatch` — 意図的 error-case (INV-4)
- `indexed-access-type` — マルチファイル用 (`test_multi_file_fixtures_compile` でカバー)
- `vec-method-expected-type` — no-builtins mode 限定の設計制約
- `external-type-struct` — no-builtins mode 限定の設計制約 (with-builtins 側は Step 1 で解消済)

**effective residual (10 fixture):**

trait-coercion, any-type-narrowing, type-narrowing, instanceof-builtin,
intersection-empty-object, closures, functions, keyword-types, string-methods, type-assertion

#### 次の Step

```
I-144 (L2 struct、CF narrowing)      ✅ 完了 2026-04-21 (I-024/I-025/I-142 Cell #14/C-1〜C-4/D-1 吸収)
  ↓
Step 5 (type conversion + null)       I-142 Step 4 C-5〜C-7 残余処理 (C-8 / C-9 完了済、並行可能)
  ↓ I-158 / I-159 (hygiene follow-ups、並行可能)
Step 6 (string + intersection)        type-narrowing は Step 1 + 6 で完全解消
  ↓
Step 7 (builtin impl)
```

#### Step 5-7 の予定 (未着手)

**Step 5: 型変換 + null セマンティクス** — Tier 2、型変換パイプライン

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-026 | 型 assertion 変換 | `as unknown as T` の中間 `unknown` を消去して直接キャスト |
| I-029 | null/any 変換 | `null as any` → `None` が `Box<dyn Trait>` 文脈で型不一致 |
| I-030 | `build_any_enum_variants()` (`any_narrowing.rs:85`) | any-narrowing enum の値代入で型強制 |

- unskip: `type-assertion`, `trait-coercion`, `any-type-narrowing`

**Step 6: string メソッド + intersection** — Tier 2、独立した小修正群

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-033 | `methods.rs` | `charAt` → `chars().nth()`, `repeat` → `.repeat()` マッピング追加 |
| I-034 | `methods.rs` | `toFixed(n)` → `format!("{:.N}", v)` 変換 |
| I-028 | `intersections.rs:132-145` | mapped type の非 identity 値型で型パラメータ T が消失 (E0091) |

- unskip: `string-methods`, `intersection-empty-object`, `type-narrowing`

**Step 7: ビルトイン型 impl 生成** — Tier 2、大規模

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-071 | `external_struct_generator/` + generator | ビルトイン型（Date, RegExp 等）の impl ブロック生成 |

- unskip: `instanceof-builtin`

#### fixture × Step 解消マトリクス

| fixture | 解消 Step / 依存 | メモ |
|---------|-----------------|------|
| ~~basic-types~~ | ~~Step 0~~ | — |
| ~~union-fallback~~ / ~~ternary~~ / ~~ternary-union~~ | ~~Step 1~~ | — |
| ~~external-type-struct (with-builtins)~~ | ~~Step 1~~ | — |
| ~~array-builtin-methods~~ | ~~Step 2~~ | — |
| ~~void-type~~ | ~~Step 3~~ | — |
| ~~async-await~~ / ~~discriminated-union~~ | ~~Step 4~~ | — |
| ~~nullish-coalescing~~ | ~~pre-Step-3 (I-022 + I-142)~~ | — |
| closures | I-048 (所有権推論) | I-020 Box wrap 解消済、残: move/FnMut |
| keyword-types | I-146 | I-025 implicit None 解消済、残: `return undefined` on void |
| functions | I-319 (Vec index move) | I-020 Box wrap 解消済 |
| type-assertion / trait-coercion / any-type-narrowing | Step 5 | — |
| string-methods / intersection-empty-object | Step 6 | — |
| type-narrowing | Step 6 | Step 1 (I-007) 依存済 |
| instanceof-builtin | Step 7 | — |
| vec-method-expected-type | — | 設計制約 (永続 skip) |
| external-type-struct (no-builtins) | — | 設計制約 (永続 skip) |

### Phase B: RC-11 expected type 伝播 (OBJECT_LITERAL_NO_TYPE 28件)

Phase A 完了後、Hono ベンチマーク最大カテゴリ（全エラーの 45%）に着手。
I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback) を対象とする。
(件数: 2026-04-21 T6-6 後 bench 実測 62 errors 中 28 件、I-144 前後で変動なし)

---

## リファレンス

- 最上位原則: [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md)
- 優先度ルール: [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md)
- TODO 記載標準: [`.claude/rules/todo-entry-standards.md`](.claude/rules/todo-entry-standards.md)
- PRD workflow: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md)
- 設計整合性: [`.claude/rules/design-integrity.md`](.claude/rules/design-integrity.md) + [`.claude/rules/prd-design-review.md`](.claude/rules/prd-design-review.md)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- PRD handoff: `doc/handoff/*.md` (I-142 Step 4 follow-up 等)
- Grammar reference: `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md`
- TODO 全体: [`TODO`](TODO)
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
- 実装調査 report: `report/*.md`
