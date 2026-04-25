# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-25)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 111/158 (70.3%) (post-T7-revert 実測、T6 完了時 112/158 から I-172 ±1 clean noise variance、T7 fix が Hono に作用する pattern 不在のため revert と無関係) |
| Hono bench errors | 63 (post-T7-revert 実測、T6 完了時 62 から I-172 ±1 errors noise variance) |
| cargo test (lib) | 3121 pass (T5 baseline 3106 から +15: T6 P3a/P3b 新規 narrow analyzer test 5 + T6 P1 helpers test 7 + T7-6 narrow × incompatible RHS error-path 2 + T7 positive boundary lock-in 1 (`narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign`、closure-reassign 不在で `!== null + early-return` が if-let-Some shadow を emit する pre-T7 / pre-I-177-D 状態の lock-in)。T7 で試行した workaround patch (NonNullish !== closure-reassign suppression branch + 関連 lock-in test 3 件) は **deep deep `/check_job` で発見した Scenario A regression を回避するため revert (2026-04-25)**、architectural fix を I-177-D PRD に委譲。T6 で P1 (`generate_truthiness_condition`/`generate_falsy_condition` を `truthy_predicate_for_expr` 経由全型網羅化) + P2 (`try_generate_primitive_truthy_condition` peek-through) + P3a (Bang arm peek-through) + P3b (Bang arm OptChain narrow event push) + P4 (test 拡張) を実装、`narrowing_analyzer/tests/guards.rs` を folder-module 分割 (mod.rs 471 + early_return.rs 569) |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 155 pass + 28 `#[ignore]` (T5 baseline 151+32 から +4 GREEN / -4 ignored; T7 で T7-1/T7-2/T7-4/T7-5 の 4 cell un-ignore (`&&=` / `\|\|=` × narrow F64 / 連鎖 / synthetic union、closure-reassign 不在で T7 fix 不発火、un-ignore は valid)。T7-3 (`&&=` × closure-reassign × narrow-suppression) は T7 workaround patch revert で pre-T7 RED 状態 (IR/TypeResolver cohesion gap + closure-mutable-capture E0506)、I-177-D で architectural fix 予定のため re-ignore、annotation を I-177-D dependency 反映に更新) |
| clippy | 0 warnings |
| fmt | 0 diffs |

**Note (2026-04-21)**: T6-4/T6-5 commit message は Hono bench 113/158 clean / 60 errors を報告したが、T6-6 empirical 再測 (clean rebuild × 複数 run) では 112/158 / 62 errors が stable な値。同一 HEAD + 同一ソースで bench に ±1 clean / ±2 errors の non-deterministic variance が発生。**I-144 前後の stable 値 net change = 0 errors**。当初 HashMap iteration order を疑ったが empirical 調査で否定 (`expr_types.get(&span)` 等は lookup only で emission 非影響)。候補 root cause は `std::fs::read_dir` の platform-dependent order / bench script の `find | xargs cp` / `module_graph` の cross-module resolution のいずれか (要調査)。pre-existing 非決定性を I-172 として TODO 起票、I-144 scope 外で別 PRD 扱い。

**Note (2026-04-22 T3)**: I-161 T3 完了時点で Hono bench 再測、clean 112/158 / errors 62 で pre-T3 と完全一致 (regression 0)。I-161 は narrow-related compile error (`&&=`/`||=` on non-bool LHS) の structural fix であり、Hono 現 bench の error category (OBJECT_LITERAL_NO_TYPE 28 + OTHER 15 + CALL_TARGET 4 + ...) には該当しないため数値無変動が ideal-implementation-primacy.md 通りの想定挙動。

**Note (2026-04-23 T4、I-172 再顕在化)**: I-171 T4 完了時点で Hono bench 再測、**T4 差分 empirical 検証** (pre-T4 binary.rs/truthy.rs を git show で取り出し release build) も含めて **111/158 clean / 63 errors** に stable 化。T3 commit (cba5f62) の 112/62 vs T4 commit の 111/63 は pre-T4 ソースでも同じ 111/63 を再現するため T4 regression 0 を empirical 確認済。I-172 の ±1 clean / ±2 errors variance が今回再顕在化した事実のみ記録。category diff (OBJECT_LITERAL_NO_TYPE 28→27 / OTHER 15→17) の 2 件 "compound logical assign on unresolved X" は T3 の `UnsupportedSyntaxError` categorization shift。

### 進行中作業

なし (I-161 + I-171 batch PRD は 2026-04-25 T8 で close 済、下記「直近の完了作業」table 参照)。次の作業は本 file 末尾「次の作業」section 参照。

_(T2-T7 完了範囲 + T8 close の詳細は git history (commits ぶら下がり) + `doc/handoff/design-decisions.md` の "I-161 + I-171 batch" section に集約。本 plan.md からは要約のみ「直近の完了作業」table に記載)_

<details><summary>T2-T7 完了範囲 (folded、archive 用、2026-04-25 close 時点)</summary>

**T7 完了範囲 (2026-04-25 revert 後の最終状態)**:

T7 PRD は当初の deliverable「classifier × emission cohesion verification」を達成。T7-3 cell で IR/TypeResolver lane coherence の architectural gap を empirically 発見し、root cause として `FileTypeResolution::narrowed_type` の closure-reassign suppression scope が enclosing fn body 全体に broad すぎる事実を trace 特定。**workaround patch は deep deep `/check_job` で発見した Scenario A regression を回避するため revert 実施 (2026-04-25)**、architectural fix は I-177-D PRD に委譲。

- T7-1〜T7-5 E2E fixture empirical 検証 (5 cell):
  - **T7-1 (`&&=` on narrowed F64、R4 re-host)**: ✓ GREEN — closure-reassign 不在、narrow 維持、`&&=` で 5 → 3 に再代入
  - **T7-2 (`||=` on narrowed F64)**: ✓ GREEN — closure-reassign 不在、truthy x で `||=` no-op
  - **T7-3 (`&&=` × closure-reassign × narrow-suppression)**: ⚠ RED ignored, **I-177-D dependency** — pre-T7 / post-revert emission `if let Some(mut x) = x { if x.is_some_and(...) { x = Some(3.0); } reset(); return x.unwrap_or_else(...); }` は IR shadow x: T と TypeResolver Option<T> view 不整合で E0599/E0282/E0308 chain + closure-mutable-capture E0506。I-177-D (TypeResolver suppression scope refactor、案 C) で IR shadow form と TypeResolver narrow が agree → cohesion 解消の見込み
  - **T7-4 (`||=` then `??=` chain)**: ✓ GREEN — 連鎖適用が正しく compose
  - **T7-5 (`&&=` on narrowed synthetic union, string RHS)**: ✓ GREEN — synthetic-union coercion 正常
- **Cohesion gap empirical 発見と documented finding** (T7 PRD 当初 deliverable):
  - 既存 closure-reassign suppression: path 2 (`=== null` early-return) と path 3 (`=== null` + non-exit else) のみ実装、`!== null` 早期 return × closure-reassign は素通し
  - Symptom: `if (x !== null) { x &&= 3; reset(); return x ?? -1; }` (closure reset = () => x = null) で `if let Some(mut x) = x` shadow が発火、内部 `x &&= 3` は TypeResolver の `Option<f64>` view を query して Option-shape 化、shadow `f64` と miss-match → E0599/E0282/E0308 chain
  - Architectural root cause (T7 で trace 特定): `FileTypeResolution::narrowed_type(var, position)` の `is_var_closure_reassigned` suppression scope が `enclosing_fn_body` (fn body 全 span) で broad すぎ、本来 narrow 保持が valid な if-body (cons-span) 内も含めて narrow を抑制。本来の意図は LET-WRAP shadow が closure の outer Option<T> reassign を破壊するのを防ぐこと、cons-span 内 suppress は overreach
- **Workaround patch attempt + revert 経緯 (T7 review iteration)**:
  - Initial fix: `try_generate_narrowing_match` に NonNullish !== closure-reassign suppression branch (sub-case (a) `if x.is_some() { body }` + sub-case (b) `if x.is_some() { body+exit; } rest;`) を追加
  - Initial /check_job: Truthy guard 誤発火 (silent semantic change risk for `Some(0.0)` etc.) を発見、guard variant pattern-match で structural restrict
  - Deep /check_job: INV-2 path 3 symmetric coverage 欠落を発見、sub-case (b) 追加で symmetric coverage 復元
  - Deep deep /check_job: sub-case (b) test 不完全 + **Scenario A regression** (`return x` body without `??` × closure-reassign で E0308 mismatch) を発見。pre-T7 shadow form は ✓ だったが post-T7 patch で ✗ に regress
  - User judgement (2026-04-25): patch は `ideal-implementation-primacy.md` interim patch 条件 (2)(4) 未充足 + structural fix を patch に降格 + Scenario A regression を knowingly commit する事態を回避 → **revert 実施**、I-177-D architectural fix に委譲
- **Revert 実施内容 (2026-04-25)**:
  - 削除: `try_generate_narrowing_match` の T7 closure-reassign suppression branch (~80 LOC) + `then_exits`/`else_exits` の hoisting
  - 削除: 3 件の T7 lock-in test (`narrowing_match_suppressed_for_neq_null_early_return_when_closure_reassign_present` / `..._with_else_then_exit_...` / `narrowing_match_does_not_use_predicate_form_for_truthy_when_closure_reassign_present`)
  - 維持: T6 P1-P4 全て (T7 と独立) / T7-1/2/4/5 un-ignore / T7-6 unit test 2 件 / `narrowing_match_uses_if_let_for_neq_null_early_return_without_closure_reassign` (pre-T7 boundary lock-in)
  - Annotation 更新: T7-3 ignore annotation を I-177-D dependency 反映に更新 (現実装は pre-T7 shadow form)
- T7-6 unit test 2 case (`error_path.rs`、`narrow_incompatible_rhs_f64_and_string_does_not_intercept` + `..._or_string_...`): narrow × incompatible RHS の dispatch が **TypeResolver lane を侵さず** desugar IR を well-formed に emit して rustc に type 不一致を委譲することを lock-in (Tier 2 compile fail 確保、Tier 1 silent miscompile 防止)。T7 fix と独立で revert 影響なし
- Report: `report/i161-i171-t7-classifier-emission-cohesion.md` を「cohesion gap 検出 + I-177-D 起票で close、fix は I-177-D で実施」体裁に更新 (gap verification 成果は documented finding として保持、revert 経緯 + Scenario A regression trace + body shape × emission form の trade-off matrix を記録)
- **Framework rule 化** (revert に伴う後続作業、TODO 起票済): T7 三度の `/check_job` iteration で発見された 4 件の defect (Truthy 誤発火 / INV-2 / sub-case test / Scenario A regression) は全て「次元 A × 次元 B の直積 enumeration 不足」に帰着し、process / design / architectural の 3 層に root cause を整理:
  - **Architectural** (TODO `[I-177-D]`): `narrowed_type` suppression scope refactor (案 C 推奨)
  - **Design** (TODO `[I-178-5]` Rule 10): Cross-axis matrix completeness (spec stage 側)
  - **Process** (TODO `[I-183]`): `/check_job` 4 層化 framework (mechanical / empirical / structural / adversarial、implementation stage 側)
- Quality gate 全 pass: cargo test **3121** lib (T7 lock-in test 3 件削除で -3) + 122 integration + 3 compile + **155** E2E + 28 ignored、clippy 0 warnings、fmt 0 diffs、file-lines OK、Hono bench revert 前 112/62 → revert 後 111/63 (I-172 ±1 clean / ±2 errors known noise band 内、category diff `OBJECT_LITERAL_NO_TYPE 28→27 / OTHER 15→17` は I-172 の categorization shift と同一 pattern、T7 fix が Hono に作用する pattern (closure-reassign + `!== null` + early-return) は Hono に存在しないため structural regression は構造的に不可能、観測値 variance は I-172 由来)

**T6 完了範囲 (2026-04-25)**:
- **P1 (`generate_truthiness_condition` / `generate_falsy_condition` 全型網羅化)**: helpers.rs の fallback を `truthy_predicate_for_expr` (I-171 T2 expr-level API) 経由に置換。Option<T> / Vec / Named struct / Fn / Tuple / StdCollection / DynTrait / Ref / always-truthy 全て対応。Any / TypeVar / Unit / Never / Result / QSelf は compile-time fence (`Expr::Ident(name)`) で rustc に委譲。callee に `&SyntheticTypeRegistry` 引数追加、loops.rs / control_flow.rs callsite 2 箇所更新
- **P2 (`try_generate_primitive_truthy_condition` peek-through)**: control_flow.rs の outer/inner unwrap を `unwrap_parens` → `peek_through_type_assertions` に置換。Matrix C-11/C-12/C-13 (`if (x as T)` / `if (!(x!))`) で primitive predicate emission が正しく発火
- **P3a (`detect_early_return_narrowing` Bang arm peek-through)**: guards.rs の Bang arm operand 取得を `unary.arg.as_ref()` → `peek_through_type_assertions(unary.arg.as_ref())` に置換。`if (!(x as T)) return;` で narrow event push (`if (!x) return;` と完全 symmetric)
- **P3b (Bang arm OptChain case 新規追加)**: `if (!x?.v) return;` で base `x` を `Option<T>` → `T` narrow。OptChain invariant: `x` が null なら `x?.v` short-circuit → undefined → !undefined = true → exit 発火、fall-through は x non-null を保証。`narrowing_patterns::extract_optchain_base_ident` 経由で base 取得、`unwrap_option_type` で inner 抽出、`PrimaryTrigger::OptChainInvariant` で event push (T6-4 `x?.v !== undefined` の symmetric)
- **P4 (test 拡張)**: helpers.rs に non-primitive 7 case (Option<F64>/Vec/Named struct/Option<Named>/Vec falsy/Any fallback/TypeVar fallback)、`narrowing_analyzer/tests/guards/early_return.rs` に T6 P3a/P3b 5 case (TsAs / TsNonNull peek-through / OptChain narrow / OptChain+Paren cohesion / OptChain non-Option no-op)
- File-line refactor: `narrowing_analyzer/tests/guards.rs` (1041 LOC) を folder-module 化 → `guards/mod.rs` (471 LOC) + `guards/early_return.rs` (569 LOC)。MockCtx + helpers を `pub(super)` で sub-module から共有
- E2E un-ignore: cell-c16b は P3b で post-if `x.v` field access が narrowed `_TypeLit0` 型で resolve されることを empirical 確認。残 3 件の orthogonal blocker (E0507 OptChain field-access closure / I-175 struct-literal expected-type coercion / I-181-like `?? ""` Option<String> closure 戻り値型) で re-ignore、annotation 最新化
- Quality gate 全 pass: cargo test **3118** lib + 122 integration + 3 compile + **151** E2E + 32 ignored、clippy 0 warnings、fmt 0 diffs、file-lines OK、Hono bench 112/158 / 62 errors (T5 baseline 111/63 から +1 clean / -1 errors、I-172 ±1 noise band 内、structural regression 0)

**T5 完了範囲 (2026-04-24)**:
- `convert_if_stmt` の Layer 2 dispatch を再構成 (`src/transformer/statements/control_flow.rs`):
  - `try_generate_option_truthy_complement_match` の outer/inner unwrap を `unwrap_parens` → `peek_through_type_assertions` に置換 (Matrix C-11/C-12/C-13: `!(<x>)` / `!(<x as T>)` / `!(<x!>)` 全て narrow consolidated match に乗る)
  - 引数に `else_body: Option<&[Stmt]>` を追加し、emission shape を `OptionTruthyShape` 列挙で 2-form に統一:
    - `EarlyReturn` (else 不在 + then always-exit): 既存 T6-3 `let x = match x { Some(x) if truthy => x, _ => { exit } }` 形式 (post-if narrow 材料化)
    - `ElseBranch` (else 存在): 新形式 `match x { Some(x) if truthy => { else_body }, _ => { then_body } }`。`Some(x)` で outer var name を shadow し、else_body 内で `x` が narrow `T` を参照 (Matrix C-5)
  - `else_body.is_none()` && `!ir_body_always_exits(then_body)` の場合は None 返却 → Layer 1 fall-through (Matrix C-4 = predicate form `if <falsy(x)> { body }`)
  - `build_option_truthy_match_arms` を `OptionTruthyShape` 受け取り形に refactor、両形式で同じ shape-aware arm builder を共有 (DRY)
  - `build_union_variant_truthy_arms` も shape-aware に。`ElseBranch` 形式では各 variant arm body の先頭に `let <var_name> = Enum::Variant(__ts_union_inner);` を挿入し、user-written else_body を inline (per-variant shadow narrow、synthetic union × else 対応)
- `convert_if_stmt` 末尾に **const-fold dead-code elimination** を追加 (Matrix C-7/C-9/C-24): condition が `Expr::BoolLit(true)` → then_body 直返却、`Expr::BoolLit(false)` → else_body or 空 stmt list。Layer 1 (`try_constant_fold_bang`) が `!null`/`!arrow`/`!always-truthy-ident` 等を `BoolLit` に fold した結果を Layer 2 で if-wrapper ごと除去、PRD 「ideal output」基準に整合 (`if true { ... }` 残骸 / unreachable post-if コード根絶)
- E2E empirical verify: 残 RED Matrix C cell 15 件のうち **10 cell GREEN 化** (cell-c4/c5/c7/c11/c12/c13/c14/c17/c19/c24)。残 5 cell は T5 emission ✓ / E2E ✗ で blocker 別物:
  - **c15** (`if (!u.v)` Member): Layer 1 emission inside `f` is correct、`main` 側で synthetic `_TypeLit0` vs registered `FU` interface の型不一致 (synthetic-type-unification gap)
  - **c16** (`if (!x?.v)` OptChain): Layer 1 emission is correct、`x.as_ref().and_then(|_v| _v.v)` の `_v.v` field move out of `&_TypeLit0` (E0507) — pre-existing OptChain field-access closure lowering defect
  - **c16b** (OptChain base narrow): T6 P3b (`guards.rs` Bang arm OptChain case) 実装で narrow event push 必要、本 T5 scope 外
  - **c18** (`if (!(x && y))` LogicalAnd post-narrow): Layer 1 De Morgan emission ✓、post-if で `x`/`y` を `format!` する narrow 材料化が CFG-level で必要、I-177 (narrow-emission-v2) scope
  - **c23** (`if (!(x || y))` LogicalOr + post-`?? "null"`): Layer 1 De Morgan emission ✓、post-if `Option<f64> ?? &str` の synthetic-union coercion が NC 側に必要 (`x.unwrap_or_else(|| "null")` で closure 戻り値型不一致)
- Unit test: **12 case** 追加 (post /check_job fix で +1):
  - `truthy_complement_match` 5 case: `option_f64_else_branch_lowers_to_match_with_shadow` (C-5 shape) / `early_return_form_peeks_through_ts_as_assertion` (C-12) / `early_return_form_peeks_through_ts_non_null_assertion` (C-13) / `else_branch_form_emits_match_even_with_non_exit_then` (else + non-exit) / `else_branch_form_synthetic_union_inlines_per_variant_shadow_let` (SG-T5-2 ElseBranch × Option<synthetic-union> per-variant shadow let inline、post /check_job 追加) / `non_exit_no_else_falls_through_to_predicate_form` (C-4 fall-through)
  - `if_while::const_fold_dead_code_elim` 6 case: `if_true_no_else_inlines_then` / `if_true_with_else_inlines_then_drops_else` / `if_false_no_else_drops_then` / `if_false_with_else_inlines_else_drops_then` / `bang_null_const_fold_dead_code_elim` (Layer 1 + Layer 2 cooperation) / `bang_arrow_const_fold_dead_code_elim`
  - 既存 `test_convert_stmt_if_no_else` / `test_convert_stmt_if_else` を const-fold と直交させるため input を `let b: boolean = true; if (b) { ... }` に変更 (literal `true` 入力の固有テストは `const_fold_dead_code_elim` mod に移管)
  - 共通 `convert_stmts` helper を `tests/mod.rs` に新設 (function body 全体の IR list を返す、const-fold 後の statement 数変化を観測可能)
- Post /check_job adversarial review fix (2026-04-24): SG-T5-1 (PRD Matrix C-15 ideal text を type-aware falsy-predicate 形に訂正、Option<String> の `Some("")` falsy 取り逃しを解消) / SG-T5-2 (ElseBranch × synthetic union unit test 追加) / IG-T5-1 (`build_option_truthy_match_arms` を lazy-compute refactor、Named 路の不要 positive_body clone 排除) / IG-T5-2 (cell-c16b annotation に OptChain `_v.v` field-access closure E0507 blocker を追記、T6 P3b 単独では unblock 不能であることを明記)
- Deep /check_job adversarial review fix (2026-04-24): SG-T5-DEEP1 (Matrix C-5 sub-case under-spec — PRD textual ideal が "then-always-exits + else-non-exit" の post-if narrow 材料化を漏らしており、bare ElseBranch shape では post-if `x: Option<T>` のままで TS narrow `T` と乖離、`x + 1` 等 post-if 使用が E0369 で compile fail)。新 shape `OptionTruthyShape::EarlyReturnFromExitWithElse { else_body, exit_body }` を追加して `let x = match x { Some(x) if truthy => { else_body; x }, _ => { exit } };` 形を emit、tail expr で narrowed value を outer let に rebind。primitive `Option<T>` + synthetic union `Option<Named>` 両 path 対応 (synthetic union は per-variant arm body に `let <var_name> = Enum::Variant(...);` shadow + `; <var_name>` tail を inline)。empirical 検証: pre-fix `cargo run` で E0369 → post-fix `cargo run` で `-1, -1, "non-exit else", 6` (TS と一致)。unit test +2 (primitive / synthetic union) + E2E fixture cell-c5b (新規 GREEN)
- Deep deep /check_job adversarial review fix (2026-04-24): TypeResolver と IR 間の cohesion 検証で 2 件の重大な gap を発見・修正。
  - **SG-T5-DEEPDEEP1 (visitors.rs narrow event push)**: `if (!x) return; else <non-exit>; return x;` (Option<T> return) で IR shadow makes x: T だが TypeResolver は narrow event 未受領 (visitors.rs:715 が `alt.is_none()` 限定だった) → Some-wrap coercion 不発火 → `return x` (Option<T> return type、IR shadow x: T) で E0308 mismatch。fix: `visitors.rs::visit_if_stmt` の guard を `if_stmt.alt.is_none() && stmt_always_exits(cons)` から `then_exits && !else_exits` に拡張、else 存在ケースでも `detect_early_return_narrowing` を発火。empirical: pre-fix h() E0308 → post-fix h() Some(x) 自動 wrap で `return x` が `return Some(x)` 化、Option<T> return type と整合。
  - **SG-T5-DEEPDEEP2 (try_generate_narrowing_match symmetric extension)**: 上記 visitors.rs 修正により `if (x === null) return; else <non-exit>; return x;` (=== null + Option<T> return) で TypeResolver が narrow event を push するようになったが、対応する emission は if-let 形式で post-if narrow を IR shadow しないため Some-wrap が `Some(Option<T>)` = `Option<Option<T>>` mismatch を生成。fix: `try_generate_narrowing_match` に新 branch を追加 (`complement_is_none && is_swap && then_exits && !else_exits && else_body.is_some()`) で `let var = match var { None => { exit }, Some(v) => { else_body; v } };` 形を emit (T5 EarlyReturnFromExitWithElse の symmetric)。closure-reassign suppression (T6-2) も対応。empirical 検証: cell-c5c で primitive arithmetic + Option<T> return 両方 GREEN。
  - **File line-count refactor**: control_flow.rs が新 logic 追加で 1005 → 584 行 (limit 1000 超過解消)。`OptionTruthyShape` enum + `try_generate_option_truthy_complement_match` + `build_option_truthy_match_arms` + `build_union_variant_truthy_arms` + `is_supported_variant_truthy_type` を `option_truthy_complement.rs` 新規 sub-module に抽出 (`impl Transformer` block を Rust の cross-module impl-fragment で分割)。caller の API は不変。
  - 注: typeof / instanceof + else-non-exit + narrowed-use の case は同様の TypeResolver-vs-IR mismatch 問題が pre-existing で残存 (本 PRD では Bang `!x` + `=== null` の 2 path のみ symmetric 修正、typeof/instanceof は別 PRD scope)。
- /check_problem audit fix (2026-04-24): T5 開発全体を振り返り、scope 内の未対応問題を systematic audit。**P1 (Layer 2 closure-reassign suppression 不在、try_generate_narrowing_match の `=== null` path には ある asymmetry)** を発見・修正:
  - Pre-fix empirical: `function f(x: number | null) { if (!x) return -1; const reset = () => { x = null; }; reset(); return x ?? 99; }` → Layer 2 の let-wrap shadow が outer `Option<f64>` を immutable shadow `f64` で上書き → closure 内の `x = null` (Option<f64> 値代入) が shadow `f64` 型と mismatch → E0308 compile fail
  - Fix: `try_generate_option_truthy_complement_match` の入口に `is_var_closure_reassigned(var_name, if_stmt_position)` 判定を追加。closure-reassign 検出時は None 返却で Layer 1 predicate form (`if !x.is_some_and(...) { return; }`) に fall-through → outer `Option<T>` が保持され closure 再代入が valid に
  - Empirical verify: post-fix で同テストが runtime `99` ✓ (closure が Some(5.0) → None に reset した後、`x ?? 99` が Default 99 を返す)
  - **P2 (新 try_generate_narrowing_match `=== null + then_exit + else_non_exit` branch (Deep-Deep-Deep-Fix-1) の closure-reassign suppression にテスト不在)** を解消: 既に branch 内で `is_var_closure_reassigned` check を実装済みだったが unit test なし。`null_check_then_exit_else_non_exit_with_closure_reassign_falls_through` を追加で lock-in
  - Test file split (P6): 1061 LOC 単一 file `truthy_complement_match.rs` を 4-file folder に分割 (mod / bang_layer_2 / synthetic_union / null_check_symmetric)、共通 helper `convert_named_fn_body` + 3 assertion helper (`extract_let_match_arms` / `extract_match_stmt_arms` / `assert_arm_body_ends_with_tail_ident`) を mod.rs に集約 (DRY、~115 LOC 削減)、各 file ≤ 1000 LOC
  - 確認済 (no further action needed in T5 scope): TODO/FIXME 残骸 0 件、E2E ignored cells (c15/c16/c16b/c18/c23) annotation accurate、TODO に I-177/I-178/I-179/I-180/I-181 全 entry 確認済
  - **T5 scope 外 latent gap の I-177 集約 (2026-04-24 user 判断)**: 振り返りで発見した 3 件 (Item A: typeof/instanceof/OptChain × `then_exit + else_non_exit` × post-narrow primitive use の INV-2 違反 / Item B: `collect_expr_leaf_types` narrowed_type query 順序 inconsistency / Item C: 反対方向 narrow `!== null` + (F, T) symmetric materialization) を **I-177 narrow-emission-v2 PRD に sub-item I-177-A/B/C として集約**。TODO の [I-177] entry に詳細追記、I-171 PRD doc Cross-cutting Invariants section (INV-2) で参照。新 INV-3 (Layer 2 closure-reassign suppression cohesion) を PRD doc に追加し P1 fix 範囲を明示。
- 4th-iteration deep deep /check_job adversarial review fix (2026-04-24): T5 全体を fresh な観点で再 audit。**SG-T5-FRESH1 (Layer 2 always-truthy 全型対応漏れ)** を発見・修正:
  - PRD Matrix C-3 が Bang `!x` × `Option<Named other>` の early-return form を "✓ T6-3 `Some(v) => v, None => exit`" として ideal 主張していたが、`build_option_truthy_match_arms` は非 synthetic-union Named (interface / class / 非 synthetic enum) で None 返却し、Layer 1 fall-through で `if x.is_none() { exit }` の predicate form のみ emit、IR shadow rebinding なし。post-narrow access (`x.label`、`x.method()`) で E0609 fail
  - 同様に `Option<Vec<T>>` / `Option<Fn>` / `Option<Tuple>` / `Option<StdCollection>` / `Option<DynTrait>` / `Option<Ref>` も Layer 2 None 返却で post-narrow access 不可 (e.g., `x.length` for Vec)
  - Fix: `build_option_truthy_match_arms` に always-truthy path を追加。`is_always_truthy_type(inner, synthetic)` 判定 (Named non-synthetic / Vec / Fn / Tuple / StdCollection / DynTrait / Ref) で single `Some(x) => <body>` arm without truthy guard を emit。primitive arm path と同じ body 構築ロジックを `build_some_arm_body` helper に抽出 (DRY)
  - Empirical 検証: `f_named(x: Tag | null) { if (!x) return "no"; return x.label; }` → pre-fix `if x.is_none() { return; } x.label` (E0609) → post-fix `let x = match x { Some(x) => x, _ => return }; x.label` (✓)。`f_vec(x: number[] | null) { if (!x) return -1; return x.length; }` → 同様に narrow materialization 化、runtime 正解 (`-1, 3`)
  - Unit test +3 (`bang_option_named_other_lowers_to_let_match_with_always_truthy_arm` / `bang_option_vec_lowers_to_let_match_with_always_truthy_arm` / `then_exit_else_non_exit_option_named_other_threads_narrow_through_outer_let`) + E2E fixture cell-c5d (新規 GREEN)
- Quality gate 全 pass: cargo test **3106** lib + 122 integration + 3 compile + **151** E2E + 32 ignored、clippy 0 warnings、fmt 0 diffs、file-lines OK (control_flow.rs 584 + option_truthy_complement.rs ≤ 1000 + truthy_complement_match folder 999 total split into 4 files each ≤ 1000)、Hono bench 111/158 / 63 errors (T4 baseline と完全一致、regression 0)

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

**T8 close 経緯 (2026-04-25)**: T6 P1-P4 (broken window fix) + T7 (cohesion verification、workaround patch revert で I-177-D 委譲) + T8 (PRD batch close = TODO entry 削除 + design-decisions.md archive + plan.md restructure + backlog file 削除) で完了。narrow-scope 関連 (Matrix A.4 / A-6 / O-6 / T7-3 / C-15n / C-16b 等) は **I-177 / I-177-D 完了後** に I-177 PRD 内 sub-task として個別 close する設計。

</details>

### 直近の完了作業

実装詳細は git log / `backlog/` (close 後 archive)、設計判断は
[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD | 日付 | サマリ (1-3 行) |
|-----|------|-----------------|
| **I-161 + I-171 batch (`&&=`/`||=` desugar + Bang truthy emission)** | 2026-04-22〜04-25 | narrow-scope `&&=`/`||=` の Tier 2 compile error + `if (!x)` 汎用 Bang truthy emission を structural fix する batch PRD。8 task で T2 (共有 helper `truthy_predicate_for_expr` / `peek_through_type_assertions`) + T3 (I-161 logical assign desugar) + T4 (I-171 Layer 1 Bang dispatch、5 layer + IG-1〜IG-6 fix、60 unit case) + T5 (I-171 Layer 2 if-stmt narrow emission、`OptionTruthyShape` 3-form + const-fold dead-code elim、Matrix C 10 cell GREEN) + T6 (broken window fix P1-P4 + Bang OptChain narrow event push、guards.rs folder split) + T7 (classifier × emission cohesion 検証で T7-3 cell の architectural cohesion gap = `narrowed_type` suppression scope の overbreadth を発見、workaround patch attempt + 三度の `/check_job` iteration で 4 件 defect 発見、Scenario A regression 判明で **revert 実施**、architectural fix を I-177-D に委譲) + T8 (close)。設計判断は `doc/handoff/design-decisions.md` の "I-161 + I-171 batch" section に archive。三度の review iteration の framework lesson (直積 enumeration 不足 + review 深度の iteration 依存) は I-178-5 (Cross-axis matrix completeness rule) + I-183 (`/check_job` 4 層化 framework) として TODO 起票。最終 quality gate: cargo test 3121 lib + 122 integration + 3 compile + 155 E2E + 28 ignored、clippy 0 / fmt 0 / file-lines OK、Hono bench 111/63 (T6 完了時 112/62 から I-172 ±1 noise variance、T7 fix が Hono に作用しない pattern のため structural regression 不可能)。詳細は git log 参照 |
| **File line-count reduction refactor (8 files)** | 2026-04-21 | 1000 LOC 超過 8 file を cohesion-driven split (21 files changed, +1964 / −8767 LOC net)。Phase 1 test files (build_registry 1123→6 / control_flow 1095→7 / generator/tests 1068→8 / switch 1028→7 / generator/expressions/tests 1019→8) + Phase 2 production files (registry/collection 1524→8 sub-dir with placeholder/decl/class/resolvers/type_literals/const_values/callable / ts_type_info/mod 1045→3 files helpers+tests / transformer/expressions/methods 1267→3 sub-dir mod+closures+tests)。visibility `pub(in crate::registry)` で original `pub(super)` scope を厳密保持。`check-file-lines.sh` OK、quality gate 全 pass、Hono bench 非後退。post-review で `map_method_call` 411 LOC 単一 match decomposition を I-174 として起票 (L4)。計画詳細は git log 参照 |
| **I-144 (control-flow narrowing analyzer umbrella)** | 2026-04-19〜04-21 | CFG-based narrowing analyzer PRD (umbrella: I-024 / I-025 / I-142 Cell #14 / C-1 / C-2a-c / C-3 / C-4 / D-1 吸収) を 9 sub-phase (T0-T6-6) で完了。T0-T2 SDCDF Spec stage (matrix-driven + Dual verdict framework) + T3-T5 analyzer 基盤 (`pipeline/narrowing_analyzer/` + `NarrowEvent` enum + `NarrowTypeContext` trait) + T6-1〜T6-5 emission 実装 (EmissionHint dispatch / coerce_default / truthy E10 / OptChain compound narrow / implicit None tail) + T6-6 close で 7 連鎖 review 11 structural fix (IMPL-1〜7 YAGNI dead variant/field 除去 + `transformer/mod.rs` 1117→718 LOC cohesion 分割)。matrix 全 9 ✗ cell GREEN。設計判断は `doc/handoff/design-decisions.md` section「Control-flow narrowing analyzer (I-144)」8-section archive、sub-phase 実装詳細は git log 参照 |
| **I-153 + I-154 batch + 以前の完了** | 2026-04-19 以前 | I-153 / I-154: switch case body nested `break` silent redirect の structural 解消 + internal label `__ts_` prefix 統一 (`report/i153-switch-nested-break-empirical.md`)。以前: I-SDCDF (spec-first framework、beta)、I-050-a (SDCDF Pilot)、Phase A Step 3/4 (I-020 部分/I-023/I-021)、I-145 / I-150 batch、INV-Step4-1、I-142 (`??=`) / I-142-b+c、I-022 (`??`) / I-138 / I-040 / I-392 ほか。git log で参照可能 |

### 次の作業 (I-171 T5 完了後 2026-04-24、spec-first workflow 適用)

**優先順位は `.claude/rules/todo-prioritization.md` (L1 > L2 > L3 > L4) および
`.claude/rules/ideal-implementation-primacy.md` (silent semantic change を最優先) に従う。**

**Tier 0 prerequisite (framework gate)**: I-178 (spec-first-prd Checklist 4-rule 拡張) — I-177 PRD 起票の framework prerequisite。Spec gap re-detection root cause (RC-A: body-exit sub-case lumping / RC-B: cross-cutting invariant unrequired) を framework rule 化することで I-177 PRD (新規 umbrella、3 sub-item) の Spec stage 設計品質を構造的に保証する。

**Tier 0 (L1 silent semantic change)**: I-177 promote (2026-04-24) — narrow emission mutation propagation 欠陥が I-161 T3 完了で silent runtime 誤動作として顕在化、umbrella PRD として 3 sub-item (I-177-A/B/C) 集約済。I-178 完了後に着手。

**実行順序 (prerequisite chain、I-161+I-171 batch close 後 2026-04-25 更新)**:

```
[I-178 framework prerequisite]  ← Tier 0 起票前のゲート (Rule 6/7/8/9 + Rule 10 = 5 rule 拡張)
       │
       ▼
[I-183 review プロセス 4 層化 framework]  ← implementation stage 側 framework
       │
       ▼
[I-177-D Tier 1 architectural (TypeResolver suppression scope refactor、T7-3 cohesion gap 本体)]
       │
       ▼
[I-177 Tier 0 (L1 silent semantic change、mutation propagation 本体 + sub-items A/B/C)]
       │
       ▼
I-162 → Phase A Step 5 → I-015 → I-158+I-159 → Phase A Step 6 → ...
```

**I-178 / I-183 の framework rule prerequisite (2026-04-25 update)**: I-178 (5-rule 拡張) + I-183 (`/check_job` 4 層化) は **I-177-D / I-177 PRD 起票時の framework prerequisite**。I-178 spec stage rule + I-183 implementation stage rule で matrix-driven PRD 品質を構造的に保証する。

**I-177-D vs I-177 mutation 本体 の順序 (2026-04-25 update)**: I-177-D (suppression scope refactor) を I-177 mutation 本体より **先行** で実施。I-177-D は I-177 sub-items A/B/C の architectural root cause も同時解消する可能性があるため、I-177 全体の structural fix を I-177-D で先行確立した後、残 work を I-177 mutation 本体で実施する設計。

**着手順の導出原則** (上記 prerequisite chain 後の通常順序):
1. I-144 Dual verdict framework で `TS ✓ / Rust ✗` として分離された narrow-related compile error は I-144 context が fresh なうちに優先 (I-161 / I-162 / I-171)
2. Phase A roadmap (Step 5 → Step 6 → Step 7) で compile_test skip 直接削減
3. Phase B (RC-11 OBJECT_LITERAL_NO_TYPE 28件 = Hono 全 error の 45%) は Phase A 完了後
4. L4 latent items (runtime 同一 / reachability なし) は notes 欄に退避

| 優先度 | レベル | PRD | 内容 | 根拠 |
|--------|-------|-----|------|------|
| **0a (Tier 0 prerequisite、framework gate)** | L3 | **I-178 (spec-first-prd Checklist 4-rule 拡張)** | Spec-Stage Adversarial Review Checklist に 6/7/8/9 項目目を追加 (framework 改善 umbrella): Matrix/Design integrity (I-161 SG-2) + Body-exit sub-case completeness (RC-A) + Cross-cutting invariant enumeration (RC-B) + Implementation-aware sub-case enumeration (RC-A 拡張) | I-171 T5 6-iteration の Spec gap re-detection root cause を framework rule として正式化。**I-177 が新規 umbrella PRD (3 sub-item) として matrix-driven Spec stage 設計を要するため、I-178 framework 完了が I-177 起票の prerequisite**。framework 適用なしに I-177 を起票すると同 root cause の再発を構造的に防げない |
| **0b (Tier 0)** | **L1** | **I-177 (narrow emission v2 umbrella、L1 promoted 2026-04-24)** | I-144 T6-3 inherited の shadow-mutation-propagation 欠陥を structural fix。silent runtime 誤動作 (Tier 0)。**集約 sub-item 3 件 (2026-04-24)**: I-177-A (typeof/instanceof/OptChain × `then_exit + else_non_exit` × post-narrow) / I-177-B (`collect_expr_leaf_types` query 順序 inconsistency) / I-177-C (`!== null` + (F, T) symmetric / Truthy `if (x)` symmetric) | I-161 T3 実装で latent defect が **runtime 誤動作** として顕在化、`conversion-correctness-priority.md` Tier 1 silent semantic change 該当 → L1 promote (旧 L2)。I-161 narrow cells (A.4 / A-6 / O-6 / T7-*) + I-171 INV-2 違反 cells の prerequisite。**I-178 完了後に I-178 強化済 framework で起票** |
| 1 | L3 | **I-162** | class without explicit constructor → `Self::new()` 自動合成 | I-144 T2 instanceof narrow の Rust 側 E2E lock-in が本 defect で block。`class Dog {}` → `struct Dog {}` 止まりで `Dog::new()` 不在で E0599 |
| 2 | L3 | **Phase A Step 5** (I-026 / I-029 / I-030) | 型 assertion / null as any / any-narrowing enum 変換 | `type-assertion`, `trait-coercion`, `any-type-narrowing` unskip (3 fixture 直接削減) |
| 3 | L3 | **I-015** | Hono types.rs `Input['out']` indexed access 解決失敗 (E0405) | `src/ts_type_info/resolve/indexed_access.rs:271`。Hono types.rs で 1 件だが dir compile blocker |
| 4 | L3 | **I-158 + I-159 batch** | Non-loop labeled stmt + 内部 emission 変数 user namespace hygiene | I-154 変数版 + I-153 labeled block 対応。I-158 が I-153 emission と interaction のため I-158 先行推奨 |
| 5 | L3 | **Phase A Step 6** (I-028 / I-033 / I-034) | intersection 未使用型パラメータ (E0091) + charAt/repeat/toFixed method 変換 | `string-methods`, `intersection-empty-object`, `type-narrowing` unskip |
| 6 | L3 | **I-143 meta-PRD** | `??` 演算子の問題空間完全マトリクス + 8 未解決セル (a〜h) | I-143-a〜h 未着手。I-143-b (`any ?? T`) は I-050 依存、他は独立 |
| 7 | L3 | **I-142 Step 4 C-5 / C-6 + Phase A Step 7 (I-071)** | I-144 非吸収の small cleanup (C-7 は I-050 依存) + `instanceof-builtin` unskip 用 builtin 型 impl 生成 | C-5/C-6 は test quality 改善 (handoff doc)、I-071 は Phase A 最終 step (1 fixture unskip) |
| 8 | L3 | **Phase B (RC-11)** (I-003 / I-004 / I-005 / I-006) | expected type 伝播の不完全性 (OBJECT_LITERAL_NO_TYPE 28件) | Hono 全 error の 45%、Phase A 完了後の最大インパクト category |

**注**: 本テーブルは着手順。各 PRD で `prd-template` skill + `.claude/rules/problem-space-analysis.md`
+ `.claude/rules/spec-first-prd.md` を適用する (I-178 完了後は同 rule の 4-rule 拡張版が適用される)。

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
