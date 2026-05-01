# I-224 (B2 fn main mechanism) Spec Stage v3 Review Handoff

**作成日**: 2026-05-01
**Status**: Spec stage v2 完了 → 第三者 `/check_job` review で 16 findings 発見 → **Spec stage iteration v3 必要**、user 判断点あり (= H-2 Option α/β/γ)
**用途**: session 切替時の議論継続用 handoff document、`/start` 再開時に本 doc を読んで discussion をそのまま継続する

---

## 1. 議論の経緯 (high-level timeline)

### 1.1 Session 開始前の状態 (= 前 session 開始時 commit `6101829`)
- I-205 Implementation Stage T13 (Iteration v20) 完了
- T14 (E2E fixtures green-ify) が 次着手 task と plan.md に記載
- T14 description に "I-162 prerequisite block 明示" の note のみ

### 1.2 Session 内の主要 actions
1. **I-205 T14 着手判定 empirical 調査** (cells 02 / 09 / 13 / 21 / 43 / 60 を直接 transpile + cell-09 を `cargo build` で empirical verify) で **3 系統 prerequisite を発見**:
   - **B1 (= I-162)**: `class C { ... }` (no explicit constructor) → `Self::new()` 自動合成なし → `const c = new C();` が silent drop。**既存 TODO entry あり** (L3、`src/transformer/classes/generation.rs` 修正方針確定)
   - **B2 (= 新規)**: `pub fn init()` のみ emit、`fn main()` 自動生成なし → Rust E0601 + user main + top-level statements 共存で **silent dead code (= L1 silent semantic change)**
   - **B3 (= 新規)**: `class C { _n = 0; ... }` (annotation 無 + initializer 有) で `_n: serde_json::Value` fallback → L3 compile error blocker + L1 silent risk (Display/PartialEq impl 経由)
2. **設計案 4 つ + 星取表** (案 α Strict serial / **案 β Universal infra leverage first + L1 mid-priority** / 案 γ L1 first / 案 δ Family batches) → 案 β を 20/24 で最良判定 + user 承認
3. **TODO 更新**: I-224 (B2) + I-225 (B3) を Tier 1 ゲートイシュー sub-section に追加 (詳細 trace + L1 silent risk note 含む)
4. **plan.md 更新**: 案 β chain (B2 → B3 → I-162 → I-205 T14-T16 → I-D → I-177 → I-A → I-B → I-201-A → I-202 → I-201-B → I-177-A → I-177-C → I-048) に再構成
5. **B2 PRD draft 起票**: `/prd-template` skill で `backlog/I-224-top-level-fn-main-mechanism.md` 作成 (~600 行 → 727 行)
6. **iteration v1 → v2** で Spec Stage Tasks TS-1〜TS-4 を進行:
   - TS-3 + TS-1 batch: 26 fixtures 作成 (`tests/e2e/scripts/i-224/`) + tsc oracle observation 全 record
   - **Critical Spec gap discovery during TS-1**: tsx + cjs runtime が top-level await を reject (Top-level await is currently not supported with the "cjs" output format) → cells 14-18/30 (Axis C1 ✗) + cells 6/7/8 (Axis C1 NA) を **Out of Scope = 新 PRD I-226 (test harness ESM support + top-level await Tier 1) cohesive batch defer** に narrow
   - TS-2 → I-226 defer
   - TS-4 完了 (Empirical file path verify、`__ts_` namespace constants source = `src/transformer/expressions/mod.rs:57-98`、`check_ts_internal_label_namespace` validator = `src/transformer/statements/mod.rs:39-48` 等)
   - I-226 TODO 起票 (Tier 1 ゲートイシュー、L4)
   - In-scope cells = 14 ✗ + 6 regression lock-in = 20 cells
7. **iteration v2 self-review**: 13-rule self-applied verify 全 ✓、`audit-prd-rule10-compliance.py` PASS、Critical=0 / High=0 / Medium=0 / Low=0 と self-claim → "Spec stage approved、Implementation stage 移行可能" 判定
8. **commit** (commit message hash unknown 本 session 内): TODO + plan.md + backlog/I-224 + tests/e2e/scripts/i-224/ 全 staged
9. **第三者 `/check_job` review (skill invoke 経由)**: **Spec stage approval 不適切** + 16 findings + 5 review insights を発見 → 本 handoff doc 作成 (= 次 session で議論継続用)

### 1.3 現在 (= handoff doc 作成時点)
- B2 PRD draft v2: 727 lines、self-review v2 では Critical=0 / High=0 (false positive)
- 第三者 review 結果: **真の Critical=4 + High=8 + Medium=4 + Review insights=5**
- **Spec stage iteration v3 必要**、user 判断 (H-2 Option α/β/γ) 待ち

---

## 2. 第三者 `/check_job` review 結果 (16 findings + 5 review insights)

### 2.1 Critical Findings (Implementation stage 移行 block 必須、4 件)

#### C-1: Rule 1 (1-2) Cartesian product 完全 enumerate 違反 — 31/70 cells しか enumerate されず

**Trace**: Axis A (7 variants: A0/A1/A2/A3/A4/A5/A6) × Axis B (5 variants: B0/B1/B2/B3/B4) × Axis C (2 variants: C0/C1) = **70 cells** が Cartesian product 必須 (Rule 1 (1-2): "Cartesian product 完全 enumerate 必須、abbreviation pattern 全面禁止")。

