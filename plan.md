# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-27 post I-184 close)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 111/158 (70.3%) |
| Hono bench errors | 63 |
| cargo test (lib) | 3144 pass / 0 fail / 0 ignored (I-177-F batch で 1 ignored 解消 + class method/constructor 2 test 追加) |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 157 pass + 29 `#[ignore]` |
| clippy | 0 warnings |
| fmt | 0 diffs |

**bench 非決定性**: ±1 clean / ±2 errors の noise variance を [I-172] として記録 (test/bench infra defect、別 PRD)。

### 進行中作業

**I-198 + I-199 + I-200 cohesive batch (PRD 2.7、Plan η Optional pre-Step 3、user 承認 2026-04-27)** Spec stage 進行中。Architectural concern = "framework Rule 10 拡張 + 拡張による coverage gap detection 完成" (1 PRD = 1 architectural concern)。

本 batch close 後、**I-201-A (AutoAccessor 単体 Tier 1 化、decorator なし subset、L3) → I-202 (Object literal Prop::Method/Getter/Setter Tier 1 化、L3) → PRD 3 (I-177 mutation propagation 本体、L1 Tier 0)** の順で進む。**Decorator framework 完全変換 (I-201-B、L1 silent semantic change)** は PRD 3 / 4 / 5 / 6 close 後の next-priority L1 (両者とも L1 だが reachability 軸で PRD 3 先行が暫定 default、Hono decorator 使用状況 audit で再評価可能、user 承認 2026-04-27 audit 結果 driven の re-plan)。

**Audit 知見 (2026-04-27)**: ts_to_rs では decorator 自体が完全に未実装 = silent drop 状態 (`grep "Decorator\|decorator" src/` 結果空、`doc/grammar/ast-variants.md` に Decorator entry 不在)。元 I-201 (AutoAccessor + decorator interaction) を I-201-A (AutoAccessor 単体、decorator なし subset、L3) + I-201-B (Decorator framework 全体、L1 silent semantic change) に分割 (1 PRD = 1 architectural concern 厳格適用)。

**Q4 確定 (2026-04-27)**: Rule 10(d) "AST node enumerate completeness check" の真の ideal を確定 — `_ => ` arm 全面禁止 + 共通 macro `unsupported_arm!()` + ast-variants.md を single source of truth + audit script CI 化。新規 code 適用は PRD 2.7 内 (I-199 / I-200 + audit script 作成)、既存 codebase 全体への application は **I-203 (Codebase-wide AST match exhaustiveness compliance)** として (d) 構造分離 pattern で別 PRD 起票。I-203 priority は audit 結果 driven (silent drop 含むなら L1、含まないなら L3)。

**Q5 確定 (2026-04-27)**: Rule 10 全体の Mandatory 化 + structural enforcement を確定 — 全 PRD で Rule 10 verification を Mandatory + matrix 不在の structural reason 明示 (Permitted reason 例示 + Prohibited keywords 明示禁止 list) + Cross-axis 軸独立 enumerate + machine-parseable format + **`prd-template` skill hard-code** + **`scripts/audit-prd-rule10-compliance.py` 新規作成 + CI 化 + merge gate**。人間判断介在 0、妥協の逃げ道 structural 排除。本 PRD 2.7 内で完結 (Q4 と Q5 の rule 改修 + skill / audit script を同時 update)。

**Q6 確定 (2026-04-27、3 度目 `/check_job` Spec stage 10-rule review で発見した Spec gap × 1 件の framework 改善検討の本質対応)**: `spec-stage-adversarial-checklist.md` Rule 4 wording 拡張で **doc-first dependency order の structural enforcement** を確立 — PRD 内 doc update task は code 改修 task の prerequisite (= single source of truth、Q4 整合) + `scripts/audit-prd-rule10-compliance.py` で Task List dependency chain auto verify (= 人手判断介在排除)。本 PRD draft 自体の T11 dependency violation (= 1 度目 + 2 度目 review で未検出) を起点に、Rule 4 改修を本 PRD scope に integrate (1 PRD = 1 architectural concern の "framework Rule 改修" cohesive concern 内、Q4/Q5/Q6 統合)。本 PRD 自体が Rule 4 改修の first-class adopter として self-applied verify。

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
PRD 2.7 (I-198 + I-199 + I-200 cohesive batch): framework Rule 10 拡張 (新 sub-rule (i) AST node enumerate completeness check) + TypeResolver coverage extension (StaticBlock + Prop::Method/Getter/Setter body resolve + AutoAccessor (b) Tier 2 error report 化) + ast-variants.md Prop section 新規追加 ← 進行中 (Spec stage、user 承認 2026-04-27)
   ↓
PRD 2.8 (I-201-A): AutoAccessor 単体 Tier 1 化 (decorator なし subset、`accessor x: T = init` → `struct field + getter/setter pair`、L3、user 承認 2026-04-27)
   ↓
PRD 2.9 (I-202): Object literal `Prop::Method` / `Prop::Getter` / `Prop::Setter` Tier 1 化 (Transformer 完全 emission、L3、user 承認 2026-04-27)
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
[PRD 2.7: I-198 + I-199 + I-200 cohesive batch — framework Rule 10 拡張 + TypeResolver coverage extension + ast-variants.md Prop section 新規追加] ← 進行中 (Spec stage、user 承認 2026-04-27)
       │
       ▼
[PRD 2.8: I-201-A — AutoAccessor 単体 Tier 1 化 (decorator なし subset)] (L3、user 承認 2026-04-27)
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
- **PRD 2.7 (I-198 + I-199 + I-200 cohesive batch、進行中 Spec stage)**: framework Rule 改修 + structural enforcement の cohesive batch (Q4 + Q5 + **Q6 = Rule 4 改修、Action 5 修正 2026-04-27**)。
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
| **進行中 (Spec stage)** | **L3** | **PRD 2.7: I-198 + I-199 + I-200 cohesive batch (framework Rule 10 拡張 + TypeResolver coverage extension + ast-variants.md Prop section 追加)** | I-198: spec-stage-adversarial-checklist Rule 10 に新 sub-rule (i) AST node enumerate completeness check 追加 + non-matrix-driven PRD への適用拡張。I-199: visit_class_decl の StaticBlock visit 追加 + AutoAccessor (b) Tier 2 error report 化 (silent drop 排除)。I-200: ObjectLit Prop::Method / Getter / Setter body resolve 経路追加。doc/grammar/ast-variants.md に Prop section 新規追加 (Grammar gap fix) | user 承認 2026-04-27 (Plan η Optional pre-Step 3 batch)。framework reinforced により後続全 PRD の structural quality 向上 (high leverage)。matrix-driven、Rule 10 拡張部は doc 改修 + I-199/I-200 部は ~50-80 LOC |
| **次優先 1 (post-PRD 2.7)** | **L3** | **PRD 2.8: I-201-A — AutoAccessor 単体 Tier 1 化 (decorator なし subset)** | TS 5.0+ stable AutoAccessor 構文 (`accessor x: T = init`) の decorator なし subset を Rust に完全変換 (`struct field + fn x() -> &T + fn set_x(&mut self, v: T)`)。ast-variants.md AutoAccessor entry を Tier 2 → Tier 1 (decorator なし subset) 昇格。Decorator interaction は PRD 7 (I-201-B) で別途達成 | user 承認 2026-04-27 (PRD 2.7 (d) 構造分離方針 + audit 2026-04-27 で decorator framework 未実装が判明、I-201 を I-201-A/I-201-B に分割)。1 PRD = 1 architectural concern 厳格適用 |
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
