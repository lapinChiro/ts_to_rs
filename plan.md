# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-05-01 post I-205 T10 Iteration v17 deep-deep `/check_job` review + `target_roots_at_self` structural extension)

| 指標 | 値 |
|------|-----|
| Hono bench clean | **111/158 (70.3%)** = T7/T8/T9/T10 baseline と同一 (Preservation、`prd-completion.md` broken-fix PRD allowed pattern: Hono が internal class method 内 setter dispatch を主要使用していないため expected) |
| Hono bench errors | **63** (T7/T8/T9/T10 baseline と同一、no new compile errors、本 PRD scope 外への regression 0 件) |
| cargo test (lib) | 3335 pass / 0 fail / 0 ignored (I-205 T10 = 3308 T9 baseline + 27 T10 = 12 this_dispatch + 15 helper unit tests。`body_requires_mut_self_borrow` recursive walker + `target_roots_at_self` self-rooted Index/Deref/nested FieldAccess detection + setter MethodCall 検出 + 12 dispatch tests for cells 60/61/63/64 + INV-2 + Tier 2 reclassify + setter body + getter body field access + T9 logical compound internal lock-in + 5 deep-deep helper extensions) |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 159 pass + 70 `#[ignore]` |
| clippy | 0 warnings |
| fmt | 0 diffs |
| ./scripts/check-file-lines.sh | OK (全 .rs file < 1000 行、2026-04-29 file-line refactor で 4 violations を構造的解消) |

**bench 非決定性**: ±1 clean / ±2 errors の noise variance を [I-172] として記録 (test/bench infra defect、別 PRD)。

### 進行中作業

**PRD 2.7 (I-198 + I-199 + I-200 cohesive batch) 完了 (2026-04-27)** — Implementation stage T1〜T15 全 task + formal `/check_job` 4-layer review + 9 課題本質 fix (F1〜F10) 完了。詳細は下記「直近の完了作業」table 参照。

**Spec stage 完了 (2026-04-28): I-205 v7 final (Class member access dispatch with getter/setter methodology framework)**。
PRD 2.8 (I-201-A) Spec stage 検討中に **既存 class Method Getter/Setter call site emission framework** の **Tier 2 broken window + L2 Design Foundation defect** を発見 (compile error E0507 / E0609、silent semantic divergence)。PRD 2.8 / PRD 2.9 / PRD 7 (I-201-B) 全 prerequisite framework として I-205 を起票、PRD 2.7 self-applied integration pattern で **framework v1.3 → v1.4 → v1.5 → v1.6 連続 revision** を first-class adopter として self-applied verify 完了。3 度の review iteration (initial → deep → deep deep) で **33 findings + 11 RC clusters resolution**、**rule-audit symmetry principle 確立**。

**主要成果**:
- I-205 PRD: 1661 lines、Spec stage v7 final、matrix ~120+ cells、6 invariants (INV-1〜6)、Spec → Impl Dispatch Arm Mapping section
- TS-0〜TS-5 全 Spec Stage Tasks 完了
- 5 dedicated test files: 3 SWC parser tests (10 passed) + i205_invariants_test.rs (6 ignored stubs) + i205_helper_test.rs (4 ignored stubs)
- 34 E2E fixtures + 34 expected files (red 状態 lock-in、Implementation Stage T14 で green 化)
- framework 改修: `spec-stage-adversarial-checklist.md` v1.6 (Rule 1 (1-4) Orthogonality merge legitimacy + Rule 11 (d-6) Architectural concern relevance + Rule 8 (8-c) audit + Rule 11 (d-6) audit) + `prd-completion.md` Tier-transition compliance + `prd-template` skill (Step 3-pre/3-pre-2/4-template/4.5)
- audit script extensions: `audit-prd-rule10-compliance.py` に 9 new verify functions (Rule 1/2/5/6/8/11/13 + orthogonality consistency + Rule 11 (d-6) + Invariants test contracts) + `audit-ast-variant-coverage.py` に `--files` flag

**進行中: I-205 Implementation Stage Tasks T11〜T15 (T1-T10 完了 2026-05-01、Iteration v9 から user 指示で「T を一つ完了するごとに `/check_job` 4-layer review + 徹底見直し + commit」運用に transition)**。