実 enumerate cells (matrix): cell 1〜31 = **31 cells のみ**。**~39 cells が silent omission**。

具体的 missing cells (~39 件):
- A0 × B3 × C1 (= 非 fn main + top-await without execution): 1 cell
- A0 × B4 × C1 (= __ts_main collision + top-await without execution): 1 cell
- A2 × B0/B1/B2/B3/B4 × C1 (= Lit init only + top-await): 5 cells
- A2 × B2/B3/B4 × C0 (= Lit init only + async/non-fn/collision): 3 cells
- A3 × B0/B1/B2/B3/B4 × C1 (= side-effect init + top-await): 5 cells
- A3 × B4 × C0 (= side-effect init + collision): 1 cell
- A4 × B2/B3/B4 × C0/C1 (= control-flow + various B): 6 cells
- A5 × B1/B2/B3/B4 × C0/C1 (= Empty/Debugger + various B): 8 cells
- A6 × B3/B4 × C0/C1 + B0/B1 × C1 残: 9 cells

**audit-prd-rule10-compliance.py が PASS した理由**: audit script は `...` / range grouping pattern 検出のみで、**implicit cell omission は検出しない**。Rule 1 (1-2) の真の意図 (= 全 cell 独立 row) を verify する mechanism が audit script に欠落 (= R-1 framework gap candidate)。

**Severity**: Critical (Rule 1 (1-2) hard violation、Spec stage 完了 block)。

**Fix direction**: matrix table を 70 cells full enumerate (各 cell が独立 row、NA cells は spec-traceable reason 列記)、orthogonality merge cell には source cell # を明示 (Rule 1 (1-4) compliance)。

---

#### C-2: Rule 3 (3-2) NA cell SWC parser empirical 必須の I-226 defer は Spec stage 違反

**Trace**: Rule 3 (3-2): "**NA cell として記載する前に** `crate::parser::parse_typescript()` を直接呼び実行し、SWC が `Err` を返す or 期待 AST shape を構築しない事を **empirical lock-in test** で verify"。

PRD doc TS-2 は "NA cells 6/7/8 SWC parser empirical lock-in defer to I-226" — **NA 記載 + empirical defer = Rule 3 (3-2) hard violation**。

**SWC parser empirical は test harness ESM 改修と orthogonal**: SWC parser test は `crate::parser::parse_typescript()` 直接呼び (Rust unit test) で実施可能、tsx runtime に依存しない。「Axis C cohesive batch」を理由に I-226 へ defer する正当性なし。

**Severity**: Critical (Rule 3 (3-2) hard violation、Spec stage 完了 block、ideal-implementation-primacy 違反: 「実装コストが高い / 影響範囲が広い」を理由に structural verification を defer = 妥協)。

**Fix direction**: 本 PRD spec stage 内で `tests/swc_parser_top_level_await_test.rs` (or 該当 path) で NA cells 6/7/8 用 SWC parser empirical lock-in test 作成。具体的 test:
- `parse_typescript("function f() { } // no exec\nawait x;")` → SWC parser が `Err` or 期待 AST shape 構築せず verify
- 同 fixture の variations で NA reasoning 補強

---

#### C-3: Cell 5 fixture content が cell spec (A0 = no execution) に違反

**Trace**:
- Cell 5 spec: `A0 (declarations only) + B4 (__ts_main collision) + C0`
- Cell 5 fixture (`tests/e2e/scripts/i-224/cell-05-ts-main-collision-no-exec.ts`):
  ```ts
  function __ts_main(): void { console.log("user defined __ts_main"); }
  __ts_main();
  ```

`__ts_main();` は **top-level Stmt::Expr** = Axis A1。fixture content が **A1 + B4 = cell 13 と同形**、cell 5 (A0 + B4) を test していない。

PRD Oracle Observations Cell 5 entry: "stdout=`__ts_main\n`" は call による output、A0 (no execution) なら stdout=(empty) が期待。

**Severity**: Critical (Rule 5 (5-1) "fixture 自体の正当性 verify" + Rule 6 (6-3) "matrix Scope 列値 ↔ fixture content consistency" 違反、cell 5 の test coverage が**実際には不在**)。

**Fix direction**: cell-05 fixture を以下に修正:
```ts
function __ts_main(): void { console.log("user defined __ts_main"); }
// pure library form、no execution stmt
```
Oracle observation も再 record (期待: stdout=(empty)、tsx exit_code=0、Rust 側 expected = Tier 2 honest error reject)。

---

#### C-4: Cell 27 が A5 sub-cells (Empty + Debugger) を 1 row merge = Rule 1 (1-2) 違反

**Trace**: matrix cell 27:
> | 27 | A5 (Stmt::Empty / Stmt::Debugger) | B0 | C0 | Stmt::Empty: skip silently; Stmt::Debugger: ... | Stmt::Empty ✓、Stmt::Debugger Tier 2 honest reclassify | **本 PRD scope** |

Stmt::Empty と Stmt::Debugger は **異なる ideal output** + **異なる判定** (✓ vs Tier 2 reclassify) を持つ別 cell。1 row merge は Rule 1 (1-2) の "(各別 cell)" placeholder pattern と等価違反。

**Severity**: Critical (Rule 1 (1-2) hard violation)。

**Fix direction**: matrix を以下に分割:
- Cell 27a: A5a (Stmt::Empty) + B0 + C0 → ✓ silent skip (regression lock-in)
- Cell 27b: A5b (Stmt::Debugger) + B0 + C0 → ✗ Tier 2 honest reclassify (本 PRD scope)

加えて Axis A 定義に sub-axis A5a / A5b 明示。

---

### 2.2 High Findings (Spec stage approval 前 fix 必須、8 件)

#### H-1: Rule 1 (1-4) Axis B orthogonality merge legitimacy 構造的 verify 不足

**Trace**: Axis B B1 = "function decl / const arrow / const fn expr 統合" claim。これらは **異なる SWC AST shape** を持つ:
- `function main()` → `Decl::Fn { fn_decl }`
- `const main = () => {}` → `Decl::Var { decl: VarDecl { decls[0].init: Expr::Arrow } }`
- `const main = function() {}` → `Decl::Var { decl: VarDecl { decls[0].init: Expr::Fn } }`

Rename to `__ts_main` の dispatch logic は形態別に異なる (Item::Fn vs Item::Const + closure)。Rule 1 (1-4) は "merge cell の adjacent text に explicit justification 記載 + referenced source cell の cell # を明示 + Spec-stage structural consistency verify" を要求。

PRD では "B1 = function decl / const arrow / const fn expr 統合 (orthogonality merge 適用済)" の 1 行のみ。**3 forms の rename 後 IR shape が identical であることの structural verify が不在**。

**Severity**: High (Rule 1 (1-4) 部分違反、Implementation stage で 3 forms detection logic divergence が顕在化する risk)。

**Fix direction**:
- Axis B 定義を明確化: B1a (function decl) / B1b (const arrow) / B1c (const fn expr) sub-cells に分離、各々の rename 後 IR shape を spec
- または "orthogonality merge legitimacy" の structural verify を spec stage で実施 (3 forms 共通の rename target が `Item::Fn` かどうか empirical verify)

---

#### H-2: Cells 14-18 / 30 Out of Scope rationale が Rule 12 (e-3) Permitted reasons に該当しない (= **user 判断必要な根本問題**)

**Trace**: Rule 12 (e-3): "Permitted reasons - infra で AST input dimension irrelevant / refactor で機能 emission decision なし / pure doc 改修"。Prohibited reasons - 「scope 小」/「light spec」/「pragmatic」/「~LOC」/「短時間」/「manageable」/「effort 大」/「実装 trivial」/「quick」/「easy」/「simple」。

PRD cells 14-18/30 Out of Scope rationale: "test harness limitation: tsx + cjs does not support top-level await per ESM proposal, requires separate PRD = harness ESM upgrade + top-level await Tier 1"。

これは Permitted reasons いずれにも該当せず、かつ Prohibited keywords にも明示されていない (= grey zone)。**「test harness 改修 prerequisite」は本質的に「実装範囲が広い (test infra 跨ぎ)」と等価**で、Prohibited reasons の精神に違反する余地あり。

**Severity**: High (Rule 12 (e-3) gray zone violation、ideal-implementation-primacy 観点で要 review)。

**Fix direction (3 options、user 判断必要、第三者 review 推奨 = Option β)**:

##### Option α: 案 β chain 維持、cells 14-18/30 を I-226 defer (現状)、Rule 12 (e-3) gray zone 許容
- **Pros**: 現状の plan.md chain そのまま継続可能、scope 小、I-226 起票済 (TODO 内 詳細 entry 完成)
- **Cons**: Rule 12 (e-3) gray zone 違反の余地、ideal-implementation-primacy 観点で「test harness limitation」を defer 理由とすることが「実装範囲が広い」と等価で Prohibited reasons の精神に違反、第三者 review で structural compromise と判定

##### Option β (第三者 review 推奨): B2 + I-226 cohesive batch 化 = test harness ESM 改修を本 PRD scope に integrate
- **Pros**: ideal-implementation-primacy 整合、1 PRD = 1 architectural concern = "Top-level executable script form の Rust emission strategy + verify infrastructure" として cohesive、Axis C 全 cells を本 PRD で empirical verify 可能
- **Cons**: scope 拡張 (= test harness ESM upgrade を含む)、I-226 起票取り消し or B2 と統合、Implementation stage T1-T6 + 新 task (= test harness ESM upgrade) 追加で task count 増加 (~+3 tasks 想定)、cohesive batch 化の architectural decomposition が必要 (test harness 改修 + transpiler 改修 を 1 PRD で treat する logical structure)
- **具体的 scope 拡張内容**:
  - `scripts/observe-tsc.sh` の tsx invocation を ESM mode に upgrade (`.mts` 拡張 / tsx ESM CLI option / temp dir に `package.json {"type": "module"}` 配置)
  - `tests/e2e_test.rs` runner の ESM-mode runner template
  - `tests/e2e/rust-runner/Cargo.toml` の tokio runtime 依存追加
  - Top-level await emission synthesis (= `#[tokio::main] async fn main()` で top-level await capture into fn main body)