~~T1 (doc update)~~ ✓ → ~~T2 (MethodSignature/TsMethodInfo kind field)~~ ✓ → ~~T3 (collect_class_info propagate + Rule 11 d-1 fix)~~ ✓ → ~~T4 (TsTypeLit kind propagate)~~ ✓ → ~~T5 (Read context dispatch + B7 traversal helper + extends 登録 Spec gap fix Iteration v9)~~ ✓ → ~~T6 (Write context dispatch via dispatch_member_write helper、Iteration v10、Spec gap = Mapping table Read/Write symmetric 化 fix)~~ ✓ → ~~T7 (UpdateExpr setter desugar、Iteration v11、Spec gap = TypeResolver Update.arg 未再帰 fix)~~ ✓ → ~~T8 (compound assign desugar + INV-3 1-evaluate compliance + T7 helpers IIFE back-port + DRY refactor + member_dispatch.rs 6-file split、Iteration v12、Spec gap = TypeResolver compound assign Member.obj 未再帰 fix)~~ ✓ → ~~T9 (logical compound `??= &&= ||=` integration、Iteration v13、Spec gap = matrix LHS type variants test lock-in + Implementation gap = ??= non-Option LHS Tier 2 honest gate)~~ ✓ → ~~T10 (Internal `this.x` dispatch、Iteration v16 + v17、Implementation gap 4 = T6 setter dispatch silent regression `&self`/`&mut self` inference fix via `body_requires_mut_self_borrow` recursive walker + setter MethodCall detection + L1-DD-1 helper test naming convention + L3-DD-1 `target_roots_at_self` Index/Deref/nested FieldAccess structural extension + L4-DD-1 doc completeness + Spec gap = T9 logical compound internal lock-in test 追加 + Review insight 2 = constructor body bug 別 TODO `[I-222]` + transitive mut method calls 別 TODO `[I-223]` 起票 + TODO doc-sync 5 entries `[I-219]`〜`[I-223]`)~~ ✓ → **T11 (static accessor dispatch + matrix expansion)** ← 次着手 → T12 (Getter body .clone() insertion) → T13 (B6/B7 corner cells Tier 2 reclassify + INV-5 verification) → T14 (E2E fixtures green-ify、34 #[ignore] 解除) → T15 (`/check_job` 4-layer review + 13-rule self-applied verify)。

**T7 単独 commit 完了 (2026-04-29、Iteration v11、`convert_update_expr` Transformer method 化 + Member dispatch + Spec gap fix)**: `convert_update_expr` を free function から Transformer method 化 (call site `mod.rs:129` 連動更新)、`convert_update_expr_member_arm` で T6 `classify_member_receiver` shared helper 経由 Static / Instance / Fallback dispatch + `dispatch_instance_member_update` / `dispatch_static_member_update` 新規 helper (5 件: getter_return_is_numeric numeric type check + build_update_setter_block 共通 setter desugar block builder + dispatch_instance_member_update + dispatch_static_member_update + non_numeric_update_message op-specific Tier 2 wording)。`unreachable!()` macro で MethodKind 3-variant + lookup non-empty invariant codify (T6 dispatch helper と symmetric structural enforcement)。**Spec gap fix (Iteration v11、T5 extends/decl.rs fix と同 pattern)**: `pipeline/type_resolver/expressions/mod.rs::resolve_expr` の `ast::Expr::Update(_)` arm が `update.arg` を recursive resolve せず、Member target の receiver expr_type が未登録 → Transformer `classify_member_receiver` で silent Fallback dispatch (= class member setter dispatch を逃す silent semantic loss) を発見、`Unary` arm pattern 踏襲で `self.resolve_expr(&update.arg)` 追加し structural 解消。**Framework 改善検討**: `spec-stage-adversarial-checklist.md` Rule 10 axis enumeration の default check axis として "TypeResolver visit coverage of operand-context expressions" 追加候補。**Cohesive cleanup (T7 scope 内)**: 既存 Ident form `convert_update_expr` の binding 名 `_old` を `__ts_old` に rename (I-154 `__ts_` namespace reservation rule extension to value bindings、user identifier collision 防止)、snapshot 3 件 (do_while/general_for_loop/update_expr) は pure rename diff で auto-update。**Production code**: `convert_update_expr` method + `convert_update_expr_member_arm` + `build_fallback_field_update_block` (B1/B9 Fallback FieldAccess BinOp block) + member_dispatch.rs に T7 helper 5 件追加。**Pre/post matrix**: Fix (Tier 2 broken → Tier 1) cells 42/43/45-a/45-c/45-dd/45-de、Reclassify (Tier 2 broken → Tier 2 honest) cells 44/45-b/45-db/45-dc、Preserve cell 45-da (PRD 2.7 honest error)、**No regression**。**Unit test 15 件**: tests/i_205/update.rs 新規 (cells 42/43/44/45-a/45-b/45-c/45-db/45-dc/45-dd/45-de + B3 setter only ++ + Computed reject + postfix/prefix 両 form lock-in)。**Final quality (post-deep-deep-review fix)**: cargo test --lib 3247 pass (3220 baseline + 15 T7 first-review + 7 second-review op-symmetric coverage + 5 deep-review D3/D4 branch coverage = 3247) / e2e 159 pass + 70 ignored / integration 122 pass / compile_test 3 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK (update.rs = 959 行 < 1000 threshold、deep-review 副次 cleanup で 1046 → 959 line refactor)。**CLI manual probe**: cell 43 `c.value++` → `{ let __ts_old = c.value(); c.set_value(__ts_old + 1.0); __ts_old }` ✓。**Defect Classification (Iteration v11 final、first/second/deep/deep-deep review 累積)**: Spec gap 5 (= first-review TypeResolver Update.arg 未再帰 + second-review L2-2 cell 44 #[ignore] message / L3-2 matrix op-axis asymmetric / L3-3 matrix Block form mismatch / L3-4 Spec→Impl Mapping B2/B3 missing、全件本 T7 内 resolved、framework 失敗 signal = Rule 9/Rule 10 axis + Rule 11 (d-2) audit + Rule 11 (d-6-a) architectural concern relevance auto-audit 追加候補) / Implementation gap 7 (= second-review L1-1 doc comment + L1-2 test assertion 弱、deep-review D1 anyhow!→UnsupportedSyntaxError + D2 const 抽出 + D3 _ => arm test + D4 static defensive arms test、**deep-deep-review DD1 convert_update_expr exhaustive match (38 variants 全 enumerate、Rule 11 (d-1) compliance + Rust compiler structural safety net)**、全件本 T7 内 resolved) / Review insight 1 (= L4-2 INV-3 1-evaluate compliance for non-Ident receiver、T8 (8-a) scope に詳細 defer = architectural concern relevance 観点で T8 に内包、structural 解消 + T7 helpers への back-port を T8 で実施) + Static B7 inherited update arm test のみ T11 (11-c) matrix expansion defer (T6 pattern 整合)。

**T1-T3 batch 完了 (2026-04-28、`/check_job` 4-layer review + Fix 1-4 適用後 final state)**: `MethodKind { Method, Getter, Setter }` enum (foundational `src/ir/method_kind.rs` 配置、registry re-export で 51 site backward compat) + `MethodSignature.kind` / `TsMethodInfo.kind` field 追加 + `collect_class_info` の `_ => {}` 排除 + class.rs:145 Rule 11 (d-1) violation fix + Pass 2 `resolve_method_sig` の kind hardcode latent bug fix + Fix 2 で `convert_method_info_to_sig` の symmetric kind drop (T4 work piece) を前倒し完了 + Fix 3 で `let _ = X` Rust idiom refactor + Fix 4 で framework Rule 9 sub-rule (c) "Field-addition symmetric conversion site audit" 追加 (v1.6 → v1.7 self-applied integration) + getter/setter propagation unit test 12 件 (4 class T3 + 5 method_kind Fix 1 + 3 type_literals Fix 2)。**新 PRD I-213 起票** (codebase-wide IR struct construction DRY refactor、L4、recurring problem evidence: I-383 T8' + I-205 T2 で 2 度連続)、Fix 4 と相補的 (process vs structural)。**Final quality (post-3-iteration `/check_job` review + light review)**: cargo test --lib 3176 pass / e2e 159 pass + 70 ignored / 122 integration pass / 3 compile_test pass / clippy 0 warning / fmt 0 diff / audit PASS / Pipeline integrity (`src/ir/` SWC indep) 維持。

**T6 単独 commit 完了 (2026-04-28、Iteration v10 first + second review = `dispatch_member_write` helper + Spec → Impl Mapping symmetric 化 + DRY refactor + dead code 排除 + C1 coverage 補完)**: T5 で structural enforcement された `for_write=true` skip path を維持しつつ、`convert_assign_expr` の plain `Assign` × Member target × non-Computed gate で `dispatch_member_write(member, value)` helper 経由に切替 (= setter dispatch / Tier 2 honest error / B1/B9 fallback FieldAccess の統合 dispatch 経路)。**第二次 fix (second-review deep deep `/check_job`)**: (Fix A = DRY violation 解消) `MemberReceiverClassification` enum + `classify_member_receiver` shared helper を抽出、Read/Write 両 helper の receiver type detection 知識を 1 箇所に集約 (subsequent T7-T9 compound dispatch も leverage 可能、増殖性 risk を structural に排除)。(Fix B = asymmetric structural enforcement 解消) T5 `dispatch_instance_member_read` の dead code (`Ok(Expr::FieldAccess)`) を `unreachable!()` macro に置換、4 helper (Read instance / Read static / Write instance / Write static) 全てが symmetric structural enforcement 統一。(Fix C/D = C1 coverage 補完) Static field lookup miss test 1 + Read 3 + Write 3 = 7 defensive dispatch arm test 追加。(T11 (11-f) defer) pre-existing latent gap = Receiver Ident unwrap (Paren / TsAs / TsNonNull wrap で static dispatch を逃す) を T11 task description に Implementation 候補 + 判断基準 詳細記載。**Production code**: `MemberReceiverClassification` enum (Static/Instance/Fallback 3 variants) + `classify_member_receiver` shared helper + `dispatch_member_write` + `dispatch_instance_member_write` (4 arm + `unreachable!()`) + `dispatch_static_member_write` (4 arm + `unreachable!()`)。**Spec gap fix (first-review source)**: `## Spec → Impl Dispatch Arm Mapping` の `dispatch_member_write` table を Read mapping と完全 symmetric な structural form (Instance / Static section 分離 + 5 arm enumerate) に拡張、Rule 9 (a) compliance restored。**Unit test 17 件**: cells 11/12/13/14/16/17/18/19 dispatch arm 8 件 (B1/B2/B3/B4/B6/B7/B8/B9 全 cover) + INV-2 E1 Read/Write symmetry 1 件 + T6 Fallback equivalence 1 件 + second-review C1 補完 7 件 (Static field lookup miss 1 + Read 3 defensive + Write 3 defensive)。**Pre/post matrix**: Fix (Tier 2 → Tier 1) cells 13/14/18、Reclassify (silent → Tier 2 honest) cells 12/16/17、Preserve cells 11/19、**No regression**。**Final quality**: cargo test --lib 3207 pass (3190 baseline + 17 T6) / e2e 159 pass + 70 ignored / compile 3 pass / clippy 0 warning / fmt 0 diff / Hono Tier-transition compliance = **Preservation** (clean 110 / errors 64 unchanged、Hono が external setter dispatch on class instances を主要使用していないため allowed per `prd-completion.md`)。**CLI manual probe**: B4 `b.x=5` → `b.set_x(5.0)` ✓、B8 `Counter.count=7` → `Counter::set_count(7.0)` ✓。**Defect Classification (final)**: Spec gap 1 (first-review fix 済 = Mapping asymmetric) / Implementation gap 4 (second-review 全 fix 済 = DRY violation / dead code asymmetric / Static lookup miss test / Defensive arms test) / Review insight 2 (first-review #1 = Framework v1.8 候補 / second-review #2 = Receiver Ident unwrap = T11 (11-f) defer)。

**T5 単独 commit 完了 (2026-04-28、Iteration v9 = T5 着手前 Spec への逆戻り 2 件 fix + 3 回 `/check_job` reviews による critical bug 2 件追加 fix + Read context dispatch + B7 traversal helper)**: T5 着手前調査で **2 件の infrastructure Spec gap** を発覚 → Iteration v9 = `spec-first-prd.md` 「Spec への逆戻り」発動 (= 1 PRD = 1 architectural concern の completeness 達成、別 PRD I-013/I-014/I-206 と orthogonal な registration phase の修正)。**(1) `class.rs:195` extends hardcode** (`extends: vec![]`) を `class.class.super_class` 経由 `Vec<String>` に propagate (interface `decl.rs:63-73` と symmetric registration pattern) + **(2) `decl.rs:264` empty body class register filter** に `extends.is_empty()` を condition 追加 (`class Sub extends Base {}` を Pass 2 結果で register、Pass 1 placeholder ↔ Pass 2 collect の data preservation invariant の structural fix)。**Production code 追加**: registry `pub fn lookup_method_sigs_in_inheritance_chain(&self, type_name, field) -> Option<(Vec<MethodSignature>, bool /* is_inherited */)>` (cycle-safe HashSet visited、parent traversal) + `resolve_member_access` Read context dispatch 拡張 (B1 fallback / B2 Getter MethodCall / B3 Setter Tier 2 honest "read of write-only property" / B4 Getter+Setter MethodCall / B6 Method Tier 2 honest "method-as-fn-reference (no-paren)" / B7 inherited Tier 2 honest "inherited accessor access" / B8 Static FnCall::UserAssocFn / B9 unknown fallback)。**3 回 `/check_job` 4-layer reviews による追加 fix (deep deep review fix)**: (Critical 1 = Implementation gap) `convert_member_expr_inner` の `for_write=true` で本 T5 Read dispatch を skip (Write context LHS leak silent regression を structural fix、T6 で setter dispatch 別途実装する正しい partition) + (Critical 2 = Spec gap) `Spec → Impl Mapping` table の Static dispatch arms 訂正 + `dispatch_static_member_read` の dead code を `unreachable!()` macro で structural enforcement (`sigs` non-empty + `MethodKind` 3 variant exhaustive invariant codified)。**Unit test 14 件**: cells 1/2/3/4/5/7/8/9/10 dispatch arm IR token-level lock-in 9 件 + B7 traversal helper 4 件 (cycle-safe / direct hit / single-step / multi-step inherited、boundary value analysis 完備) + Write context regression 1 件 (Read dispatch leak の structural lock-in)。**Pre/post matrix**: Fix (Tier 2 → Tier 1) cells 2/3/5/9、Reclassify (silent → Tier 2 honest) cells 4/7/8、Preserve cells 1/6/10、**No regression**。**Final quality (post 3 reviews)**: cargo test --lib 3190 pass / e2e 159 pass + 70 ignored / 3 compile_test pass / clippy 0 warning / fmt 0 diff / Hono Tier-transition compliance = **improvement** (post-deep-deep-fix bench: clean 110 / errors 64、+1 OTHER = `router/smart-router/router.ts:46:20` `method-as-fn-reference (no-paren)` は本 T5 dispatch arm B6 の silent FieldAccess emit → Tier 2 honest reclassify、ideal-implementation-primacy 観点で silent semantic loss 排除 = improvement、本 PRD scope 外 file への new compile error 0 件)。**Defect Classification (3 reviews 累積)**: Spec gap 4 (extends 登録 / decl.rs:264 / static dispatch wording missing / Mapping table 誤記、全て本 T5 内 resolved、framework 失敗 signal) / Implementation gap 1 (Write context LHS leak、本 T5 内 resolved) / Review insight 4 = **6 件本 T5 内 resolved + 3 件 T11/T13 task description に詳細 defer (Mixed class is_static = T11 (11-b) / Static field strategy = T11 (11-d)(11-e) / INV-5 verification = T13 (13-b)(13-c) / Multi-step N>=3 step boundary = T13 (13-d))**。**Framework 改善検討 6 candidates** (Iteration v9 entry 内 詳細記載) は本 PRD close 時 integrate or 別 framework PRD 起票候補。

PRD 2.8 (I-201-A) は I-205 完了後に再開、I-205 framework foundation を leverage。

**Plan η update (2026-04-26 post I-177-B empirical investigation)**: I-177-B 実装中の empirical verification で **TypeResolver-Synthetic registry integration の pre-existing latent bug** を発見 (`SyntheticTypeRegistry::fork_dedup_state` が `union_dedup` を継承しつつ `types: BTreeMap::new()` で fork、builtin pre-registered union 型を使う narrow guard で `compute_complement_type` が None を返し、post-narrow scope の EarlyReturnComplement event が push されない silent failure)。Plan η chain に **Step 1.5 = I-177-E (synthetic fork inheritance fix)** を I-177-B prerequisite として挿入し close。

**Plan η (2026-04-26 user 確定 + 2026-04-26 Step 1.5 + 2.5 挿入): I-177 umbrella + I-048 を 1 PRD = 1 architectural concern で順次 close する 8 PRDs serial 構成**:

```
Phase 0: empirical audit (silent change quantification、完了 2026-04-26、report/I-177-step0-audit/)
   ↓
PRD 1 (I-177-D): TypeResolver narrowed_type suppression scope refactor (案 C、**完了 2026-04-26**)
   ↓
PRD 1.5 (I-177-E): TypeResolver synthetic fork inheritance gap fix (~5 LOC core + ~50 test、**完了 2026-04-26**)
   ↓
PRD 2 (I-177-B): collect_expr_leaf_types query 順序 fix + leaf type resolution cohesion (canonical helper extract、~75 LOC、non-matrix-driven、**完了 2026-04-26**)
   ↓
PRD 2.5 (I-177-F): resolve_arrow_expr / resolve_fn_expr / class constructor / class method body の visit_block_stmt 経由統一 (~4 LOC production + 4 unit test + 1 E2E、**完了 2026-04-26**、I-177-B callable arrow form `#[ignore]` 解除)
   ↓
PRD 2.7 (I-198 + I-199 + I-200 cohesive batch): framework Rule 改修 (Rule 3/4/10/11/12) + TypeResolver coverage extension (StaticBlock + Prop::Method/Getter/Setter body resolve + AutoAccessor Tier 2 error reported) + ast-variants.md Prop/PropOrSpread/Decorator section 新規追加 + audit scripts CI 化、**完了 2026-04-27** (Implementation Revision 1 PropOrSpread Grammar gap fix + Revision 2 cell 15 Prop::Assign critical Spec gap fix、3162 lib pass + 18 new unit tests + 19 audit self-tests、Hono 0 regression)
   ↓
PRD 2.75 (I-205): Class member access dispatch with getter/setter methodology framework (PRD 2.8/2.9/I-201-B prerequisite、L2 Design Foundation、Tier 2 broken framework → Tier 1 完全変換、Spec stage v7 final 完了 2026-04-28、framework v1.3 → v1.6 self-applied integration、Implementation Stage T1-T15 ← **次着手**)
   ↓
PRD 2.8 (I-201-A): AutoAccessor 単体 Tier 1 化 (decorator なし subset、`accessor x: T = init` → `struct field + getter/setter pair`、L3、user 承認 2026-04-27、I-205 framework leverage)
   ↓
PRD 2.9 (I-202): Object literal `Prop::Method` / `Prop::Getter` / `Prop::Setter` Tier 1 化 (Transformer 完全 emission、L3、user 承認 2026-04-27、I-205 framework leverage)
   ↓
PRD 3 (I-177 mutation propagation 本体): F1/F3 body mutation propagation (Tier 0 silent semantic change、L1、案 A vs 案 B 確定)
   ↓
PRD 4 (I-177-A): else_block_pattern Let-wrap 化 + I-194 typeof if-block elision (拡張可)
   ↓
PRD 5 (I-177-C): symmetric XOR early-return detection
   ↓
PRD 6 (I-048): closure ownership 推論 (T7-3 完全 GREEN-ify)
   ↓
PRD 7 (I-201-B): Decorator framework 完全変換 (TC39 Stage 3、AutoAccessor + class + method + property + parameter 全 application 共通、L1 silent semantic change、user 承認 2026-04-27、PRD 3 後の next-priority L1 = reachability 軸で PRD 3 先行 + I-201-A foundation を leverage)
```

**Plan η Step 1.5 (I-177-E) 起票根拠**: I-177-B 実装中の empirical verification (CLI 経由の `function h(...)` typeof + post-if return scenario) で hard error が解消されない事象を逐次 dbg trace し、`compute_complement_type` の `synthetic_enum_variants` query が builtin pre-registered union signature に対し None を返す pattern を確定。`fork_dedup_state` の `types: BTreeMap::new()` を `types: self.types.clone()` に修正することで構造的に解消。本 PRD は I-177-B PRD 起票時 problem space に未認識だった prerequisite で、Plan η framework の 1 PRD = 1 architectural concern 原則に従い独立 PRD として起票。

### 直近の完了作業

実装詳細は git log、設計判断は [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。

| PRD | 日付 | 残課題 / 後続への影響 |
|-----|------|---------------------|
| **I-205 T10 (Internal `this.x` dispatch、E2 context、Iteration v16 + Iteration v17 deep-deep `/check_job` review + `&mut self` regression structural fix + `target_roots_at_self` Index/Deref structural extension + TODO doc-sync 5 entries)** | 2026-05-01 | **Architectural concern**: Internal `this.x` dispatch (E2 context、class method body 内) を external dispatch (E1) と structural symmetric に統一、INV-2 (External (E1) と internal (E2 this) dispatch path symmetry) を **構造的に達成** (= 重複 logic 不要、既存 T5/T6/T7/T8/T9 dispatch helpers が `Expr::This` receiver で uniformly fire)。**Empirical foundation**: TypeResolver の `visit_class_body` (visitors.rs:439) が `this` を `RustType::Named { class }` で scope_stack register、`classify_member_receiver` (mod.rs:147) Instance gate で fire、`Expr::This` → `Expr::Ident("self")` IR conversion 既存 (expressions/mod.rs:196) で uniform。**Pre-T10 silent regression discovered + structural fix (Iteration v16 critical finding)**: T6 setter dispatch 導入により IR shape が `Expr::Assign { FieldAccess(self, x), v }` → `Expr::MethodCall { self, "set_x", [v] }` に変化、`body_has_self_assignment` (helpers.rs:100、top-level `Stmt::Expr(Expr::Assign)` のみ検出) が setter MethodCall 見落とし、internal `this.x = v` (cell 61) / `this.x += v` (cell 63) / `this.x++` (cell 64) で silent `&self` emit → Rust E0596 compile error "cannot borrow `*self` as mutable" を引き起こす silent regression を発見。本 T10 で **structural fix**: `body_has_self_assignment` → `body_requires_mut_self_borrow` rename + 拡張 (recursive `IrVisitor` walker `MutSelfRequirementVisitor` + 2 detection cases: case (1) `Expr::Assign { target: self.field, .. }` (pre-T10 case) + case (2) `Expr::MethodCall { object: self, method: starts_with("set_"), .. }` (T6/T7/T8/T10 setter dispatch family))。Recursive descent により pre-T10 helper の depth limitation も同時に解消 (= `if cond { this.x = 5 }` 等 non-trivial top-level structures でも正しく `&mut self` emit)。`is_self_setter_call` shared helper extraction (= prefix `"set_"` + receiver `Expr::Ident("self")` の symmetric structural enforcement)。**Production code**: `src/transformer/classes/helpers.rs` (~80 LOC: `body_requires_mut_self_borrow` + `MutSelfRequirementVisitor` + `is_self_setter_call`) + `src/transformer/classes/members.rs` (call site 1 行更新)。**`/check_job` 4-layer review (Iteration v16)**:
- **Layer 1 (Mechanical)**: 0 findings (全 file < 1000 lines / clippy 0 / fmt 0 / test name pattern 準拠)
- **Layer 2 (Empirical)**: 0 findings (CLI probe で cells 60/61/63/64 の generated Rust が compile + 期待 stdout を produce すること empirical 確認 = stand-alone Rust で `Counter.incrInternal()` 1+1=2 等 verify、Hono Preservation)
- **Layer 3 (Structural cross-axis)**: 1 Spec gap = T9 logical compound `this.x ??= v` (cell 38 internal counterpart) の dispatch test 不在、orthogonality merge inheritance のみに依存 → 本 T10 内 lock-in test 追加で structural verify (`test_internal_this_b4_nullish_assign_emits_block_form_with_predicate`)
- **Layer 4 (Adversarial trade-off)**: 0 findings (pre-T10 baseline では internal `this.x = v` 系が silent compile error、本 T10 で structural fix → pre/post matrix: cells 61/63/64 が Tier 2 broken → Tier 1 fix。Trade-off: false positive `set_*` prefix regular method は &mut self emit (sound = strictly more permissive)、false negative は Rust E0596 で surface する safe fail-safe)

**Defect Classification (5 category)**: Spec gap 1 (T9 logical compound internal test missing、本 T10 内 fix) / Implementation gap 1 (T6 setter dispatch 導入時 `body_has_self_assignment` を symmetric audit 不足、setter MethodCall 検出 logic を helper に追加せず → 本 T10 で structural fix。**Framework 失敗 signal**: I-205 v1.7 Rule 9 sub-rule (c) Field-addition symmetric audit を T6 review 時に **逆 direction (= IR shape 変化 → caller helper update audit)** に拡張する candidate、本 T6→T10 chain は Rule 9 (c) "IR shape evolution" axis を framework 追加する empirical evidence) / Review insight 1 (Constructor body conversion `try_extract_this_assignment` が `this.<accessor> = v` の B3/B4 dispatch を bypass し `Self { value: 7.0 }` 等 invalid struct field を emit する pre-existing bug を発見、別 TODO `[I-222]` 起票)。

**Pre/post matrix**: cells 61 (`this.x = v` internal B4) / 63 (`this.x += v` internal B4) / 64 (`this.x++` internal B4): Tier 2 broken (silent E0596) → Tier 1 (correct `&mut self` + setter dispatch) / cell 60 (Read internal B2): preserved / setter body internal `this.x = v`: Tier 2 broken → Tier 1 / Tier 2 honest error reclassify cells (B2/B3/B6 internal): preserved / Hono Preservation (clean 111 / errors 63 unchanged)。**Iteration v17 deep-deep review (本 T10 内 追加 fix)**: L1-DD-1 (helper test naming convention、`test_*` prefix で全 rename) + L2-DD-1 (cells 63/64 stand-alone Rust empirical compile + run verify、output 1/2/3 ✓) + **L3-DD-1 (`is_self_field_access` → `target_roots_at_self` recursive helper extension、`self.arr[i] = v` Index target + `self.x.y = v` nested FieldAccess + `(*self).x = v` Deref を structural detect、5 件 NEW unit test 追加)** + L4-DD-1 (`is_self_setter_call` prefix-based heuristic の sound 性 + transitive mut `[I-223]` boundary を doc comment 明記)。**TODO doc-sync (T9 commit `cf0d7ce` の `pre-commit-doc-sync.md` violation の本 T10 内 fix)**: I-219 (TypeResolver `resolve_member_type` Spec gap、L3) / I-220 (Setter accept type asymmetry、L4) / I-221 (Top-level Module-level statement expression-context dispatch、L4) を TODO 追加 + I-222 (Constructor body bug、L4) 新規起票 + **I-223 (Method receiver inference does not detect transitive mut propagation、L4、Iteration v17 L3-DD-2 由来)** 新規起票。**Final quality**: cargo test --lib **3335 pass** (3308 baseline + 12 this_dispatch + 15 helper = 3335) / e2e_test 159 pass + 70 ignored / compile_test 3 pass / integration 122 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK (helpers.rs 603 行 / this_dispatch.rs 631 行、両者 < 1000 threshold)。 |
| **I-205 T9 (Logical compound `??= &&= \|\|=` Member target setter dispatch + Iteration v14 deep-deep review structural completeness + Iteration v15 `/check_problem` cleanup)** | 2026-04-30 | **Iteration v15 `/check_problem` cleanup (2026-04-30 post-deep-deep review)**: 4 件 cleanup を本 T9 commit 内 fix — (A-1) `_span: Span` 未使用 parameter 削除 / (A-2) `let _ = ts_obj;` workaround 削除 + predicate-unavailable err span を `ts_obj.span()` で sibling errors と consistency 確保 / (A-3) `span: Span` parameter chain 全削除 (entry method `try_dispatch_member_logical_compound` から `dispatch_*_member_logical_compound` → `dispatch_b4_strategy` → `emit_*` の chain 全体、`use swc_common::Span` 不要に) / (A-4) PRD doc matrix cells 39/40 D-axis orthogonality に sub-cells 7 件追加 (38-identity / 38-blocked / 39-other / 39-truthy / 39-blocked / 40-other / 40-truthy / 40-blocked) で Iteration v14 実装の structural form を完全 enumerate (Rule 10 (Cross-axis matrix completeness) compliance restored)。**`ReceiverCalls` struct → enum refactor (Iteration v15、user 承認)**: 「意味のないフィールド」code smell (`Static.object` placeholder + `Instance.class_name` 空文字列) を type-safe enum (`Instance { object, field } / Static { class_name, field }`) に置換、4 emit_* helpers + dispatch_b4_strategy + resolve_receiver_for_calls + wrap_with_iife_if_needed の dispatch を match-based 化、SE-having branch (= 必ず Instance variant) で `unreachable!()` macro により invariant codify (Static は statically SE-free per resolve_receiver_for_calls)。design-integrity.md 凝集度 / 責務分離 改善、future T11 (11-c) static matrix expansion 等で類似 helper 再導入時の robustness 確保。**新規 TODO 起票 (4 件)**: I-219 (TypeResolver `resolve_member_type` Spec gap = T8 F-SX-1 予測済) / I-220 (Setter accept type asymmetry = T6/T7/T8/T9 共通 pre-existing) / I-221 (Top-level Module statement expression-context dispatch = T14 / E2E suboptimal)。**Final quality**: cargo test 全 pass (lib 3308 = 3274 baseline + 34 T9、e2e 159 + 70 ignored、compile 3、integration 122) / clippy 0 warning / fmt 0 diff / check-file-lines OK / Hono Preservation (clean 111 / errors 63 unchanged)。 |
| **I-205 T8 (Compound assign Member target setter dispatch + INV-3 1-evaluate compliance + T7 IIFE back-port + DRY refactor + member_dispatch.rs 6-file split、Iteration v12)** | 2026-04-29 | **Architectural concern**: arithmetic / bitwise compound assign (`+= -= *= /= %= \|= &= ^= <<= >>= >>>=`、11 ops) Member target で B4 instance / B8 static setter desugar yield_new (`{ let __ts_new = obj.x() OP rhs; obj.set_x(__ts_new); __ts_new }`)、B2/B3/B6/B7 Tier 2 honest error reclassify (`compound assign to read-only / read of write-only / to method / to inherited accessor` wording)、INV-3 1-evaluate compliance for side-effect-having receiver (IIFE form `{ let mut __ts_recv = receiver; ... }`)、T7 dispatch_instance_member_update への INV-3 back-port (cohesive batch、`build_instance_setter_desugar_with_iife_wrap` shared helper)。**Spec gap fix (Iteration v12)**: TypeResolver `resolve_assign_expr` の compound `Member` arm が `is_propagating_op` のみで `resolve_expr(&member.obj)` 経路を通っていた → arithmetic / bitwise compound でも receiver expr_type を unconditional register に修正 (Iteration v11 T7 Update.arg 未再帰と同 pattern、framework 失敗 signal)。**DRY refactor (Iteration v12 third-review)**: T7 update + T8 compound × Instance/Static の B4 setter desugar arm が完全 identical だった IIFE wrap + setter call construction logic 60 行を `member_dispatch/shared.rs::build_instance_setter_desugar_with_iife_wrap` + `build_static_setter_desugar_block` shared helpers に集約 (subsequent T9 logical compound も leverage 可能)。**File split (Iteration v12 third-review)**: `member_dispatch.rs` (1179 行、CLAUDE.md threshold violation) を `member_dispatch/{mod, shared, read, write, update, compound}.rs` (6 file 計 1331 行、各 file 100-369 行) に architectural concern 別 split。**Production code**: `convert_assign_expr` T8 dispatch gate (T6 plain `=` 直後、`arithmetic_compound_op_to_binop` 1-to-1 mapping helper 経由) + `Transformer::dispatch_member_compound` entry method + `dispatch_instance_member_compound` / `dispatch_static_member_compound` 新規 helper + `is_side_effect_free` / `wrap_with_recv_binding` / `build_setter_desugar_block` (旧 `build_update_setter_block` を generalize) shared infrastructure + `TS_RECV_BINDING = "__ts_recv"` constant (I-154 namespace extension)。**`/check_job` 4-layer review (first + second iteration 累積)** で発見された 10 件 finding 全 fix + 別 scope 2 件 TODO 起票:
- **First review** (commit 前 initial): F1 (Implementation gap = `is_side_effect_free` 二重呼び出し) / F2 (Implementation gap = `_` arm Rule 11 d-1 違反 → AndAssign/OrAssign unreachable + ExpAssign UnsupportedSyntaxError + NullishAssign unreachable で exhaustive 化) / F3 (Spec gap = ExpAssign × Member anyhow! → UnsupportedSyntaxError 経由 transparent error reporting) / F5 (Review insight = Cell 21 corollary semantic safety PRD section 追加) を本 T8 内 全 fix。
- **Second review** (post-fix state、追加発見): F-SL-1 (Review insight = compound desugar match comment misleading clarify) / F-SL-2 (Implementation gap = TS_OLD_BINDING doc comment stale reference `build_update_setter_block` → `build_setter_desugar_block`) / F-SX-1 (Spec gap = TypeResolver compound Member arm field type completeness partial = receiver 軸 only resolve、comment clarify + 別 TODO `[I-218]` 起票) / F-EM-1 (Review insight = 11 ops orthogonality merge unit test 不足 → 7 op exhaustive mapping unit test 追加で structural verify 完成) / F-AT-1 (Review insight = Fallback path INV-3 1-evaluate compliance gap pre-existing、本 T8 setter dispatch path scope と orthogonal、別 TODO `[I-217]` 起票で Resolution direction = `is_side_effect_free` / `wrap_with_recv_binding` shared helper Fallback path 適用 詳細 record)。**Unit test 27 件 (= first 20 + second-review F-EM-1 で 7 op orthogonality mapping verify 追加)**: cells 20/22/23/25/26/27/28/29-d/29-e-d/33/34-c/35-d (compound) + Static defensive arms 4 件 + INV-3 FieldAccess recursive judgment + cell 21 SE-free + cell 21 IIFE (T8 cells、計 19 件) + T7 INV-3 back-port verify (`getInstance().value++` で IIFE form emit、1 件) + 7 op orthogonality merge mapping verify (MulAssign/DivAssign/ModAssign/BitAndAssign/BitXorAssign/RShiftAssign/ZeroFillRShiftAssign で B4 instance dispatch + BinOp 置換 verify、second-review F-EM-1 fix)。**Pre/post matrix**: Fix (silent Tier 2 → Tier 1 / Tier 2 honest reclassify) cells 21/21-IIFE/22/23/25/26/27/29-d/29-e-d/33/34-c/35-d、Preserve cells 20/28、T7 cell 43 IIFE back-port fix、**No regression**。**Final quality (post second-review fix)**: cargo test --lib 3274 pass (3247 baseline + 27 = 19 T8 + 1 T7 IIFE back-port + 7 second-review op orthogonality) / e2e 159 pass + 70 ignored / compile_test 3 pass / integration 122 pass / clippy 0 warning / fmt 0 diff / check-file-lines OK / Hono Tier-transition compliance = **Preservation** (clean 111 / errors 63 = T7 baseline 同一、no new compile errors、`prd-completion.md` broken-fix PRD allowed pattern)。**Defect Classification (Iteration v12 first + second review 累積 final)**: Spec gap 3 (= first review 2 + second F-SX-1) / Implementation gap 3 (= first 2 + second F-SL-2) / Review insight 4 (= first F5 + second F-SL-1/F-EM-1/F-AT-1)、全件本 T8 内 resolved or 別 TODO 起票 + PRD doc 詳細 record (= F-SX-1 → I-218、F-AT-1 → I-217)。**framework 改善 candidate**: Rule 10 default axis "TypeResolver visit coverage of operand-context expressions" の正式昇格 + audit script auto verify (Iteration v11/v12 連続 2 度発生 source、3 度目 prevention)。 |
| **環境整備: 行数超過 4 file 構造的分割 + DRY refactor (non-PRD environmental cleanup)** | 2026-04-29 | **Architectural concern**: CLAUDE.md "0 errors / 0 warnings" の `./scripts/check-file-lines.sh` 1000 行 threshold violation (4 file: `type_resolution.rs` 1201 / `return_wrap.rs` 1176 / `type_resolver/expressions.rs` 1024 / `synthetic_registry/tests.rs` 1022) を **凝集度の高い責務分離** + **構造的 DRY 解消** で根本対応。**File 構造変化 (4 → 27 file)**: (1) `type_resolution/mod.rs` (data + impl + DRY helper `position_in_range`) + `tests/{basic_queries, narrowing_suppression, canonical_primitives}.rs` 3 split、(2) `return_wrap/{mod, context, wrapping, collection}.rs` (architectural concern 別 4 split + tests inline、context construction / leaf wrapping / SWC AST collection)、(3) `type_resolver/expressions/` を AST expr type 別 8 split (`mod` dispatcher + `binary` + `assignments` + `member` + `object` + `conditional` + `assertions` + `opt_chain` + `new_expr`) + `assertions.rs` 内 `resolve_type_assertion_inner` shared helper で TsAs/TsTypeAssertion DRY violation 解消、(4) `synthetic_registry/tests/` 6 file split (`mod` + `helpers` + `dedup` + `naming` + `scope` + `ops` + `integration`)。**構造的 DRY 解消 helpers**: `position_in_range(position, lo, hi) -> bool` (4 ヶ所の半開区間 check 集約)、`pub_field(name, ty) -> StructField` (20+ inline literal 集約)、`resolve_type_assertion_inner(type_ann, inner) -> ResolvedType` (TsAs/TsTypeAssertion 4-step logic 集約)。**`/check_job` 4-layer review + `/check_problem` で post-refactor cohesion violation 4 件 (Cond ↔ TsAs/TsTypeAssertion file 同居 + TsAs/TsTypeAssertion DRY + is_none_expr/coerce_string_literal 直接 test 不在 + position_in_range 直接 test 不在) 全 fix。**新 TODO 起票**: I-393 (`expected_types.insert + propagate_expected` pattern 17+ call site DRY 統合、別 PRD)、I-394 (`collect_*_leaf_types` ↔ `wrap_body_returns` SWC walker positional invariant DRY、別 PRD)。**Quality**: cargo test (lib) 3207 → 3220 (+13 branch coverage / boundary value analysis tests、regression 0)、clippy 0 warning、fmt 0 diff、check-file-lines OK、Hono bench 0 regression (semantic 変更なし、純 structural refactor)。 |
| **PRD 2.7 (I-198 + I-199 + I-200 cohesive batch、framework Rule 10/11/12 + Rule 4 拡張 + TypeResolver coverage extension + ast-variants.md Prop/PropOrSpread/Decorator section 追加)** | 2026-04-27 | **Architectural concern**: framework Rule 改修 (Rule 10/11/12 + Rule 4 doc-first dependency order) + 拡張による coverage gap detection 完成 + structural enforcement (1 PRD = 1 architectural concern)。**T1〜T15 全 task 完了** + `/check_job` 4-layer review (formal initial invocation) で発見 9 課題を本質的に解決 (test coverage gap 6 件 + try_convert_as_hashmap inconsistency + Rule 3 SWC empirical 必須 wording 未実施 + plan/numbering ritual)。**主要変更**: `spec-stage-adversarial-checklist.md` Rule 4 sub-rule (4-1)(4-2)(4-3) + Rule 10 axis (i) AST dispatch hierarchy + Rule 11 AST node enumerate completeness check (sub-rule d-1〜d-4) + Rule 12 Mandatory application + structural enforcement (sub-rule e-1〜e-8) + **Rule 3 sub-rule (3-1)(3-2)(3-3) SWC parser empirical observation 必須** (Implementation Revision 2 lesson の self-applied integration)。`problem-space-analysis.md` non-matrix-driven 適用 spec 追加。`prd-template` skill Step 0c (Rule 10 application 必須 section) hard-code。`scripts/audit-ast-variant-coverage.py` (tree-sitter-rust 経由、`_` arm 全面禁止 + Tier sync verify) + `scripts/audit-prd-rule10-compliance.py` (yaml fenced code block parse + Rule 4 (4-3) doc-first dependency chain auto verify) + `.github/workflows/ci.yml` audit step + README branch protection 手順。`doc/grammar/ast-variants.md` に PropOrSpread (section 12) + Prop (section 13) + Decorator (section 20) 追加、PropName-TsTypeElement section 14-19 renumber、AutoAccessor entry を Tier 2 honest error reported via UnsupportedSyntaxError + I-201-A/B 言及に update。`src/pipeline/type_resolver/visitors.rs` (visit_class_body の StaticBlock arm + AutoAccessor explicit no-op + TsIndexSignature/Empty filter-out reason、`_` arm 削除) + `expressions.rs` (Object expr inner match で Prop::Method/Getter/Setter body の visit_block_stmt 経由 walk + visit_prop_method_function helper、Prop::Assign は Implementation Revision 2 で no-op) + `data_literals.rs` (3 site: convert_object_lit + convert_discriminated_union_object_lit + try_convert_as_hashmap、全 wildcard 削除 + UnsupportedSyntaxError 経由 Tier 2 honest error 統一)。**Implementation Revision 1 (Grammar gap、PropOrSpread section 不在)** + **Implementation Revision 2 (critical Spec gap、cell 15 Prop::Assign NA 誤認識を SWC parser empirical observation で覆し Tier 2 honest error reclassify)** を本 PRD scope 内 fix。**Test 完成**: TypeResolver layer 9 unit tests (StaticBlock typeof narrow / AutoAccessor no-op / Prop::Method/Getter/Setter body visit / Prop::Assign no-op + corollary / Prop::KeyValue/Shorthand regression) + Transformer layer 9 unit tests (cell 7/12-14/15/15-corollary/17-line:col + 10-11 regression) + SWC parser empirical regression test 3 件 (`tests/swc_parser_object_literal_prop_assign_test.rs`) + E2E fixture integration (`tests/e2e/scripts/prd-2.7/`、cell 6 = post-PRD I-204 として #[ignore]、cell 10/11 GREEN)。**Quality**: 3162 lib pass / 157 e2e pass + 30 ignored / 3 SWC parser pass、clippy 0 warning、fmt 0 diff、Hono bench clean 111 / errors 63 (0 regression)、audit-ast-variant-coverage.py PASS (本 PRD scope file)、audit-prd-rule10-compliance.py PASS (本 PRD doc self-applied)。**別 PRD 起票**: I-204 = Transformer StaticBlock emission strategy 改修 (cell 6 GREEN 化用、L1 候補)、I-203 = codebase-wide AST match exhaustiveness compliance (Rule 11 (d-1) 既存 codebase application)。 |
| **I-184 (CI fresh-clone defect: stale gitignored template files post pool refactor)** | 2026-04-27 | CI compile_test 3 件 panic (`failed to write tests/compile-check/src/lib.rs: No such file or directory`) を起点に、`/check_job` Layer 4 trade-off review + 歴史的経緯調査で **post-pool-refactor で I-161 の「write-only artifact」前提が消滅** していたが gitignore のみ stale 化していた事実を文書化、Problem Space matrix で全 12 cell を網羅 enumerate して 4 件 ✗ cell を確定。**Approach A (asymmetric design)**: 各 subproject の access pattern を honest に反映 — compile-check は **template-buffer pattern** (毎 test `fs::write` 上書き → `*.rs` ignore + `.keep` で dir 保持)、e2e/rust-runner は **pool pattern** (`E2eRunnerPool`、template = read-only skeleton → `src/main.rs` を tracked stub 化、`.keep` 配置せず、pool init は `copy_runner_template_file("src/main.rs", ...)` 統一形)。両 subproject の `Cargo.lock` は test fixture reproducibility 保証のため tracked 化。**変更**: `.gitignore` 全面書き直し (asymmetric handling 根拠を comment 明記、stale `write_with_advancing_mtime` 言及削除)、両 `Cargo.lock` (`cargo generate-lockfile` で再生成し tracked)、`tests/compile-check/src/.keep` 新規、`tests/e2e/rust-runner/src/main.rs` を doc comment 付き stub として tracked 化、`tests/compile_test.rs` / `tests/e2e_test.rs` doc comment 整合更新。**True fresh-clone state verify** (両 Cargo.lock + compile-check src/lib.rs 削除、e2e tracked stub は保持): compile_test 3 pass / e2e_test 157 pass 29 ignored、`cd tests/e2e/rust-runner && cargo metadata && cargo generate-lockfile` が temp stub 作成なしで成功 (template が valid Cargo project)、clippy 0 warning、fmt 差分なし。**Lesson** (`backlog/I-184` 内 3 項目 fully recorded): (1) Stale gitignore は latent CI defect 温床 — refactor 時に access pattern 変化 file の git tracking state 整合 checklist が必要、(2) False symmetry を避ける — 表層 DRY は本質的差異を覆い隠し vestigial defensive coding を生む、(3) PRD `Background` に歴史的経緯を必ず記録。 |
| **I-177-E + I-177-B + I-177-F batch (Plan η Step 1.5 + Step 2 + Step 2.5)** | 2026-04-26 | **I-177-E**: `SyntheticTypeRegistry::fork_dedup_state` で `types: BTreeMap::new()` を `types: self.types.clone()` に修正、builtin pre-registered union 型を fork から query 可能に。`SyntheticTypeDef` / `SyntheticTypeKind` に `Clone` derive 追加。`compute_complement_type` が `synthetic_enum_variants` query で正しく variants を取得できるようになり、typeof / instanceof / OptChain narrow guard with synthetic union の post-narrow EarlyReturnComplement event 押下が cohesive に。production code change ~3 行 + test 3 件追加 + E2E 1 fixture。**I-177-B**: `FileTypeResolution` に `resolve_var_type(name, span)` / `resolve_expr_type(expr)` canonical primitive を追加、3 production site (`Transformer::get_type_for_var` / `get_expr_type` / `transformer::return_wrap::collect_expr_leaf_types`) を canonical 経由に統一。「Ident は narrowed_type 優先 → expr_type fallback」knowledge を 1 箇所に集約し DRY violation 完全解消。production code change ~75 LOC (canonical helper) + 5 unit test。**I-177-F (scope 拡張済)**: `resolve_arrow_expr` / `resolve_fn_expr` / `visit_class_decl` constructor body / `visit_method_function` class method body の **4 site** で `for stmt in &body.stmts { ... }` を `self.visit_block_stmt(body)` に統一 (`visit_fn_decl` と完全 symmetric)、`current_block_end` を全 fn body 形式で set。production change 4 行 + 4 unit test (arrow form `#[ignore]` 解除 + fn_expr / class method / class constructor 新規) + 1 E2E fixture。**`/check_job` 4-layer review に基づく追加対応 (1 度目 + 2 度目)**: (1) Cell #5 (AnyEnum) NA 確定、(2) `apply_substitutions_to_items` doc-impl mismatch を call site comment 強化 → **I-177-G** (defense-in-depth) 起票、(3) test name prefix を全新規 test に統一、(4) cross-axis matrix completeness を non-matrix-driven PRD でも適用する framework 改善 → **I-198** 起票 (5 度の Spec gap chain で Severity reinforced)、(5) `/check_job deep deep` 2 度目で I-177-F の Cross-axis 直交軸 audit 不足 (class method / constructor 漏れ) を発見し本 PRD scope に編入、(6) `/check_job deep deep` 2 度目で TypeResolver coverage gap (static block / AutoAccessor / object literal method body) を発見 → **I-199 + I-200** 起票 (本 batch scope 外、TypeResolver coverage extension の broader 概念で別 PRD batch 化)。Hono bench 0 regression (clean 111 / errors 63 unchanged)。**次の作業**: PRD 3 (I-177 mutation propagation 本体、Tier 0 silent semantic change)、ただし I-199 + I-200 batch (TypeResolver coverage extension) と I-198 (framework Rule 10 拡張、Severity reinforced) を pre-Step 3 候補として user 判断 |
| **I-177-D (TypeResolver `narrowed_type` suppression scope refactor、案 C、Plan η Step 1)** | 2026-04-26 | trigger-kind-based dispatch refactor (Primary 非 suppress / EarlyReturnComplement 維持 suppress) で I-161 T7 cohesion gap を architectural に解消。**Plan η framework の最初の適用**: prd-template skill + spec-stage-adversarial-checklist 10-rule + check-job-review-layers 4-layer + post-implementation-defect-classification 5-category を初実戦投入し、`/check_job` 2 度の review iteration で findings 全 fix。Tier 3-4 deferral として **I-194** (typeof if-block elision、I-177-A scope 拡張候補) / **I-195** (struct field literal coerce) / **I-196** (framework dimension 拡張) / **I-197** (test 名 prefix audit) を TODO 起票。T7 i177-d 5 E2E fixtures は post-I-048 + post-I-177-A + post-I-162 の合成 dependency により ignore scaffold で保持、T7-3 ignore annotation を 3-fix dependency (I-177-D / I-177 main / I-048) 明記に update |
| **I-178 + I-183 + Rule corpus optimization batch** | 2026-04-25 | matrix-driven PRD framework (10-rule checklist + 4-layer review + 5-category defect classification) を整備、`.claude/rules/` 21 file + `.claude/skills/` 18 skill + `.claude/commands/` 9 command + CLAUDE.md に reference graph を確立。Tier 3-4 deferral として [I-184]〜[I-193] (10 件) を TODO 起票 |
| **I-161 + I-171 batch (`&&=`/`||=` desugar + Bang truthy emission)** | 2026-04-22〜04-25 | narrow-related compile error の structural fix。**T7 で `narrowed_type` suppression scope の architectural cohesion gap を発見** → I-177-D PRD で解消済 (2026-04-26)。narrow-scope mutation propagation 欠陥が runtime 誤動作として顕在化 → I-177 (Tier 0 L1) として umbrella 化、3 sub-item (A/B/C) 集約 |

---

## 次の作業

**優先順位は [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md) (L1 > L2 > L3 > L4) および [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) (silent semantic change を最優先) に従う。**

### 実行順序 (prerequisite chain、Plan η 確定 2026-04-26 + Step 1.5 + 2.5 挿入後)

```
[PRD 1: I-177-D — TypeResolver suppression scope refactor、案 C] (完了 2026-04-26)
       │
       ▼
[PRD 1.5: I-177-E — TypeResolver synthetic fork inheritance gap fix] (完了 2026-04-26)
       │
       ▼
[PRD 2: I-177-B (~75 LOC) — collect_expr_leaf_types query 順序 fix + canonical helper extract、non-matrix-driven] (完了 2026-04-26)
       │
       ▼
[PRD 2.5: I-177-F — resolve_arrow_expr / resolve_fn_expr block_end traversal cohesion] (完了 2026-04-26)
       │
       ▼
[PRD 2.7: I-198 + I-199 + I-200 cohesive batch — framework Rule 改修 (Rule 3/4/10/11/12) + TypeResolver coverage extension + ast-variants.md Prop/PropOrSpread/Decorator section 新規追加 + audit scripts CI 化] (完了 2026-04-27)
       │
       ▼
[PRD 2.75: I-205 — Class member access dispatch with getter/setter methodology framework (PRD 2.8/2.9/I-201-B prerequisite、Tier 2 broken framework → Tier 1 完全変換、L2、Spec stage v7 final 完了 2026-04-28、Implementation Stage T1〜T10 完了 2026-05-01、T11 (static accessor dispatch + matrix expansion) ← **次着手**)]
       │
       ▼
[PRD 2.8: I-201-A — AutoAccessor 単体 Tier 1 化 (decorator なし subset、I-205 framework leverage)] (L3、user 承認 2026-04-27)
       │
       ▼
[PRD 2.9: I-202 — Object literal Prop::Method/Getter/Setter Tier 1 化 (Transformer 完全 emission)] (L3、user 承認 2026-04-27)
       │
       ▼
[PRD 3: I-177 mutation propagation 本体 (Tier 0 silent semantic change、L1) — F1/F3 body mutation、案 A vs 案 B 確定]
       │
       ▼
[PRD 4: I-177-A — else_block_pattern Let-wrap (+ I-194 typeof if-block elision 拡張可)]
       │
       ▼
[PRD 5: I-177-C — symmetric XOR early-return detection]
       │
       ▼
[PRD 6: I-048 — closure ownership 推論 (T7-3 完全 GREEN-ify)]
       │
       ▼
[PRD 7: I-201-B — Decorator framework 完全変換 (TC39 Stage 3、L1 silent semantic change)] (user 承認 2026-04-27、PRD 3 後の next-priority L1 = reachability 軸 + I-201-A foundation leverage)
       │
       ▼
I-162 → Phase A Step 5 → I-015 → I-158+I-159 → Phase A Step 6 → ...
```

**PRD 凝集度原則 (2026-04-26 user 確定)**: 凝集度高 + 適切な粒度。1 PRD = 1 architectural concern。

- **PRD 1.5 (I-177-E、完了)**: TypeResolver synthetic fork inheritance gap fix。`fork_dedup_state` を `union_dedup` 継承 + `types` 空 fork → 全 state clone 形式へ修正。
- **PRD 2 (I-177-B、完了)**: `collect_expr_leaf_types` (`return_wrap.rs:419`) の query 順序を `narrowed_type → expr_type` に修正、Transformer 一般 path との整合性回復。canonical primitive `FileTypeResolution::resolve_var_type` / `resolve_expr_type` を追加し 3 site (`get_type_for_var` / `get_expr_type` / `collect_expr_leaf_types`) を統一。~75 LOC + 5 unit test。
- **PRD 2.5 (I-177-F、完了)**: `resolve_arrow_expr` / `resolve_fn_expr` / `visit_class_decl` constructor / `visit_method_function` の 4 site の body 直接 stmt iterate を `visit_block_stmt` 経由に統一し `current_block_end` を全 fn body 形式で set (`visit_fn_decl` と完全 symmetric)。production change 4 行 + 4 unit test + 1 E2E。I-177-B の callable arrow form `#[ignore]` 解除完了。**`/check_job deep deep` audit (2026-04-26) で class method / constructor 漏れを発見 → 本 PRD scope に編入 (初版 PRD scope の Cross-axis 直交軸 audit 不足が判明、I-198 framework 改善 TODO に lesson reflect 済)。**
- **PRD 2.7 (I-198 + I-199 + I-200 cohesive batch、完了 2026-04-27)**: framework Rule 改修 + structural enforcement の cohesive batch (Q4 + Q5 + **Q6 = Rule 4 改修** + **post-`/check_job` F1 = Q3 Rule 3 改修** = Implementation Revision 2 lesson の self-applied integration)。
  - **Q4** (Rule 10 sub-rule (d) AST node enumerate completeness): `_` arm 全面禁止 + 既存 `UnsupportedSyntaxError` mechanism 統一 + `doc/grammar/ast-variants.md` single source of truth + audit-ast-variant-coverage.py CI 化
  - **Q5** (Rule 10 sub-rule (e) Mandatory 化): 全 PRD Mandatory + matrix 不在の structural reason 明示 + machine-parseable format + Anti-pattern 明示禁止 list + `prd-template` skill hard-code + audit-prd-rule10-compliance.py CI 化 + merge gate
  - **Q6** (Rule 4 sub-rule (f) doc-first dependency order): PRD 内 doc update task が code 改修 task の prerequisite (= single source of truth structural 維持) + audit-prd-rule10-compliance.py に Task List dependency chain auto verify 拡張
  - TypeResolver coverage extension (StaticBlock visit + Prop::Method/Getter/Setter body resolve + AutoAccessor (b) Tier 2 error report 化で silent drop 排除) + `doc/grammar/ast-variants.md` Prop section 新規追加 (Grammar gap fix、Q6 doc-first compliance に従い T11 が T8/T9/T10 の prerequisite)
  - 1 architectural concern = "framework Rule 改修 (Rule 10 + Rule 4) + 拡張による coverage gap detection 完成 + structural enforcement"。
  - **Q3 (Prop::Assign) は本 PRD 内 NA cell + lock-in test で triple ideal 自動達成、別 PRD 不要**。
  - **既存 codebase 全体の `_` arm refactor (I-203) と AutoAccessor 完全 Tier 1 化 (I-201-A) と decorator framework (I-201-B) と object literal Tier 1 化 (I-202) は (d) 構造分離 pattern で別 PRD 化**。
- **PRD 2.8 (I-201-A、user 承認 2026-04-27)**: AutoAccessor 単体 Tier 1 化 (decorator なし subset、`accessor x: T = init` → `struct field + fn x() -> &T + fn set_x(&mut self, v: T)`、L3)。1 architectural concern = "AutoAccessor (decorator なし) class member emission completeness"。`doc/grammar/ast-variants.md` の AutoAccessor entry を Tier 2 → Tier 1 (decorator なし subset) に昇格。Decorator interaction 完全変換は PRD 7 (I-201-B) で別途達成 (1 PRD = 1 architectural concern 厳格適用)。
- **PRD 2.9 (I-202、user 承認 2026-04-27)**: Object literal `Prop::Method` / `Prop::Getter` / `Prop::Setter` Tier 1 化 (Transformer 完全 emission、L3)。post PRD 2.7 で Transformer convert_object_lit は Tier 2 honest error 状態、本 PRD で Tier 1 完全変換に拡張。1 architectural concern = "Object literal getter/setter/method emission completeness" (decorator なし、object literal context、I-201-A の class context と orthogonal)。
- **PRD 3 (I-177 mutation propagation 本体、L1 Tier 0)**: F1/F3 narrow body 内 mutation の outer Option<T> propagation (Tier 0 silent semantic change)。matrix-driven。案 A (mutation-ref `match &mut x`) vs 案 B (writeback `x.take()`) を spec stage で empirical 確定。
- **PRD 4 (I-177-A)**: `try_generate_narrowing_match` else_block_pattern bare match → Let-wrap 化、post-if narrow materialization。~20-30 LOC。**I-194 (typeof if-block elision) を scope 拡張候補として検討** (Phase 0 audit で発見の Transformer IR emission gap)。
- **PRD 5 (I-177-C)**: `visit_if_stmt` (then XOR else) 拡張 + guards.rs symmetric direction handling。~10-15 LOC。
- **PRD 6 (I-048)**: closure capture mode 推論 (move/FnMut/Fn)、T7-3 E0506 解消、closures/functions fixture unblock。要 spec stage 詳細化。
- **PRD 7 (I-201-B、user 承認 2026-04-27、L1 silent semantic change)**: Decorator framework 完全変換 (TC39 Stage 3、AutoAccessor + class + method + property + parameter 全 application 共通)。**Audit (2026-04-27) で ts_to_rs は decorator 自体が完全未実装 = silent drop 状態と判明** (= Tier 1 silent semantic change = L1 Reliability Foundation)。1 architectural concern = "Decorator framework full coverage"。PRD 3 と両 L1 だが reachability 軸 (PRD 3 = narrow 機能を使う全 TS code 広域 / I-201-B = decorator 含む TS code 局所、Hono 使用状況要 audit) で PRD 3 先行が暫定 default。I-201-A (AutoAccessor 単体 emission strategy) を foundation として leverage。要 spec stage 詳細化 (decorator hook semantic = init/get/set/addInitializer の Rust 等価表現確立)。
- **I-203 (Codebase-wide AST match exhaustiveness compliance、user 承認 2026-04-27、audit driven priority)**: PRD 2.7 で確定する Rule 10(d) 真の ideal (`_ => ` arm 全面禁止 + 共通 macro `unsupported_arm!()` + doc-code sync audit script + CI 化) の **既存 codebase 全体への application**。1 architectural concern = "Codebase-wide AST match exhaustiveness compliance"、(d) 構造分離 pattern で PRD 2.7 と独立。priority = audit 結果 driven (silent drop 含むなら L1、含まないなら L3)。実施: PRD 2.7 batch close 後の早期 audit、結果次第で PRD chain 内位置確定 (L1 = PRD 3 / I-201-B reachability 軸比較、L3 = PRD 7 後 deferred)。

### 着手順の導出原則

1. I-144 Dual verdict framework で `TS ✓ / Rust ✗` として分離された narrow-related compile error は I-144 context が fresh なうちに優先 (I-177-D / I-177)
2. Phase A roadmap (Step 5 → Step 6 → Step 7) で compile_test skip 直接削減
3. Phase B (RC-11 OBJECT_LITERAL_NO_TYPE 28件 = Hono 全 error の 45%) は Phase A 完了後
4. L4 latent items (runtime 同一 / reachability なし) は notes 欄に退避

### 着手順 table

| 優先度 | レベル | PRD | 内容 | 根拠 |
|--------|-------|-----|------|------|
| 0 (完了) | L4 | **PRD 1.5 + PRD 2 + PRD 2.5: I-177-E + I-177-B + I-177-F batch (synthetic fork + leaf type cohesion + arrow/fn-expr block_end)** | I-177-E: fork_dedup_state を全 state clone 化。I-177-B: canonical primitive を 3 site 統一 (DRY 完全解消)。I-177-F: resolve_arrow_expr / resolve_fn_expr の body traversal を visit_block_stmt 経由に統一、current_block_end を arrow / fn-expr 内でも set | 2026-04-26 完了。I-177-D + 本 batch で TypeResolver-IR cohesion + Synthetic registry cohesion + leaf type lookup cohesion + body traversal cohesion を確立 |
| 0 (完了) | L3 | **PRD 2.7: I-198 + I-199 + I-200 cohesive batch (framework Rule 改修 + TypeResolver coverage extension + ast-variants.md Prop/PropOrSpread/Decorator section 追加 + audit scripts CI 化)** | 2026-04-27 完了。Implementation stage T1〜T15 全 task + formal `/check_job` 4-layer review + 9 課題本質 fix (F1〜F10)。Implementation Revision 1 (PropOrSpread Grammar gap) + Revision 2 (cell 15 Prop::Assign critical Spec gap) self-applied integration。Spec gap chain trajectory **5→3→0→1→0→1→0** completion |
| **次着手** | **L3** | **PRD 2.8: I-201-A — AutoAccessor 単体 Tier 1 化 (decorator なし subset)** | TS 5.0+ stable AutoAccessor 構文 (`accessor x: T = init`) の decorator なし subset を Rust に完全変換 (`struct field + fn x() -> &T + fn set_x(&mut self, v: T)`)。ast-variants.md AutoAccessor entry を Tier 2 → Tier 1 (decorator なし subset) 昇格。Decorator interaction は PRD 7 (I-201-B) で別途達成 | user 承認 2026-04-27 (PRD 2.7 (d) 構造分離方針 + audit 2026-04-27 で decorator framework 未実装が判明、I-201 を I-201-A/I-201-B に分割)。1 PRD = 1 architectural concern 厳格適用 |
| **次優先 2 (post-PRD 2.8)** | **L3** | **PRD 2.9: I-202 — Object literal Prop::Method/Getter/Setter Tier 1 化** | post PRD 2.7 で Transformer convert_object_lit は Tier 2 honest error 状態、本 PRD で Tier 1 完全変換に拡張 (object literal の anonymous struct 表現 strategy 確立) | user 承認 2026-04-27 (Q2 (d) 構造分離方針)。class context (I-201-A) と orthogonal な architectural concern = "Object literal getter/setter/method emission completeness" |
| **0a (Tier 0)** | **L1** | **PRD 3: I-177 mutation propagation 本体 (narrow emission v2、L1 silent semantic change)** | I-144 T6-3 inherited の shadow-mutation-propagation 欠陥を structural fix。F1/F3 narrow body 内 mutation の outer Option<T> propagation を案 A (mutation-ref `match &mut x`) vs 案 B (writeback `x.take()`) で確定 | I-161 T3 実装で latent defect が runtime 誤動作として顕在化、Tier 0 silent semantic change 該当。matrix-driven |
| **0b (Tier 1)** | **L3** | **PRD 4: I-177-A (else_block_pattern Let-wrap 化)** | typeof/instanceof/OptChain × `then_exit + else_non_exit` × post-narrow primitive use の bare match → Let-wrap 化、INV-2 違反解消 (~20-30 LOC)。**I-194 (typeof if-block elision) を scope 拡張候補として検討** | I-171 T5 で発見、Plan η Step 4 |
| **0c (Tier 1)** | **L3** | **PRD 5: I-177-C (symmetric XOR early-return detection)** | `visit_if_stmt` (then XOR else) 拡張 + guards.rs symmetric direction handling (~10-15 LOC) | Plan η Step 5、narrow framework 対称性完成 |
| **0d (Tier 1)** | **L3** | **PRD 6: I-048 (closure ownership 推論)** | closure capture mode (move/FnMut/Fn) 推論。T7-3 E0506 解消、closures/functions fixture unblock。要 spec stage 詳細化 | Plan η Step 6、`closures` / `functions` fixture unskip、T7-3 完全 GREEN-ify |
| **次優先 (Tier 1、post-PRD 6)** | **L1** | **PRD 7: I-201-B — Decorator framework 完全変換 (TC39 Stage 3、L1 silent semantic change)** | ts_to_rs では decorator 自体が完全未実装 = silent drop 状態 (audit 2026-04-27)。decorator semantic (init/get/set/addInitializer hook) の Rust 等価表現確立、AutoAccessor + class + method + property + parameter 全 application 共通 framework 構築 | user 承認 2026-04-27 (audit 結果 driven)。PRD 3 と両 L1 だが reachability 軸 (PRD 3 広域 / I-201-B 局所) で PRD 3 先行が暫定 default、Hono decorator 使用状況 audit で再評価可能。I-201-A foundation を leverage |
| **audit driven (post-PRD 2.7)** | **L1 候補 / L3** | **I-203: Codebase-wide AST match exhaustiveness compliance (Rule 10(d) compliance、既存 `_` arm 全 audit + explicit enumerate fix)** | PRD 2.7 で確定する Rule 10(d) 真の ideal (`_` arm 全面禁止 + 共通 macro `unsupported_arm!()` + doc-code sync audit script) の既存 codebase 全体 application。silent drop 候補が含まれるかを audit で確定し priority reclassify | user 承認 2026-04-27 ((d) 構造分離 pattern)。PRD 2.7 batch close 後の早期 audit 実施、結果次第で PRD chain 内位置確定 (L1 = PRD 3 / I-201-B reachability 軸比較、L3 = PRD 7 後 deferred) |
| 1 | L3 | **I-162** | class without explicit constructor → `Self::new()` 自動合成 | I-144 T2 instanceof narrow の Rust 側 E2E lock-in が本 defect で block。`class Dog {}` → `struct Dog {}` 止まりで `Dog::new()` 不在で E0599 |
| 2 | L3 | **Phase A Step 5** (I-026 / I-029 / I-030) | 型 assertion / null as any / any-narrowing enum 変換 | `type-assertion`, `trait-coercion`, `any-type-narrowing` unskip (3 fixture 直接削減) |
| 3 | L3 | **I-015** | Hono types.rs `Input['out']` indexed access 解決失敗 (E0405) | `src/ts_type_info/resolve/indexed_access.rs:271`。Hono types.rs で 1 件だが dir compile blocker |
| 4 | L3 | **I-158 + I-159 batch** | Non-loop labeled stmt + 内部 emission 変数 user namespace hygiene | I-154 変数版 + I-153 labeled block 対応。I-158 が I-153 emission と interaction のため I-158 先行推奨 |
| 5 | L3 | **Phase A Step 6** (I-028 / I-033 / I-034) | intersection 未使用型パラメータ (E0091) + charAt/repeat/toFixed method 変換 | `string-methods`, `intersection-empty-object`, `type-narrowing` unskip |
| 6 | L3 | **I-143 meta-PRD** | `??` 演算子の問題空間完全マトリクス + 8 未解決セル (a〜h) | I-143-a〜h 未着手。I-143-b (`any ?? T`) は I-050 依存、他は独立 |
| 7 | L3 | **I-142 Step 4 C-5 / C-6 + Phase A Step 7 (I-071)** | I-144 非吸収の small cleanup (C-7 は I-050 依存) + `instanceof-builtin` unskip 用 builtin 型 impl 生成 | C-5/C-6 は test quality 改善 (handoff doc)、I-071 は Phase A 最終 step (1 fixture unskip) |
| 8 | L3 | **Phase B (RC-11)** (I-003 / I-004 / I-005 / I-006) | expected type 伝播の不完全性 (OBJECT_LITERAL_NO_TYPE 28件) | Hono 全 error の 45%、Phase A 完了後の最大インパクト category |

**注**: 各 PRD で `prd-template` skill + `.claude/rules/problem-space-analysis.md` + `.claude/rules/spec-first-prd.md` + `.claude/rules/spec-stage-adversarial-checklist.md` (10-rule) + `.claude/rules/check-job-review-layers.md` (4-layer) を適用する。

### 次点 / L4 deferred (上記 table 外)

- **I-013 + I-014 batch** (L3、RC-5 abstract class 変換パス欠陥) — class inheritance 系、抱え込み依存が強いため独立 PRD 着手時に整備
- **I-140** (L3、TypeDef::Alias variant 追加) — `type MaybeStr = string \| undefined` alias 経由の Option 認識。I-134 / I-056 と batch 可能
- **I-050 umbrella** (L3、Any coercion) — I-143-b + I-050-b + I-050-c が依存。structural 母体として設計維持
- **I-146** (L3、`return undefined` on void fn) — `keyword-types` unskip の残条件
- **I-074** (L4、`Item::StructInit` broken window) — pipeline-integrity 違反、PRD 化候補
- **I-160** (L4、Walker defense-in-depth Expr-embedded Stmt::Break) — 現時点 reachability なし
- **I-165 / I-166 / I-167 / I-170** (L4 narrow precision umbrella) — I-144 後の latent imprecision、runtime 動作同一、Rust 精度のみ向上
- **I-168** (L4、`NarrowEvent::Reset` event 未消費) — Hono で顕在化なし pre-existing imprecision
- **I-172** (L4、bench 非決定性) — test / bench infra、別 PRD
- **I-177-G** (L4、`apply_substitutions_to_items` round-trip mutation safety、defense-in-depth) — 現状 reachability なし、I-177-E fork inheritance fix で顕在化候補に。I-177-E + I-177-B batch close 由来 (2026-04-26)
- **I-198 + I-199 + I-200 batch** — **PRD 2.7 として進行中 (Spec stage、user 承認 2026-04-27)**。本 deferred section から進行中作業へ昇格。
- **I-201-A (AutoAccessor 単体 Tier 1 化、decorator なし subset)** — **PRD 2.8 として next-priority 1 (L3、user 承認 2026-04-27)**。本 deferred section から next-priority へ昇格。
- **I-202 (Object literal Prop::Method/Getter/Setter Tier 1 化)** — **PRD 2.9 として next-priority 2 (L3、user 承認 2026-04-27)**。本 deferred section から next-priority へ昇格。
- **I-201-B (Decorator framework 完全変換、TC39 Stage 3)** — **PRD 7 として post-PRD 6 next-priority (L1 silent semantic change、user 承認 2026-04-27)**。audit 2026-04-27 で decorator framework 未実装 = silent drop 状態が判明、L1 priority。本 deferred section から PRD chain へ昇格。
- **I-203 (Codebase-wide AST match exhaustiveness compliance — 既存 `_` arm 全 audit + explicit enumerate fix)** — **PRD 2.7 完了後の早期 audit 実施 (audit driven priority、L1 候補 = silent drop 含む / L3 = silent drop 不在)、user 承認 2026-04-27**。Rule 10(d) 真の ideal の codebase-wide application、(d) 構造分離 pattern で本 entry に分離。audit 結果次第で PRD chain 内挿入位置確定 (L1 確定なら PRD 3 / I-201-B と reachability 軸比較、L3 確定なら PRD 7 後 deferred)。
- **I-206 〜 I-211 batch (PRD I-205 Spec stage Discovery 由来 6 件、user 確定 2026-04-28)** — PRD I-205 Spec stage で発見、本 PRD I-205 では Tier 2 honest error reclassify 化、Tier 1 化は別 architectural concern として split 確定:
  - **I-206** (L3、Class inheritance dispatch — B7 inherited accessor の Tier 1 化) — I-013 + I-014 (abstract class 変換パス) と cohesive batch 候補
  - **I-207** (L3、Destructure pattern dispatch — class instance / getter destructure) — I-202 (Object literal) と cohesive 候補
  - **I-208** (L3、Class Method body T-aware comprehensive `.clone()` insertion C2 pattern) — I-048 closure ownership 推論完了後 next 候補 (cell 80 nested closure body は I-048 と integrate 必須)
  - **I-209** (L3、Function reference semantic — class regular method `obj.x` no-paren reference の Tier 1 化) — I-048 完了後 closure wrap strategy で leverage
  - **I-210** (L3、typeof of class instance member の Tier 1 化 — runtime string return semantic) — Hono bench reachability audit で priority 確定
  - **I-211** (L3、`in` operator on class instance の Tier 1 化 — property reflection semantic) — Hono reachability 低想定、I-206 完了後 inherited property 対応で integrate
- **I-212 (Framework convergence metric framework PRD)** — L4 framework infra、PRD I-205 iteration v7 F-deep-deep-8 由来 (`spec-stage-adversarial-checklist.md` v1.3 → v1.6 連続 revision の収束判定 mechanism 不在)、user 確定 2026-04-28。framework rule 改修頻度 audit (e.g., 6 revision in 1 month threshold 超過) で L3 promote 余地。
- **I-214 (`convert_call_expr` static method call dispatch DRY violation + 3 latent gaps)** — L3 (Hono reachability audit 後 L1 promote 余地)、PRD I-205 T6 Iteration v10 third-review (`/check_problem`) 由来、user 確定 2026-04-28。`src/transformer/expressions/calls.rs:213-225` (I-378 T9 由来 Static method call dispatch) と T6 で導入された `classify_member_receiver` shared helper の DRY violation + calls.rs 側の 3 latent gaps (a) is_interface filter なし / (b) `get_expr_type` None gate なし (= shadowing 不防止) / (c) `lookup_method_sigs_in_inheritance_chain` 不使用 (= inherited static method call で `Sub::method` emit、compile error)。修正方針: calls.rs を classify_member_receiver 経由 refactor + `dispatch_static_member_call(class_name, sigs, is_inherited, args)` 新規 helper。**T11 (11-b/c/d/f) と cohesive batch 候補** = "Static dispatch full coverage" PRD として 1 architectural concern 統合検討。
- **I-215 (`arr.length = v` write Tier 2 silent gap — TS truncate semantic vs Rust E0609)** — L4 (Localized、Vec/Array specific syntax のみ、Hono reachability 低想定)、PRD I-205 T6 Iteration v10 third-review 由来、user 確定 2026-04-28。Read 側 `arr.length` (T5 既存 `arr.len() as f64`) と Write 側 `arr.length = v` (現状 `Expr::Assign { FieldAccess, value }` で E0609 emit) の対応度 asymmetric。修正方針: オプション A (Tier 1 truncate/clear/resize emission) / オプション B (Tier 2 honest error reclassify)、Hono reachability audit で priority 確定。
- **I-216 (`!(b.x = 5)` bang on B4 setter assignment Tier 2 behavioral change の structural enforcement)** — L4 (Localized、`!(assign-expr)` syntax は anti-pattern で Hono reachability 極低想定)、PRD I-205 T6 Iteration v10 third-review 由来、user 確定 2026-04-28。T6 で B4 plain assign が `Expr::MethodCall { set_x, [value] }` に変わった結果、`convert_bang_assign` (binary.rs:509) destructure pattern が fail、Layer 4 fall-through で `!MethodCall` (= `!void`) compile error。Pre/post T6 両方とも Tier 2、silent semantic change なし。修正方針: オプション A (`convert_bang_assign` を MethodCall setter dispatch に拡張) / オプション B (Tier 2 honest error reclassify)、reachability ゼロなら scope 外 + 永続 ignore 候補。
- **I-219 (TypeResolver `resolve_member_type` returns Unknown for class member getter access — Spec gap)** — L3 (architectural concern: TypeResolver type tracking completeness for class member access via getter)、PRD I-205 T8 second-review F-SX-1 で予測、T9 Iteration v14 deep-deep review で再確認、user 確定 2026-04-30。`src/pipeline/type_resolver/expressions/member.rs::resolve_member_type` は registry の `lookup_field_type` 経由で fields のみ check、class methods (Getter/Setter) は skip → `c.value` の expr_types[member_span] は Unknown。Side effect: ??=/&&=/||= × class member の RHS expected_type propagation が動作せず、TypeResolver が rhs を inner T で coerce できない (= silent type widening risk for unusual rhs types)。T9 では sigs から Getter return type を直接抽出する self-contained 回避を採用 (TypeResolver 拡張は scope 外)。**修正方針**: `resolve_member_type` で fields lookup miss 後 methods registry を check、Getter sig の return_type を type-of-member-access として返却。Read context (`resolve_member_expr`) + assign context (`resolve_assign_expr` is_propagating_op block) 両 path で benefit。**Caveat**: 非対称 setter (= `set value(v: T)` accepts T but `get value(): Option<T>` returns Option<T>) で Read context type tracking と Write context expected propagation が divergent、要 separate treatment (= `resolve_member_type_for_write` 別 helper 候補)。Subsequent PRD で取り扱う、本 T9 では sigs-based extraction で代替済。
- **I-220 (Setter accept type asymmetry vs getter return type — pre-existing pattern T6/T7/T8/T9 共通 latent gap)** — L4 (Localized、TS asymmetric setter syntax は corner case、Hono reachability 低想定)、PRD I-205 T9 Iteration v14 deep-deep review 由来、user 確定 2026-04-30。`set value(v: T)` accepts T while `get value(): Option<T>` returns Option<T> の asymmetric class definition で、T9 `wrap_setter_value` は lhs_type = getter return type を見て Some-wrap、setter accept type と一致しない → Some(rhs) 渡しが setter expecting T に対し type mismatch (Rust E0308 catch)。T6 `dispatch_member_write` も raw value passing で同類 issue。**修正方針**: setter sig (= MethodKind::Setter sig with params[0].ty) を抽出する extract_setter_accept_type helper を追加、wrap_setter_value で getter return type ではなく setter accept type を見る形に refactor。T6/T7/T8/T9 全 dispatch helpers で同 fix を symmetric apply。reachability audit で priority 確定。
- **I-221 (Top-level Module-level statement path uses expression-context dispatch — pre-existing infrastructure gap)** — L4 (infra、E2E fixture suboptimal Rust output、functional Tier 1)、PRD I-205 T8/T9 E2E fixture probe 由来、user 確定 2026-04-30。Top-level `obj.x ??= 42;` / `obj.x += v;` 等の statement が convert_stmt path ではなく convert_expr path 経由で処理される模様 (= TailExpr 付き Block emit、Stmt::Expr で discard)。Functional Tier 1 だが Rust output に余分な TailExpr 残存。Pre-existing (T8 cells 21/27 でも同 pattern 観察)。**修正方針**: top-level Module statement processing path で convert_stmt 経由を確認、必要なら expression-context fallback を統一。**T14 scope 候補** (E2E fixtures green-ify と cohesive)。

### Batching 検討

未着手 batch 候補 (上記 table 内 PRD 着手時に再検討):

- **I-158 + I-159**: namespace hygiene 系 (I-154 と同系)。I-158 先行推奨 (I-153 emission との interaction)
- **I-143 + I-050-b + I-050-c**: `??` / Any / Synthetic union coercion が共通 `resolve_expr` / `propagate_expected` 基盤を持つ
- **I-140 + I-134 + I-056**: type alias 関連、`TypeDef::Alias` variant 新設で DRY 可能
- **I-013 + I-014**: abstract class 変換パス (強依存、`generate_child_of_abstract()` 拡張)
- **I-165 / I-166 / I-167 / I-170**: narrow precision umbrella (`VarId` binding identity + CFG analysis の基盤を共有)

---

## 次の PRD 着手前の参照ポイント

新規 PRD 着手時は `prd-template` skill + 関連 rule (`problem-space-analysis.md` / `spec-first-prd.md` / `spec-stage-adversarial-checklist.md` / `check-job-review-layers.md`) を適用する。

特定 PRD 用の handoff doc:

- **Phase A Step 5 / 6 / 7**: 下記「開発ロードマップ」section + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- **I-144 設計判断 (archive)**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) (CFG narrowing analyzer / NarrowTypeContext trait / EmissionHint dispatch / coerce_default table / closure reassign Policy A)
- **I-142 Step 4 残余 (C-5〜C-9)**: [`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md)
- **I-158 / I-159**: TODO 参照
- **I-143 meta-PRD (`??` 完全仕様)**: TODO I-143 本体 + a〜h 未解決セル

---

## 設計判断の引継ぎ

後続 PRD 向けの設計判断アーカイブは [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) に集約。新規 PRD 着手時は関連 section を事前レビュー、実装が設計判断と乖離していたら該当 section を最新化 (削除は禁止 — 過去の設計判断は reference として保持)。

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。skip 解消後は新たな skip 追加を原則禁止とし、回帰検出を自動化する。

**完了済 (Step 0〜4 + I-153/I-154 + pre-Step-3)**: 詳細は git log + [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md) 参照。

**永続 skip (設計制約 4 件)**:

- `callable-interface-generic-arity-mismatch` — 意図的 error-case (INV-4)
- `indexed-access-type` — マルチファイル用 (`test_multi_file_fixtures_compile` でカバー)
- `vec-method-expected-type` — no-builtins mode 限定の設計制約
- `external-type-struct` — no-builtins mode 限定の設計制約 (with-builtins 側は Step 1 で解消済)

**effective residual (10 fixture)**: trait-coercion, any-type-narrowing, type-narrowing, instanceof-builtin, intersection-empty-object, closures, functions, keyword-types, string-methods, type-assertion

#### 次の Step

```
Step 5 (type conversion + null)       I-026 + I-029 + I-030
  ↓ I-158 / I-159 (hygiene follow-ups、並行可能)
Step 6 (string + intersection)        I-028 + I-033 + I-034
  ↓
Step 7 (builtin impl)                 I-071
```

#### Step 5-7 詳細 (未着手)

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

#### 残 fixture × 解消依存

| fixture | 解消 Step / 依存 | メモ |
|---------|-----------------|------|
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

Phase A 完了後、Hono ベンチマーク最大カテゴリ (全エラーの 45%) に着手。I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback) を対象とする。

---

## リファレンス

- 最上位原則: [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md)
- 優先度ルール: [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md)
- TODO 記載標準: [`.claude/rules/todo-entry-standards.md`](.claude/rules/todo-entry-standards.md)
- PRD workflow: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md)
- Spec stage 完了 verification: [`.claude/rules/spec-stage-adversarial-checklist.md`](.claude/rules/spec-stage-adversarial-checklist.md) (10-rule)
- Implementation stage 完了 verification: [`.claude/rules/check-job-review-layers.md`](.claude/rules/check-job-review-layers.md) (4-layer)
- 設計整合性: [`.claude/rules/design-integrity.md`](.claude/rules/design-integrity.md) + [`.claude/rules/prd-design-review.md`](.claude/rules/prd-design-review.md)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- PRD handoff: `doc/handoff/*.md`
- Grammar reference: `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md`
- TODO 全体: [`TODO`](TODO)
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
- 実装調査 report: `report/*.md`