##### Option γ: framework rule update で "test harness limitation" を Permitted reasons に追加
- **Pros**: scope 拡張なし、framework rule level の structural integration、I-D scope creep を許容
- **Cons**: Rule 12 (e-3) Permitted reasons の precedent が新 reason class を許容することで Prohibited reasons との boundary が曖昧化、framework integrity 低下、futureの PRD で "test harness limitation" 等の reason を流用される risk
- **具体的 framework update**:
  - `.claude/rules/spec-stage-adversarial-checklist.md` Rule 12 (e-3) Permitted reasons list に "test harness limitation requiring separate infrastructure PRD" 追加
  - `audit-prd-rule10-compliance.py` の Permitted reasons regex pattern 拡張

##### 推奨判断 (第三者 review)
**Option β (cohesive batch)**。理由:
- ideal-implementation-primacy 観点で「test harness limitation を理由に scope を分離」は **「実装範囲が広い」を理由にした defer = compromise** であり、最も理想的な path は B2 architectural concern の **完全 verify mechanism** を含めた cohesive 解決
- Option γ は framework rule の boundary を弱める risk があり、futureの PRD spec stage で "test harness limitation" 等の流用が増加する pattern → framework integrity 維持の観点で不適切
- Option α は現状維持だが、第三者 review で発見された compromise を放置することになり ideal-implementation-primacy 違反

##### Option β 採用時の plan.md chain 影響
- 現状: I-224 → I-225 → I-162 → I-205 T14-T16 → I-D → I-177 → ...
- Option β 適用後: I-224 (拡張済 = B2 + I-226 統合) → I-225 → I-162 → I-205 T14-T16 → I-D → I-177 → ...
- I-226 entry を TODO から削除 (= I-224 cohesive batch に統合)
- I-226 references を plan.md chain section から削除

##### Option β 採用時の B2 PRD scope 拡張内容 (詳細)
- Axis C C1 全 cells (14-18/30) を **In Scope** に re-classify
- NA cells 6/7/8 SWC parser empirical lock-in を本 PRD scope (= C-2 fix と統合)
- Spec Stage Tasks に新規追加: TS-5 (test harness ESM upgrade)、TS-6 (top-level await Tier 1 synthesis spec)
- Implementation Stage Tasks に新規追加: T7 (`scripts/observe-tsc.sh` ESM upgrade)、T8 (`#[tokio::main]` synthesis 拡張で top-level await capture)、T9 (Axis C1 cells e2e fixture green-ify)

---

#### H-3: Cells 21-24 Scope description "+ I-016 prerequisite chain" が Design と矛盾

**Trace**:
- matrix cell 21 Scope: "本 PRD scope (capture mechanism) + I-016 (init 変換) prerequisite chain"
- Design #3: "A3 (Decl::Var with side-effect init): convert_stmt → IR Stmt::Let { name, init } → push to `main_stmts`"

Design #3 によると executable mode では `convert_var_decl_module_level` (= I-016 owner) を bypass、`convert_stmt` 経由で fn main body capture。**I-016 prerequisite ではない**。

しかし matrix Scope は "+ I-016 prerequisite chain" と claim、Design と矛盾。

**Severity**: High (Rule 6 (6-1) "Matrix Ideal output ↔ Design token-level 一致" 違反、author 自身の認識混乱を示唆)。

**Fix direction**: cells 21-24 Scope 列値を "本 PRD scope (executable mode で fn main body capture path、library mode の I-016 path とは別 dispatch)" に明確化。

---

#### H-4: Design #2 dispatch tree が Out of Scope cells を内包

**Trace**: PRD Design #2 dispatch tree:
```
(true, UserMain::None, true) => synthesize async fn main with #[tokio::main] (cell 14/17)   ← cells 14, 17 Out of Scope
(true, UserMain::Fn { is_async: true }, _) => ... (cell 11/23/30)   ← cell 30 Out of Scope
(true, UserMain::NonFn, _) => ... (cell 12/17/24)   ← cell 17 Out of Scope
(true, UserMain::Fn { is_async: false }, true) => ... (cell 15)   ← cell 15 Out of Scope
```

Out of Scope cells (14/15/16/17/30) が dispatch tree に enumerate されたまま。**Design = 本 PRD で実装する dispatch logic** であるべきで、Out of Scope cells を含めるのは矛盾。

**Severity**: High (Rule 6 (6-1) "Design token-level 一致" 違反、Implementation stage で confusion source)。

**Fix direction**: Design #2 dispatch tree から Out of Scope cells を削除 + 各 leaf に "Out of Scope = I-226 defer" 明示 (= 本 PRD で dispatch しない leaves を separate enumerate)。**Option β 採用時は Out of Scope cells が In Scope に migration するため自動 fix**。

---

#### H-5: INV-2 verification scope に Out of Scope cells 15/16 が残存

**Trace**: INV-2 (c) Verification method: "Cell 10/11/15/16/22/23 fixture で user `main()` call site が `__ts_main()` に substitute されることを fixture probe + IR token-level test で verify"

cells 15/16 は Out of Scope = I-226 defer。INV-2 verification が本 PRD scope cells のみ (10/11/22/23/29/31) で完結すべき。

**Severity**: High (Rule 8 (b)(c) "Verification method ↔ in-scope cells consistency" 違反)。

**Fix direction**: INV-2 (c) を "Cell 10/11/22/23/29/31 fixture で..." に update。**Option β 採用時は cells 15/16 が In Scope に migration するため自動 fix**。

---

#### H-6: `convert_var_decl_module_level` の dual-path dispatch (Library/Executable mode) が Design 不在

**Trace**:
- Library mode: top-level Decl::Var → `convert_var_decl_module_level` (`arrow_fns.rs:15`) で top-level const emit
- Executable mode (本 PRD scope): top-level Decl::Var → `convert_stmt` 経由で fn main body capture

Design #3 では executable mode のみ言及、library mode の path は "既存 path 維持" とのみ記載 (Impact Area)。**dispatch decision (どの mode に該当するかの判定) の logic が Design に不在**。

例: `class C { _n = 0; } const c = new C(); console.log("hi");` (executable mode trigger by console.log) では `const c = new C();` も fn main 内 capture。
一方 `class C { _n = 0; } const c = new C(); export {};` (library mode、no execution) では `const c = new C();` どうする? Library mode で `convert_var_decl_module_level` 経由 → I-016 silent skip 残存?

**Severity**: High (Design integrity gap、Implementation stage で behavior 未定 ambiguous)。

**Fix direction**: Design に "Decl::Var dispatch decision tree" 追加:
- if executable_mode: `convert_stmt` 経由 fn main body capture (cell 21-24 path)
- else (library mode): `convert_var_decl_module_level` 経由 (existing path、I-016 owner)

---

#### H-7: Cell 31 sub-axis "multiple main() calls" が Axis 定義に不在

**Trace**: Cell 31 description: "A1 with multiple `main()` calls (e.g., `main(); main();`)". Axis A 定義には "single call vs multiple call" の sub-axis 不在。Cell 10 は単一 `main();` call、cell 31 は multiple call で ideal output dispatch (= __ts_main() 多重 substitute) が異なる。

INV-2 verification の sub-case として独立 cell 化されているが、Axis 定義に欠落 → Rule 8 (8-c) Axis enumeration completeness 違反。

**Severity**: High (Rule 1 + Rule 8 axis 定義の structural inconsistency)。

**Fix direction (2 options)**:
- (Fix A) Axis A1 を sub-axis A1a (single call) / A1b (multiple call) に分離
- (Fix B) Cell 31 を INV-2 sub-case として独立 cell から削除、INV-2 verification method に "multiple call substitution sub-case (= cell 10 のbase + multi-call extension)" 明記

---

#### H-8: T6 task が複数 architectural concerns を mix

**Trace**: T6:
> **Work**: I-154 namespace doc 追記 + `scripts/audit-no-pub-fn-init.sh` を CI integrate + `audit-prd-rule10-compliance.py` の Empirical file path verify rule reinforce

3 つの異なる architectural concerns:
1. I-154 namespace doc update (B2 直接関連)
2. `audit-no-pub-fn-init.sh` 作成 + CI integrate (B2 直接関連)
3. `audit-prd-rule10-compliance.py` reinforce (= **framework rule integration、I-D scope**)

**Severity**: High (1 task = 1 concern boundary 違反、I-D scope creep)。

**Fix direction**: T6 を 2 task に分割:
- T6a: I-154 doc update + audit-no-pub-fn-init.sh 作成 + CI integrate (B2 scope)
- (削除) audit-prd-rule10-compliance.py reinforce → I-D scope へ migrate

---

### 2.3 Medium Findings (Implementation stage で fix 可能、ただし spec stage で明示推奨、4 件)

#### M-1: Cells 6/7/8 NA reasoning が技術的に imprecise

**Trace**: PRD Cell 6 NA reason: "top-level await は execution context 内のみ valid、本 cell A0 は execution 不在のため await 配置不能"。

実 SWC + tsc 挙動 (empirical 2026-05-01):
> error TS1375: 'await' expressions are only allowed at the top level of a file when that file is a module, but this file has no imports or exports

NA root cause = **TS module declaration 不在** (TS spec)、**not** "execution context 不在"。Axis A0 が "imports / exports のみ" を含む場合、module declaration あり → top-await が **AST shape 上 A1 (Stmt::Expr Await) or A3 (Decl::Var with await init) に該当**するため A0 + C1 は AST 構造的に unreachable。

**Severity**: Medium (Rule 3 (3-1) NA spec-traceable wording の precision)。

**Fix direction**: NA reason wording を "Axis C C1 (top-level await) は AST shape 上 Stmt::Expr (Expr::Await) or Decl::Var (with await init) を要求 = Axis A1 / A3 に該当、Axis A0 (= no execution stmt) と AST 構造的に mutually exclusive"。

---

#### M-2: Module export 有無が axis dimension 不在

**Trace**: TS module で `export function f() { ... }` (library export) + `console.log("hi");` (executable side effect) は両立可能。本 PRD では axis dimension 不在のため、ideal output 未確定。

Cell 9 (A1 + B0) では `export {}` 不在で synthesize fn main、library export 併存 case の dispatch は不明。Rust crate 構造 (binary vs library) に影響する architectural 判断点。

**Severity**: Medium (Rule 10 default axis check (g) "outer emission context" の sub-dimension 漏れ)。

**Fix direction**: Axis E "Module export presence" 追加 (E0: no export / E1: export 存在)、各 cell 影響を明示。

---

#### M-3: Audit script `scripts/audit-no-pub-fn-init.sh` 作成 task が implicit

**Trace**: PRD は本 script を Quality Gates + INV-4 verification で使用と claim。T6 は "CI integrate" と書くが **作成 task が unspecified**。T4 (`pub fn init` 廃止) に implicit 含まれる可能性あり、ただし明示なし。

**Severity**: Medium (Rule 9 (b) Spec → Impl Mapping 部分 violation、Implementation stage で「あるべき script」と「実在 script」の gap)。

**Fix direction**: T4 completion criteria に "scripts/audit-no-pub-fn-init.sh 新規作成" 明示、または独立 task T4a として分離。

---

#### M-4: Test Plan で regression lock-in cells 1/2/3/4/19/20 の test entry 不在

**Trace**: PRD Test Plan E2E section: "tests/e2e_test.rs: per-cell test fn entries (`run_cell_e2e_test("i-224", "cell-NN-*")`)".

20 fixtures 全てに `run_cell_e2e_test` test entry 必要。regression lock-in cells (1/2/3/4/19/20) の test entry が「✓ regression lock-in test 必須」と claim されているが、test fn naming pattern + entry 一覧不在。

**Severity**: Medium (test plan vagueness、Implementation stage T5 の completion criteria precision 不足)。

**Fix direction**: Test Plan に full e2e test fn list 追加 (20 entries)、test fn naming pattern 統一 (= e.g., `test_e2e_cell_i224_NN_<semantic_name>`)。

---

### 2.4 Review Insights (Action item は別 PRD 候補 or future PRD 起票、5 件)

#### R-1: `audit-prd-rule10-compliance.py` matrix cell completeness 検出 mechanism 不在

C-1 (Cartesian product 完全 enumerate 違反) を audit script が検出しなかった。**framework gap signal** として `verify_cartesian_product_completeness` function 追加候補 (Axis 定義から expected cells 数を計算 + matrix table の cell # と diff)。

**Action item**: 別 PRD I-D scope に integrate (framework rule integration)。

---

#### R-2: `pub fn init` 廃止の external API breaking change 影響 audit

INV-4 で `pub fn init` 廃止 = 全 transpile output から `pub fn init` 識別子排除 = **breaking change**。external user / test が `init()` を call する case の audit 不在。

empirical verify needed: codebase + Hono + 既存 e2e test での `init()` call site grep。

**Action item**: 本 PRD spec stage に追加 task として "`pub fn init` 廃止 impact audit" を integrate or 別 PRD 起票候補。

---

#### R-3: TypeResolver impact assessment 不在

PRD は Transformer 改修 focus、**TypeResolver layer の impact が unspecified**。fn main synthesis + user main rename が TypeResolver の type resolution flow に影響しない確証なし。

**Action item**: Design に "TypeResolver unaffected, rationale: ..." 明示 or Implementation stage T1 着手前の empirical probe で confirm。

---

#### R-4: `__ts_main` reservation の **既存 user code 衝突 audit** 不在

INV-5 (`__ts_main` collision detection) は本 PRD で実装する mechanism。しかし **既存 codebase / Hono / 既存 e2e test に `__ts_main` 識別子 user code が存在しないかの empirical audit 不在**。reachable なら本 PRD で Tier 2 reject すべき新 errors が surface する risk。

**Action item**: codebase + Hono grep `__ts_main` で 0 hits を verify (Hono bench Tier-transition compliance verify の前提)。

---

#### R-5: Multiple PRD spec stage interleaving rule の framework 化

本 PRD spec stage 中に I-226 起票 → plan.md update → 後続 PRD chain 整合 update。**multi-PRD interleaving の formal procedure 不在**。

**Action item**: framework v2-2 candidate (= I-D scope) として `spec-first-prd.md` に "Spec stage 中の Spec gap 由来 PRD 起票" 手順追加。

---

## 3. Defect Classification Summary (5-category)

| Category | Count | 内訳 | Action |
|----------|-------|------|--------|
| Grammar gap | 0 | (該当なし) | (なし) |
| Oracle gap | 0 | (該当なし) | (なし) |
| **Spec gap** | **8** | C-1, C-2, C-3, C-4, H-3, H-4, H-5, H-6 | C-1/C-2 は audit script 強化 candidate (R-1)、C-3 は fixture 修正、C-4 は matrix split、H-3/H-5 は wording fix、H-4/H-6 は Design specification 補完 |
| Implementation gap | 0 (Implementation 未着手) | (該当なし) | (なし) |
| **Review insight** | **5** | H-1, H-2, H-7, H-8, M-1〜M-4, R-1〜R-5 | 各 Action item に従い対処 |

**特に重要 (= framework 失敗 signal)**:
- **C-1 (Cartesian product 完全 enumerate 違反)**: audit script が検出できなかった = framework gap (R-1)、`audit-prd-rule10-compliance.py` に `verify_cartesian_product_completeness` 追加候補
- **C-2 (Rule 3 (3-2) defer)**: spec stage 中の "I-226 cohesive batch" rationalization が Rule 3 (3-2) hard requirement を回避する compromise pattern、ideal-implementation-primacy 違反
- **H-2 (Out of Scope rationale)**: "test harness limitation" は Rule 12 (e-3) Permitted reasons 不在、cohesive batch 統合か framework rule update が必要

---

## 4. 次の Concrete Actions (Spec stage iteration v3)

### 4.1 user 判断必要点 (= H-2)

**Q1**: Option α (現状維持) / Option β (cohesive batch、第三者 review 推奨) / Option γ (framework rule update) のいずれを採用?

判断材料:
- Option β は scope 拡張 (= test harness ESM upgrade を本 PRD に integrate)、ideal-implementation-primacy 整合最良
- Option α は scope 維持、Rule 12 (e-3) gray zone 許容
- Option γ は framework rule level の change、boundary 弱化 risk

### 4.2 Option β 採用時の Spec stage iteration v3 work breakdown

**Critical fixes (4 件、必須)**:
1. **C-1 fix**: matrix を 70 cells full enumerate (各 cell 独立 row、orthogonality merge cell に source cell # 明示)
2. **C-2 fix**: NA cells 6/7/8 用 SWC parser empirical lock-in test を本 PRD scope で作成 (Option β 採用時は cells 14-18/30 + 6/7/8 全て In Scope に migration、SWC parser empirical も本 PRD で実施)
3. **C-3 fix**: cell-05 fixture 修正 (= `__ts_main();` call 削除、A0 形に整合)、Oracle observation 再 record
4. **C-4 fix**: matrix cell 27 を 27a (Empty) + 27b (Debugger) に分離、Axis A 定義に sub-axis A5a / A5b 明示

**High fixes (8 件、必須)**:
5. **H-1 fix**: Axis B B1 を sub-axes B1a/B1b/B1c に分離 or orthogonality merge structural verify を spec stage で実施
6. **H-2 fix**: Option β 採用時は cells 14-18/30 + 6/7/8 を In Scope に migration、I-226 entry を TODO から削除、plan.md chain から I-226 references 削除、PRD scope 拡張 (TS-5 / TS-6 / T7 / T8 / T9 等の新 task 追加)
7. **H-3 fix**: cells 21-24 Scope 列値を "本 PRD scope (executable mode で fn main body capture path、library mode の I-016 path とは別 dispatch)" に修正
8. **H-4 fix**: Design #2 dispatch tree を In Scope cells のみに narrow (Option β 採用時は全 cells 対象)、各 leaf に scope annotation
9. **H-5 fix**: INV-2 verification cells を in-scope only に narrow (Option β 採用時は cells 10/11/15/16/22/23/29/31)
10. **H-6 fix**: Design に Decl::Var dual-path dispatch (Library/Executable mode) 追加
11. **H-7 fix**: Cell 31 を Axis A1 sub-axis に integrate (Fix A) or INV-2 sub-case として整合 (Fix B)
12. **H-8 fix**: T6 を T6a (B2 scope) + I-D candidate (移管) に分離

**Medium fixes (4 件、推奨)**:
13. **M-1 fix**: NA cells 6/7/8 wording を "AST shape 上 mutually exclusive" に precision up
14. **M-2 fix**: Axis E "Module export presence" 追加 + 各 cell 影響明示
15. **M-3 fix**: T4 / T5 completion criteria に audit-no-pub-fn-init.sh 作成 task 明示
16. **M-4 fix**: Test Plan に regression lock-in cells (6 件) test fn entries 追加

**Review insights actions (5 件)**:
17. **R-1 action**: I-D scope に `verify_cartesian_product_completeness` 追加 candidate 起票
18. **R-2 action**: 本 PRD spec stage に追加 task "`pub fn init` 廃止 impact audit" を integrate
19. **R-3 action**: Design に "TypeResolver unaffected, rationale: ..." 明示
20. **R-4 action**: codebase + Hono grep `__ts_main` で 0 hits を empirical verify (本 PRD spec stage)
21. **R-5 action**: I-D scope に `spec-first-prd.md` "Spec stage 中の Spec gap 由来 PRD 起票" 手順追加 candidate 起票

### 4.3 iteration v3 完了判定

- iteration v3 完了 = 16 件 fix 全 resolve + 13-rule self-applied verify pass + audit-prd-rule10-compliance.py PASS
- iteration v3 で第三者 review に相当する自己批判検証 + Spec stage approval

---

## 5. 関連 Files (Spec stage v3 で修正対象)

### 5.1 PRD doc
- `backlog/I-224-top-level-fn-main-mechanism.md` (現状 727 lines、iteration v3 で修正、~900-1000 lines 想定)

### 5.2 Test fixtures (現状 20 fixtures、iteration v3 で追加 + cell-05 修正 + Option β 採用時 cells 14-18/30 fixture 復元)
- `tests/e2e/scripts/i-224/cell-NN-*.{ts,expected}` (20 fixtures、cell-05 内容修正必要)
- (Option β 採用時) cells 14-18/30 fixtures 復元 + tsx ESM mode で oracle 再 record
- (新規、TS-2 fix) `tests/swc_parser_top_level_await_test.rs` (NA cells 6/7/8 SWC parser empirical lock-in)

### 5.3 Project files (modify 不要、reference のみ)
- `TODO` (I-224 entry 修正 + Option β 採用時 I-226 entry 削除)
- `plan.md` (iteration v3 status update + Option β 採用時 chain 修正)
- `report/I-224-spec-stage-v3-review-handoff.md` (本 doc、iteration v3 完了時に archive 候補)

### 5.4 Audit scripts
- `scripts/audit-prd-rule10-compliance.py` (iteration v3 で PASS 維持必要)
- `scripts/audit-ast-variant-coverage.py` (iteration v3 で PASS 維持必要)

---

## 6. /start 再開時の手順

`/start` 実行時に本 doc を反映するための手順:

### Step 1: plan.md + 本 handoff doc 確認
- `plan.md` "進行中作業" section で B2 PRD spec stage v3 が次着手 task と記載されている
- 本 doc (`report/I-224-spec-stage-v3-review-handoff.md`) を読んで議論経緯を把握

### Step 2: 第三者 review 結果 (16 findings + 5 review insights) を理解
- 本 doc Section 2 で各 finding の詳細 (trace + severity + fix direction) を確認
- 特に H-2 の Option α / β / γ 設計判断点を理解

### Step 3: H-2 user 判断確認 (前 session で未判断の場合)
- 本 doc Section 4.1 の Q1 を user に提示
- user 判断 (= Option β 推奨) を仰ぐ

### Step 4: iteration v3 着手
- Option β 採用時: Section 4.2 の 16 件 fix + 5 件 action items を順次 resolve
- Option α 採用時: H-2 fix 不要、残 15 件 fix
- Option γ 採用時: H-2 fix を framework rule update に migrate、残 15 件 fix + framework rule update

### Step 5: iteration v3 self-review + audit verify
- 13-rule self-applied verify pass
- `audit-prd-rule10-compliance.py backlog/I-224-top-level-fn-main-mechanism.md` PASS
- Critical = 0 + High = 0 + Medium = 0 で Spec stage 完了判定 → user 承認 → Implementation stage T1-T6 (Option β なら T1-T9) 移行

---

## 7. References

### 7.1 Rules
- `.claude/rules/spec-first-prd.md` (matrix-driven PRD lifecycle)
- `.claude/rules/spec-stage-adversarial-checklist.md` (13-rule checklist、本 review の base)
- `.claude/rules/check-job-review-layers.md` (4-layer review framework、Implementation stage 用)
- `.claude/rules/post-implementation-defect-classification.md` (5-category)
- `.claude/rules/problem-space-analysis.md` (matrix construction)
- `.claude/rules/ideal-implementation-primacy.md` (最上位原則)
- `.claude/rules/conversion-correctness-priority.md` (Tier 分類)
- `.claude/rules/prd-completion.md` (Tier-transition compliance)
- `.claude/rules/type-fallback-safety.md` (本 PRD は N/A)
- `.claude/rules/testing.md` (test design techniques)
- `.claude/rules/design-integrity.md`
- `.claude/rules/pipeline-integrity.md`

### 7.2 Skills / Commands
- `/prd-template` (Spec stage 起動、本 doc Section 1.2 step 5 で invoke 済)
- `/check_job` (本 doc 作成の trigger、第三者 review、Section 2 の findings source)
- `/quality-check` (work 完了前 verification)
- `/start` (session 再開、本 doc を base に discussion 継続)
- `/end` (PRD close 時の trigger)

### 7.3 Audit scripts
- `scripts/audit-prd-rule10-compliance.py` (Rule 4/10/11/12 compliance)
- `scripts/audit-ast-variant-coverage.py` (Rule 11 (d-1) AST variant coverage)
- `scripts/observe-tsc.sh` (Oracle observation、Section 1.2 step 6 で empirical evidence source)
- `scripts/record-cell-oracle.sh` (Oracle .expected 記録)

### 7.4 Related TODO / backlog entries
- `TODO` 内 [I-224] (本 PRD source、Tier 1 ゲートイシュー)
- `TODO` 内 [I-225] (B3 class field literal type inference、本 PRD と peer)
- `TODO` 内 [I-162] (constructor synthesis、本 PRD と peer + cells 21-24 の I-162 prerequisite chain mention)
- `TODO` 内 [I-226] (test harness ESM support + top-level await Tier 1、Option β 採用時に削除予定)
- `TODO` 内 [I-016] (module-level const variant、本 PRD と orthogonal、library mode counterpart)
- `TODO` 内 [I-203] (codebase-wide AST exhaustiveness compliance、本 PRD scope 外 `_` arm violations の defer 先)
- `TODO` 内 [I-154] (`__ts_` namespace reservation、本 PRD で `__ts_main` 拡張)

---

## 8. 終了条件 (本 handoff doc が archive 可能になる条件)

- B2 PRD spec stage iteration v3 完了 (= 16 件 fix 全 resolve + 5 件 action items 全 record)
- 13-rule self-applied verify pass + 第三者 review 風 critical re-verify で Critical = 0 + High = 0
- audit-prd-rule10-compliance.py PASS 維持
- user 承認 → Implementation stage T1-T6 (Option β なら T1-T9) 移行

archive 候補先: `report/archive/I-224-spec-stage-v3-review-handoff-archived.md` (iteration v3 完了時に move)
