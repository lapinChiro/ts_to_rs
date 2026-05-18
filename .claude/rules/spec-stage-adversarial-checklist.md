---
paths:
  - "backlog/**/*.md"
  - ".claude/rules/**/*.md"
  - ".claude/skills/**/SKILL.md"
  - ".claude/commands/**/*.md"
  - "doc/handoff/**/*.md"
---

# Spec-Stage Adversarial Review Checklist

## When to Apply

Matrix-driven PRD (`spec-first-prd.md` 適用対象) の **Spec stage 完了時**、Implementation stage への移行可否を判定する自己 review として全項目を verification する。1 つでも未達の項目があれば Implementation stage への移行は不可。

## Core Principle

> **Spec stage 完了の合否を「artifact が揃った」ではなく「全 13 項目の adversarial check が
> pass した」で判定する。各 rule は empirical defect chain から derive されたもので、
> rule を逐次 verification することで同 root cause の defect 再発を構造的に防ぐ。**

## Checklist (13 rule)

```markdown
## Spec-Stage Review Checklist

- [ ] **Matrix completeness + abbreviation prohibition**:
      - **(1-1)** 全セルに ideal output が記載されている (空欄 / TBD なし)
      - **(1-2)** Cartesian product 完全 enumerate 必須、**abbreviation pattern 全面禁止**:
        - matrix table 内 `...` (omission ellipsis) 禁止
        - 連番 row range (`| N-M | A | ... |` 等の grouping) 禁止 — 各 cell 独立 row として記載
        - `representative` / `representative cell のみ` / `代表的` / `省略` / `abbreviated` 等の wording 禁止
        - `(各別 cell)` / `(同上)` / `varies` / `(... と同 logic)` 等の placeholder 禁止
      - **(1-3)** Audit verify mechanism: `scripts/audit-prd-rule10-compliance.py` で
        Problem Space matrix table を parse し abbreviation pattern detection
        (上記禁止 list の regex match) で自動検出、merge gate
      - **(1-4) Orthogonality merge legitimacy + Spec-stage structural verify**:
        `D 全` / `B 全` / `Bn-Bm` 等 axis-merge wording は Rule 10 Step 2
        orthogonality merge として **legitimate** (dispatch logic 同一の場合のみ)。
        ただし以下 3 条件 (1-4-a)/(1-4-b)/(1-4-c) を **全 verify** した場合のみ
        Rule 1 (1-2) compliant と認める。defer は禁止 (= Spec stage で全 verify):
        - **(1-4-a) Orthogonality verification statement**: merge cell の adjacent
          text に "orthogonality-equivalent" / "orthogonality-equivalent dispatch" /
          "Rule 10 Step 2 orthogonality merge" 等 explicit justification 記載 +
          **referenced source cell の cell # を明示** (例: "cells N-M と
          orthogonality-equivalent")
        - **(1-4-b) Spec-stage structural consistency verify**:
          referenced source cell が **matrix 内に存在** + 本 cell と source cell の
          Scope 列値が **一致 or compatible** であることを audit script で structural
          verify。本 verify は Spec stage で完了必須 (Implementation stage defer は
          pragmatic compromise = framework integrity 損失 のため禁止)
        - **(1-4-c) Spec-stage referenced cell symmetry probe**: merge cell が claim
          する dispatch (例: "Tier 2 honest error reclassify") と referenced source
          cell の dispatch が **symmetric (token-level prefix一致)** であることを
          audit script で verify (例: 複数 cell が同一 "<reclassify reason>" を claim
          する場合、各 cell の dispatch wording が token-level prefix で一致することを
          verify)
        Audit script `audit-prd-rule10-compliance.py` で (1-4-b)(1-4-c) を auto verify
        (`verify_orthogonality_merge_consistency` function)。
- [ ] **Oracle grounding + PRD doc embed mandatory**:
      - **(2-1)** ✗ / 要調査 セルの ideal output が tsc observation log と
        cross-reference されている
      - **(2-2)** PRD doc 内に **`## Oracle Observations` section を独立 hard-code**、
        各 ✗ / 要調査 cell について以下 4 項目記載必須:
        - **TS fixture path**: `tests/e2e/scripts/<prd-id>/<cell-id>.ts` 等
        - **tsc / tsx output**: stdout / stderr / exit_code 全 embed
          (`scripts/observe-tsc.sh` 出力転記)
        - **Cell number reference**: matrix table の cell # と 1-to-1 link
        - **Ideal output rationale**: tsc 出力から Rust ideal output を derive する
          論理 (preserve / reject / equivalent / Tier 2 honest error)
      - **(2-3)** Audit verify mechanism: `audit-prd-rule10-compliance.py` で
        `## Oracle Observations` section 不在 + matrix に ✗/要調査 cell あれば
        audit fail
- [ ] **NA justification + SWC parser empirical observation 必須**:
      - **(3-1)** NA セルの理由が spec-traceable (TS spec 上 syntax error / grammar
        constraint / Rust type system 構造的制約 等) であり、「稀」「多分」「頻度低」等の
        曖昧理由がない
      - **(3-2)** TS spec で "syntax error" / "parse error" / "rejected" と documented
        されていても、**SWC parser が actual に reject するかは empirical 確認必須**
        (= TS spec ≠ SWC parser behavior、SWC parser は寛容 parsing で TS spec 違反
        syntax を AST に含める ケースあり、例: object literal context での `{ x = expr }`
        (= `Prop::Assign`) は TS spec 上 parse error documented だが SWC parser は
        AST に含めて accept)。NA cell として記載する前に
        `crate::parser::parse_typescript()` を直接呼び実行し、SWC が `Err` を返す or
        期待 AST shape を構築しない事を **empirical lock-in test**
        (`tests/swc_parser_*_test.rs` 等の structural placement) で verify
      - **(3-3)** SWC parser が accept する場合 = NA cell ではなく **Tier 2 honest
        error** に reclassify (= `UnsupportedSyntaxError` 経由 explicit reject、
        `ideal-implementation-primacy.md` Tier 1 silent semantic change リスクを排除、
        `unreachable!()` macro の precondition violation を構造的に防止)
- [ ] **Grammar consistency + doc-first dependency order の structural enforcement**:
      - **(4-1)** matrix に reference doc に未記載の variant が存在しない
        (存在すれば reference doc を先に更新)
      - **(4-2)** PRD 内 doc update task (= `doc/grammar/ast-variants.md` /
        `doc/grammar/rust-type-variants.md` / `doc/grammar/emission-contexts.md` /
        その他 reference doc 更新 task) は **code 改修 task** (= TypeResolver /
        Transformer / Generator 等の `src/` 配下 Rust source 改修 task) の
        **prerequisite** として位置付ける必須 dependency 制約。code 改修が doc を
        ground truth として参照する **単方向 dependency** (= doc-first)、doc を
        code 後に sync する **逆方向 dependency** は **Rule 4 違反** (= single
        source of truth の structural 違反、Rule 11 の `doc/grammar/ast-variants.md`
        single source of truth 原則と整合)。
      - **(4-3)** Verification mechanism: `scripts/audit-prd-rule10-compliance.py`
        で PRD doc Task List section を parse し、以下を auto verify:
        - PRD doc 内 task の Depends on / Prerequisites を抽出
        - doc update task ID (= `ast-variants.md` / 関連 reference doc 更新を含む
          task) を identify
        - code 改修 task ID (= `src/` 配下の Rust source 改修を含む task) を
          identify
        - 各 code 改修 task の Prerequisites に doc update task ID が存在することを
          check (= doc-first verify、人手判断介在排除)
        - 不在時 audit fail (CI fail = PRD merge 不能)
- [ ] **E2E readiness + Stage tasks separation**:
      - **(5-1)** 各 ✗ cell に対応する E2E fixture が `tests/e2e/scripts/<prd-id>/cell-NN-*.ts`
        (red 状態) で準備済 (Spec stage 完了時点)
      - **(5-2)** PRD doc Task List section は Stage 1 / Stage 2 で **2-section 分離 hard-code**:
        - `## Spec Stage Tasks`: Stage 1 artifacts 完成 task (matrix construction /
          oracle observation / fixture creation / SWC parser empirical lock-in /
          impact area audit findings record)
        - `## Implementation Stage Tasks`: Stage 2 code change task (`src/` 修正、
          unit test / integration test / E2E green-ify、dispatch logic 拡張、etc.)
      - **(5-3)** Spec stage 完了 = `## Spec Stage Tasks` 全完了 + 13-rule self-applied
        verify pass = Implementation stage 移行可能。Spec stage tasks に code 改修
        (`src/` 修正) を含めること禁止 (= stage boundary 違反)。
      - **(5-4)** Audit verify: matrix-driven PRD で `## Spec Stage Tasks` /
        `## Implementation Stage Tasks` のいずれか不在 → audit fail
      - **(5-5) Fixture tsx runtime empirical observation**: 各 ✗ cell の E2E fixture は
        **fixture 自体の tsx runtime empirical observation** で fixture content 正当性を
        verify する (= `scripts/observe-tsc.sh` で fixture を実行、stdout / stderr / exit_code を
        Spec stage で formal record + Oracle Observations section に embed)。
        test harness 制約 (= cjs vs ESM mode の selection / `--esm` / `--no-auto-main` flag /
        top-level `await` の executable mode requirement 等) を **Spec stage で前倒し検出**
        するため、tsc parse 通過のみではなく runtime probe を mandatory step 化。
        **Recurring problem rationale**: tsc が fixture を parse 通過する一方で tsx runtime
        実行時に harness 制約 (= cjs vs ESM 等) で fail する pattern が、Implementation stage
        T-task 着手後に発覚し Spec stage 逆戻り cost が累積する。Spec stage で fixture
        tsx runtime probe を mandatory 化することで harness 制約由来 defect class を
        structural prevent する (= test harness 制約 を fixture spec の一部に embed)。
- [ ] **Matrix/Design integrity + Scope 3-tier consistency**:
      - **(6-1)** matrix Ideal output 列と Design section emission strategy が
        **token-level に一致**。乖離 1 例でも存在すれば (a) どちらが正規 spec か明記、
        (b) 非正規側を正規側へ updating commit してから checklist 満たしたとみなす。
        Verification: matrix の各 cell について Design section から該当 emission rule を
        引用し side-by-side で diff
      - **(6-2)** PRD doc Scope section は **3-tier 形式 hard-code**:
        - `### In Scope`: 本 PRD で **Tier 1 完全変換** する features
        - `### Out of Scope`: 別 PRD or 永続 unsupported な features
        - `### Tier 2 honest error reclassify`: 本 PRD で **Tier 2 honest error 化**
          する features (= 別 PRD で Tier 1 化候補、orthogonal architectural concern)
      - **(6-3)** matrix Scope 列値は次から択一: `本 PRD` / `別 PRD (I-NNN)` /
        `Tier 2 honest error reclassify (本 PRD)` / `Tier 2 honest error reclassify (別 PRD I-NNN)` /
        `regression lock-in`
      - **(6-4)** Scope section の 3-tier 列挙と matrix Scope 列の cross-reference
        consistency を audit script で verify (Scope 列に "本 PRD" cell が In Scope に
        記述されていること、Tier 2 honest error reclassify cell が同 section に
        記述されていること、等)
      - **(6-5) Dense matrix density limit + spec-table-driven generator recommendation**:
        matrix-driven PRD で **80+ cells × 6+ cross-reference contexts** の dense matrix が
        **manual-tracking density limit** を超える場合、**spec-table-driven generator**
        (= matrix を **single source-of-truth** として他 sections (= Spec→Impl Mapping table /
        Implementation Stage Tasks task list / Test contract path enumeration / Invariants
        cell list 等) を機械的 derive する utility) を使用必須。手動 sync は cross-reference
        defect class (= manual-tracking density limit を超えた状態での反復的 sync drift) を
        累積する pattern が再発するため、density limit を超えた dense matrix では generator
        utility 経由の machine-derived sync が prerequisite。
        **Recurring problem rationale**: dense matrix では手動 cross-reference sync が
        cognitively unscalable で、Iteration の都度 cross-reference defects が新規発生する。
        density limit を numeric criteria (= 80 cells × 6 contexts) で明確化することで
        generator utility 採用判断を spec-traceable に固定し、reviewer の主観判断依存を排除する。
- [ ] **Control-flow exit sub-case completeness**:
      - **(7-1) Dimension trigger**: Matrix cell の dimension に "body shape" /
        "branch shape" が含まれる場合、各 branch の **exit-or-fallthrough 状態** を
        独立次元として enumerate する
      - **(7-2) 4 sub-case 明示記載**: 最低 4 sub-case
        (then_exits × else_exits = T/T, T/F, F/T, F/F) を明示 cell として PRD に記載し、
        各 sub-case に対応する E2E fixture と ideal output を spec する
      - **(7-3) Aggregation expression 禁止**: "any × any" / "either-exits" /
        "regardless of else" 等の集約表現は禁止 (集約は post-implementation の audit で
        defect を hide する)
      - **(7-4) Verification**: matrix table で body / else dimension の cell を抽出し、
        各 cell の row が 4 行に展開されていることを目視確認
- [ ] **Cross-cutting invariant enumeration**: 機能仕様の中に「matrix cell に展開
      できない / 全 cell で同時に成立する必要がある」transversal property が存在しないか
      自問し、存在する場合は PRD に独立 section として `## Invariants` を設けて列挙する。
      - **(8-1) Per-invariant field requirements**: 各 invariant entry について以下
        4 項目を必須記述:
        - (a) **Property statement**: 1 文で書けるレベルの不変条件
              (例: 「TypeResolver の expr_type と IR の Type が同一 span に対し一致」)
        - (b) **Justification**: なぜこの invariant が必要か
              (この invariant 違反でどんな defect class が発生するか)
        - (c) **Verification method**: 実装後に invariant 成立を verify する具体手順
              (probe / test / static analysis のどれを使うか)。**Multi-dispatch flow
              empirical probe coverage** (Rule 8 (c) sub-rule): 対象 PRD の architectural
              mechanism が **複数 dispatch flow** を持つ場合 (例: single-file vs multi-file /
              sync vs async / library vs executable mode 等の orthogonal flow split)、
              **全 flow を prototype probe で empirical cover** することを **Verification method 必須要件** として明示化 (= partial coverage = prototype が single
              flow のみ probe、production T-task で multi-flow を初めて verify する
              pattern を structural prevent)。Single-flow probe で済ます場合は "多 dispatch
              flow 不在" を spec-traceable に明示 (= dispatch flow enumeration の zero
              count を declared state として treat)。
        - (d) **Failure detectability**: invariant 違反が compile error / runtime error /
              silent semantic change のどれで顕在化するか

        候補 invariant カテゴリ (探索 prompt として活用): TypeResolver-IR cohesion /
        並列 emission path symmetry / closure-reassign suppression cohesion /
        scope boundary preservation / mutability propagation。
      - **(8-2) Audit verify mechanism**: matrix-driven PRD で `## Invariants` section
        不在 or 空 (4 項目 (a)(b)(c)(d) のいずれか missing) → audit fail
      - **(8-3) Invariant verification cell coverage double-partition symmetric verify**:
        各 invariant entry の **verification cell list (= invariant が holds する scope を
        指定する cell enumeration)** は **本 PRD scope の Axis X 全 cells** claim と
        **Cartesian product cells** の cross-reference で **exhaustive coverage** を
        auto verify。**library mode vs executable mode** 両 partition (= 機能 emission を
        determine する orthogonal partition、Rule 10 の dispatch flow axis に派生する
        symmetric pair) の coverage gap を syntactic detect する **double-partition
        symmetric verify** を audit script (= `verify_invariant_cell_coverage_double_partition`
        function) で hard-code。**Recurring problem rationale**: invariant verification cell
        list が "library mode `fn main directly emit` cells 漏れ" 等の **partition
        asymmetry** を持つ pattern は manual review で latent 化しやすく (= reviewer が
        single partition のみ confirm して PASS 判定する pattern が再発)、framework rule
        level での Cartesian product cells の cross-reference 必須化が structural
        prevention prerequisite。
- [ ] **Dispatch-arm sub-case alignment**: Matrix の各 type-dimension は、実装側で
      **branch / dispatch / pattern-match を分けるあらゆる sub-classifier** と一対一の
      粒度で enumerate する。具体例: `Named` を単一 cell として記述する代わりに、実装が
      `is_synthetic_union` flag や `is_always_truthy` 判定で dispatch 分岐するなら
      `Named (synthetic union)` / `Named (always-truthy)` / `Named (other)` の 3 cell に
      分割。
      Verification (3 方向の同期):
      - **(9-1)** **Spec → Impl**: PRD 確定後、実装着手前に「実装 file の dispatch / match を
            全 enumerate し、matrix cell と 1-to-1 対応するか」を check
      - **(9-2)** **Impl → Spec**: 実装中に新しい dispatch arm を追加する必要を発見した場合、
            Spec stage に戻って matrix cell を分割する (`spec-first-prd.md` の
            「Spec への逆戻り」手順を発動)
      - **(9-3)** **Field-addition symmetric conversion site audit**:
            PRD task が **IR struct field 追加** (例: `MethodSignature.kind` /
            `TsMethodInfo.kind` 等) を含む場合、その field を **produce する全 site**
            (constructor / builder /
            converter / external_types registration) と **consume する全 site** (reader /
            dispatcher / serializer) を **Spec stage で proactive enumerate** し、各 site の
            責務 (`hardcode default` 妥当 / `propagate from source` 必須 / `propagate from
            m.X` 必須) を意識的判断 + matrix table に記載必須。
            Verification mechanism:
            - **(9-3-1) Pre-implementation symmetric audit**: 新 field 追加 PRD で、`grep
              -rn "<TargetStruct> {" src/` で全 construction site を抽出、各 site に対して
              新 field の責務 (3 strategy のいずれか) を spec-traceable に記録。`## Field
              Addition Symmetric Audit` section を PRD doc に embed mandatory
            - **(9-3-2) Audit script auto-verify**: `audit-prd-rule10-compliance.py` に
              `verify_field_addition_symmetric_audit` function 追加候補 (新 field grep +
              全 construction site enumerate verify、`## Field Addition Symmetric Audit`
              section 不在時 audit fail)
            - **(9-3-3) Post-implementation review trigger**: bulk-script による mass field
              addition 後は **必ず symmetric review pass** を実施 (= bulk-script 終了 ≠
              symmetric review 完了。compile pass + test pass のみでは latent symmetric
              gap 検出不能)
            **Recurring problem rationale**: bulk-script による IR struct field
            addition (= 全 construction site に default 値 hardcode) では、field を
            生成元から伝搬すべき site (= converter / pass-N transformation 等の
            propagation-required site) における symmetric review 漏れにより
            **latent field drop** が発生する pattern が codebase 内で複数回発生済。
            compile pass + test pass のみでは latent drop は検出不能で、Adversarial
            trade-off review (Layer 4) で初めて発見される性質のため、structural
            prevention (= 本 sub-rule による mandatory symmetric audit) が必須。

            **補足 (complementary solution)**: 本 sub-rule (9-3) は **process-level**
            解決 (新 field 追加時の symmetric audit checklist)。codebase-wide での
            **structural-level** 解決 (= IR struct construction site の集約
            abstraction 化) は相補的に有効 (process が短期防御、structural が長期
            根本解消)。
      - **(9-4)** **Cell numbering convention single-source-of-truth**:
            Matrix cell の canonical identifier は **matrix #** (= matrix table 内 row
            number、1-based sequential) を **single-source-of-truth** として確定し、
            Spec→Impl Mapping table / Implementation Stage Tasks / Test contract path /
            Invariants / Iteration Log 等 全 PRD cross-reference contexts で **同一 matrix
            # を使用** する。surface convention drift (= 同一 cell に複数 identifier 体系を
            混在使用、例: matrix 内では `Cell 1`、INV section では `e2e fixture-1`、Iteration
            entry では `C-1` 等) は **structural framework rule 違反** で禁止。
            Verification mechanism:
            - **(9-4-1) `## Cell Numbering Convention` section embed mandatory**: PRD doc に
              `## Cell Numbering Convention` heading section を hard-code、本 section 内に:
              (i) canonical identifier 形式 (= "matrix #" or "Cell N" 等の declared form) +
              (ii) section coverage policy (= 本 convention が適用される PRD sections の
              列挙、Rule 13 (13-2) `## Spec Review Iteration Log` section との
              overlap allow) + (iii) audit auto-detect mechanism reference
              (= 下記 (9-4-2) 参照) を embed 必須
            - **(9-4-2) Audit script auto-detect (identifier-level fork detection)**:
              `audit-prd-rule10-compliance.py` に `verify_cell_numbering_drift_detection`
              function 追加 + Path E utility (`scripts/verify_prd_self_audits.py`) Axis 3
              で `CELL_SLOT_AS_IDENTIFIER_RE` narrow scope (= "cell-slot N" / "cell-slot #N"
              数値 identifier 用法 = canonical 違反 detection) を auto-detect。Helper
              `has_cell_numbering_convention_section()` で auto-detect (= `## Cell Numbering
              Convention` section 不在 PRD は audit out-of-scope、future-proof design =
              section 追加で audit scope 内自動 promote)
            - **(9-4-3) Identifier-level fork scope**: 本 sub-rule (9-4) は **identifier-level
              fork detection** に限定 (= numeric identifier 用法のみ flag、descriptive uses
              ("cell-slot occurrence" / "cell-slot vocabulary fork" 等 concept descriptors) は
              legitimate として allow)。**Broader vocabulary fork detection** (= "cell # /
              candidate ID / matrix #" 間の mixed canonical naming, semantic-level 検出)
              は別 framework concern で本 sub-rule scope 外 (= 別 PRD で structural extend
              候補)
            **Recurring problem rationale**: 同一 identifier name (例: `cell-N`) が
            異なる numbering scheme (例: matrix # vs sequential filename numbering)
            で異なる cell を指す **surface convention drift** が発生すると reader
            confusion (= 同名異物による誤読) を生じる。個別 patch 対応のみでは再発
            防止できず、framework rule level での single-source-of-truth enforcement
            (= 本 sub-rule + audit script auto-detect) が必須。
      - **(9-5) Spec→Impl Dispatch Arm Mapping table embed mandatory**:
            matrix-driven PRD は `## Spec→Impl Dispatch Arm Mapping` heading section を
            **独立 sub-section として hard-code**、各 in-scope matrix cell ↔ dispatch
            tree leaf (= Implementation Stage Tasks T-N 系列の leaf-level task) の
            **1-to-1 correspondence** を table 形式で record。table 構造 minimum
            columns: (i) Cell # / (ii) Candidate ID or Cell label / (iii) Implementation
            Task ID / (iv) Test contract path / (v) Audit verify mechanism。
            **Verification mechanism**: `audit-prd-rule10-compliance.py` に
            `verify_dispatch_arm_mapping_table` function (= 本 sub-rule (9-5) に対応
            する audit auto-verify、cell 9 v4-3 candidate 由来) を hard-code、本 table の
            **completeness + 1-to-1 invariant** (= 全 in-scope matrix cell が exactly
            one task に dispatch + 全 task が exactly one cell から dispatch、no
            double-claim + no fall-through) を auto verify。
            **Recurring problem rationale**: dense matrix の cross-reference contexts
            (matrix table cell # ↔ task list ↔ test contracts) が複数存在する PRD で
            cell ↔ task mapping を informal narrative 内に scatter すると 1-to-1
            mapping invariant の verify が manual 化し、reviewer の review iteration
            毎に partial mapping drift が defect として露呈する pattern が再発する。
            structured table としての hard-code + audit auto-verify で structural
            prevention prerequisite。
      - **(9-6) Substitute / rewrite logic dispatch arm symmetric application**:
            Rule 9 (Dispatch-arm sub-case alignment) を **substitute / rewrite logic** の
            dispatch arm にも symmetric 適用。具体: PRD が **rewrite-time** に user
            source の AST を substitute / inject する logic を含む場合 (例: `__ts_main()`
            substitute call の async wrap、`await` injection 等)、dispatch arm の
            symmetric coverage = **sync substitute / async substitute / no substitute**
            の **3 arm 全てが test cell coverage を持つ** verify mechanism を Spec stage
            で mandatory 確立。
            **Recurring problem rationale**: substitute / rewrite logic の dispatch arm は
            "現状の rewrite logic は async substitute のみ" claim が無 enumerate のまま
            spec に embed され、sync substitute / no substitute arm に対する test cell
            coverage gap が Tier 1 silent semantic loss として latent 化する pattern が
            再発する。Rule 9 symmetric application で 3 arm 全 coverage を framework rule
            level で enforce する structural prevention prerequisite。
      実装の dispatch arms と PRD matrix cell が乖離する場合は **Spec gap signal**:
      PRD 起草時に問題空間を網羅していなかった証拠であり、現 PRD scope の rework または
      別 PRD 切り出しを判断する。
- [ ] **Cross-axis matrix completeness**: PRD の matrix は **「PRD の解決軸 single
      dimension」ではなく「機能 emission を決定する全 input dimension の直積
      (Cartesian product)」** で構築する。
      Construction procedure (3 steps):
      1. **Axis enumeration**: 機能 emission を決定する独立 input dimension を全列挙
         する。最低限 check すべき axis 候補:
         (a) trigger condition (operator / syntax-form), (b) operand type variants,
         (c) guard variant (typeof / equality / instanceof / truthy),
         (d) body shape (block / expr / single-stmt / empty),
         (e) closure-reassign 有無, (f) early-return 有無,
         (g) outer emission context (return / assign target / call arg / branch arm),
         (h) control-flow exit (上記 Rule "Control-flow exit sub-case completeness"
         の 4 sub-case),
         (i) **AST dispatch hierarchy**: parent enum + child enum の各 layer を
         独立 axis として enumerate (例: `for prop in object_lit.props` の最外層 match
         で `PropOrSpread::{Spread, Prop(Box<Prop>)}` を dispatch、内部で `Prop::*`
         を dispatch する場合、両 layer を独立 cell として matrix 化。parent enum を
         child enum と混在記述すると dispatch arm completeness が破綻し ast-variants.md
         single source of truth 違反となる)。
         **caller-supplied wrap context awareness extension (cell 15 / v11-3 由来)**:
         axis (i) AST dispatch hierarchy wording は **rewrite / substitute / IR-injection logic の caller-supplied wrap context awareness** にも extend する。具体: PRD が
         **substitute-time** に AST node を injection / rewrite する logic を含む場合、
         同 AST node が **outer caller context** で wrap される可能性 (= 例: substitute
         時 `.await` wrap + outer `Expr::Await` の二重作用 = **double-await structural bug**) を axis (i) sub-dimension として enumerate 必須。caller-supplied wrap
         context を axis (i) で enumerate しない場合、Layer 3 cross-axis review で
         latent defect として後発検出される pattern が再発するため、Spec stage で axis
         (i) を caller wrap context awareness にも extend した形で enumerate prerequisite。
         (j) **double-source consistency (cell 18 / v11-6 由来)**: 解決軸の **同義 doc
         surfaces** (= handoff doc + script comment + canonical source comment 等の
         **double-source / triple-source surfaces**、同一 design decision を複数 doc /
         comment surface で express する場合) が **token-level に accurate な双方 update** を verify する axis。double-source / triple-source surfaces が exists
         する PRD では、各 source surface 間の wording sync 状態を spec-traceable に
         enumerate + audit script で auto verify 必須。
         **Test infra PRD 用 axis extension (cell 23 / v11-11 由来、Iteration v27 post-/check_job L3-1 fix で Axis F/G capital → (k)/(l) lowercase に scheme normalize で structural consistency 確立)**: **test infra defect PRD** では以下 axis を default check axis として enumerate 必須:
         (k) **cargo profile = debug/release**: test 実行時 cargo profile variance が
           defect repro / mitigation の dimension となる場合 (例: debug profile での
           parallel test isolation defect が release profile で reproducibility 異なる
           pattern) を axis として enumerate
         (l) **rustc version variance**: rustc version (= MSRV / nightly /
           specific stable) variance が defect repro / mitigation の dimension となる
           場合 (例: rustc-version-specific の compile error / runtime behavior 差異) を
           axis として enumerate
         out-of-scope なら N/A justification (= "本 PRD は test infra defect ではなく
         conversion mechanism PRD のため (k)/(l) applicable せず" 等の spec-traceable
         declare) を `## Rule 10 Application` section 内に explicit declare 必須。
         機能依存で必要な axis を追加するが、上記 axes (a)-(l) を default check 対象とする
         (= 9 general purpose axes (a)-(i) + 1 cross-cutting double-source axis (j) +
         2 test-infra-specific axes (k)/(l) = 12 total default、(k)/(l) は test infra PRD
         でのみ relevant、その他 PRD では N/A justification で declare)。**Naming scheme
         single-source-of-truth (post-Iteration v27 L3-1 fix)**: 全 default check axes を
         **lowercase letter naming scheme** で uniform 化、Spec stage Oracle Observation
         由来の旧 "Axis F / Axis G" capital wording は (k)/(l) に normalize、Rule 10 axis
         enumeration の **structural consistency** 確立 (= dual naming scheme inconsistency
         を structural prevention)。
      2. **Orthogonality verification**: 各 axis pair (A, B) について「A の variant 変化が
         B の variant 出力を変えるか」を自問し、yes なら独立 dimension として保持、
         no なら一つに統合 (NA cell を作る前にこの統合を実施)。
      3. **Cartesian product expansion**: 残った全 axis の直積を matrix table に展開し、
         各 cell に ideal output を spec する。NA cell には spec-traceable な reason を
         記載 (`spec-first-prd.md` Stage 1 artifact #1 参照)。

      直交軸の発見方法 (axis enumeration を補完する 3 prompt):
      - **(I) 逆問題視点**: 解決軸の対立軸を試案化。
        例: 解決軸=cohesion → 反問軸=trade-off /
        解決軸=symmetric-coverage → 反問軸=asymmetric-coverage /
        解決軸=preservation → 反問軸=erasure
      - **(II) 実装 dispatch trace**: 実装の dispatch / branch / pattern-match が消費する
        次元 (guard variant, type, operator, context) を全列挙し、各々を独立 axis として
        PRD に reflect
      - **(III) 影響伝搬 chain**: "X が変わると Y が変わるか" を再帰適用し、間接 dimension
        を抽出

      禁止事項: "実装した cell リスト" を matrix と称すること /
      "思いつく組合せのみ" / "代表的な軸のみ" / 「典型的でない組合せは省略」。

      **Failure pattern**: 解決軸内 coverage を意識していても直交軸 (cross-axis)
      enumeration を欠くと、複数 review iteration を跨いで類似 defect が連続発生
      (例: trigger conditions × guard variants / emission paths × null-check direction /
      test source × structural properties / emission form × body shape 等の異なる
      axis pair で defect が露呈) する。axis pair 単位で proactive enumerate しない限り
      structural 防止不能。
- [ ] **AST node enumerate completeness check**:
      Rust source 内の **全 enum match 文** で以下を verification:
      - **(11-1)** **`_ => ` arm の使用を全面禁止** (compile time exhaustiveness、
        新 variant 追加時 compile error で全 dispatch fix 強制)
      - **(11-2)** 未対応 variant も explicit enumerate、**phase 別 mechanism で
        structural enforcement**:
        - **Transformer (変換 phase)**: 既存 `UnsupportedSyntaxError::new(kind, span)`
          mechanism (`src/transformer/mod.rs:193-219`) で error return 必須。
          format 統一 + production-friendly Err return (panic / todo! ではない)、
          user-facing transparent error は `src/lib.rs:96-97 resolve_unsupported()`
          経由 line/col 含む
        - **TypeResolver (静的解析 phase)**: abort 不可 (= 全 conversion path 早期
          abort = ideal 違反のため `UnsupportedSyntaxError` 呼出 不可)、
          **明示 no-op (reason comment 付き empty arm) で記述**。例:
          `ast::ClassMember::TsIndexSignature(_) => { /* Tier 2 filter out: 型 only、`
          `runtime effect なし、no-op (reason: ast-variants.md Tier 2 filter out 参照) */ }`
        - **NA cell (= structurally unreachable)**: `unreachable!()` macro 呼び出し
          (= bug detection mechanism、AST parser reject 前提の defensive coding。
          例: SWC parser が object literal context で `Prop::Assign` を reject、
          もし fire したら parser 仕様変更 bug)
      - **(11-3)** `doc/grammar/ast-variants.md` を **single source of truth** とし、
        各 enum section の Tier 1 (Handled) / Tier 2 (Unsupported) / NA 分類が
        code の handle 状況と sync (Rule 4 (4-2) doc-first dependency order と整合)
      - **(11-4)** `scripts/audit-ast-variant-coverage.py` を CI 化、doc-code sync を
        自動検証 (新 variant 追加時 audit fail で fix 強制、merge gate)
      - **(11-5)** **Pre-draft ast-variant audit mandatory**: 本 PRD scope の修正対象
        file (Impact Area で列挙) に対し PRD draft 段階で
        `python3 scripts/audit-ast-variant-coverage.py --files <impact-area-files>` を
        run、結果を PRD doc 内 `## Impact Area Audit Findings` section に embed。
        既存 violations 全列挙 + 各 violation について本 PRD scope で fix する判断 or
        別 specialized PRD へ defer する判断を spec-traceable に記録。`## Impact Area
        Audit Findings` section 不在 + matrix-driven PRD → audit fail
      - **(11-6) Architectural concern relevance scope**:
        Rule 11 (11-1) "`_ =>` 全面禁止" + "1 PRD = 1 architectural concern" の inherent
        tension の resolution。下記 (11-6-1) で defer 可能 boundary を定義、(11-6-2)(11-6-3)
        で各 defer 対象 `_ =>` arm に対する verification statement を `## Impact Area
        Audit Findings` section に spec-traceable に embed:
        - **(11-6-1) Architectural concern relevance principle**:
          Rule 11 (11-1) は **本 PRD architectural concern に relevant な code path 内**
          の `_ =>` arms に厳格適用。関連 code path = 本 PRD で modify する dispatch
          logic / state mutation / IR construction / control flow を含む arms。
          関連 code path 外の arms (= 本 PRD で touch する file 内でも、別 architectural
          concern に属する arms) は別 specialized PRD (codebase-wide concern fix) へ
          defer 可 (= "1 PRD = 1 architectural concern" との整合、boundary は
          touched files ではなく concern relevance)。
        - **(11-6-2) Architectural concern orthogonality** (defer 対象 arm に対し embed):
          defer 対象 arm が本 PRD architectural concern (= PRD title の dispatch /
          state / IR concern) と orthogonal (関連 code path 外) であることを
          explicit declare
        - **(11-6-3) Non-interference probe** (defer 対象 arm に対し embed): 本 PRD で
          modify する arms の control flow correctness が defer 対象 arm の挙動に
          **dependent しない** こと (= defer arm の bug が本 PRD logic の correctness
          を破らない) を probe で verify、probe location 明記

        (11-6-2)(11-6-3) 両条件を満たさない `_ =>` arm は **本 PRD scope 内で fix 必須**
        (別 PRD defer 不可)。
- [ ] **Rule 10/11 Mandatory application + structural enforcement**:
      - **(12-1)** 全 PRD で Rule 10 + Rule 11 verification を **Mandatory**
        (matrix-driven / non-matrix-driven 区別なし)
      - **(12-2)** Matrix-driven PRD = Rule 10 の 9 default check axes を enumerate +
        Cartesian product matrix 構築
      - **(12-3)** Non-matrix-driven PRD = matrix 不在の structural reason を
        spec-traceable に明示
        - **Permitted reasons**: infra で AST input dimension irrelevant /
          refactor で機能 emission decision なし / pure doc 改修
        - **Prohibited reasons (Anti-pattern、明示禁止 list)**: 「scope 小」/
          「light spec」/ 「pragmatic」/ 「~LOC」/ 「短時間」/ 「manageable」/
          「effort 大」 (`feedback_no_dev_cost_judgment.md` 違反)
      - **(12-4)** matrix 不在でも Cross-axis 直交軸 (解決軸 symmetric / 対称ケース /
        context 軸) の独立 enumerate 必須
      - **(12-5)** structural reason を machine-parseable format で記入
        (PRD doc 内 `## Rule 10 Application` heading + yaml fenced code block):
        ```yaml
        Matrix-driven: yes | no
        Rule 10 axes enumerated:
          - <axis 1>
          - <axis 2>
          - ...
        Cross-axis orthogonal direction enumerated: yes | no
        Structural reason for matrix absence: <reason、Permitted reasons から選択 or N/A>
        ```
      - **(12-6)** `prd-template` skill に Rule 10 application 必須 section を
        hard-code (空のまま skill 終了不可、Step 0c で verification step invocation)
      - **(12-7)** `scripts/audit-prd-rule10-compliance.py` を CI 化 + merge gate
        (PRD doc parse + Rule 10 application section + prohibited keywords 不在 verify
        + Rule 4 (4-3) doc-first dependency order auto verify)
      - **(12-8)** `/check_job` Layer 3 (Structural cross-axis) に Rule 10 application
        verification を integrate

      **Rationale**: (12-3) の Prohibited reasons list は「開発工数 / LOC / scope size を
      判断根拠としない」原則 (`feedback_no_dev_cost_judgment.md` 参照) に基づく
      structural enforcement。
- [ ] **Spec Stage Self-Review (skill workflow integrated)**:
      - **(13-1)** PRD draft 完了直後 (`prd-template` skill workflow Step 4.5) に
        **13-rule self-applied verify** を skill 内 systematic 適用 (skill が check items を
        text として提示、author が逐一 verify)。Step 4.5 不在のまま skill closing 不可
      - **(13-2)** 各 finding を PRD doc `## Spec Review Iteration Log` section に record
        (iteration v1 / v2 / v3 history)。各 iteration entry format:
        - **Iteration #**: v1 / v2 / v3 / ...
        - **Date**: YYYY-MM-DD
        - **Findings count**: Critical N / High N / Medium N / Low N
        - **Findings detail**: 各 finding の summary + RC 対応 (root cause clustering)
        - **Resolution**: PRD doc fix + (該当する場合) framework self-applied integration
      - **(13-3)** Critical findings (= Implementation stage 移行 block する findings) 全 fix
        後、再度 self-applied verify pass で Spec stage 完了
      - **(13-4)** Audit verify mechanism: matrix-driven PRD で `## Spec Review Iteration Log`
        section 不在 or "self-review not performed" の placeholder のみ → audit fail
      - **(13-5)** **Self-applied integration**: review で発見された framework gap
        (= 本 checklist の rule で捕捉できなかった defect class) は PRD close 時に
        skill / rule への improvement として組み込む。derivation 元 PRD 自身を
        first-class adopter として improvement を逆適用し validate する
        (= rule の妥当性を導出元 PRD で self-applied verify)
      - **(13-6)** **Cell numbering convention audit symmetry**: Rule 9 (9-4) `## Cell Numbering
        Convention` section embed mandatory に対応する **audit auto-verify mechanism** を
        Rule 13 audit (= `verify_*` family in `audit-prd-rule10-compliance.py`) として
        hard-code。具体的には:
        - **(13-6-a) Section presence verify**: matrix-driven PRD で `## Cell Numbering
          Convention` heading section 不在 → audit fail (= Helper
          `has_cell_numbering_convention_section()` False return 時の audit error)
        - **(13-6-b) Auto-detect helper as audit out-of-scope dispatcher**: 同 Helper
          が True を return する PRD のみ `verify_cell_numbering_drift_detection` 等の
          NEW verify functions を apply (= `## Cell Numbering Convention` section 不在
          PRD は audit out-of-scope に自動分類、既存 PRD への retroactive audit
          application を回避 = baseline preservation principle)
        - **(13-6-c) Audit ↔ Rule symmetry principle**: 全 cell numbering convention rule
          (= Rule 9 (9-4) sub-rules) に対応する audit script auto-verification を整備
          (= framework rule-audit symmetry principle = 全 rule に対応する audit
          auto-verification、verification deferral eliminate の cell numbering convention
          領域への適用)
      - **(13-7) Pending verdict severity Critical default + audit auto-verify**:
        13-rule self-applied verify table 内 sub-rule rows に **pending verdict** (=
        verify-pending / 未確定 / TBD 等の non-confirmed state) が存在する場合、以下を
        rule per default 適用:
        - **(13-7-a) findings count を ≥1** (= pending verdict 1 entry = finding 1 件
          として count、self-review が 0 findings claim する false-positive を structural
          prevent)
        - **(13-7-b) Severity default = Critical** (= Spec stage 移行 block する severity、
          pending verdict が Spec stage 移行 blocker として treat される default)
        - **(13-7-c) Audit auto-verify mechanism**: `audit-prd-rule10-compliance.py` 内
          sub-check で 13-rule self-applied verify table を parse、pending verdict
          patterns (= "verify pending" / "TBD" / "要確認" / 同義 wording) を detect、
          findings count vs claimed Critical count の inconsistency を auto fail
        **Recurring problem rationale**: PRD draft Iteration v1 self-review が Rule 1 等
        の structural violation (= severity Spec stage 移行 block) を "High" / "Medium"
        と self-classify する false-positive pattern (= severity blindness) が再発する。
        Pending verdict severity default = Critical の rule per default 適用 + audit
        auto-verify で severity assignment subjectivity を eliminate し、reviewer
        independence の structural prevention prerequisite。
```

## Prohibited

- 1 つでも未達の checkbox がある状態で Implementation stage に移行すること
- checkbox を「[x] にした」だけで内容未検証で済ませること (各 rule の Verification 手順を
  実際に実施すること)
- 各 rule の Failure pattern / Rationale / Recurring problem rationale 等を
  「過去の事例だから」と読み飛ばすこと (= rule 適用時の severity 判断基準として
  load-bearing、特に Recurring problem rationale は structural prevention の必要性を
  正当化している)
- 「典型的でない」「実装が複雑になる」を理由に matrix cell を省略すること
  (`problem-space-analysis.md` の最上位原則違反)

## Related Rules

| Rule | Relation |
|------|----------|
| [spec-first-prd.md](spec-first-prd.md) | 本 checklist を発動する PRD lifecycle workflow。Spec stage 完了 verification 手段として本 file を参照 |
| [problem-space-analysis.md](problem-space-analysis.md) | Rule 10 (Cross-axis matrix completeness) の理論的根拠。matrix construction の detailed methodology |
| [check-job-review-layers.md](check-job-review-layers.md) | 本 checklist の symmetric counterpart (Implementation stage 側 review framework)。Rule 10 ↔ Layer 3 (Structural cross-axis) は同 lesson の spec/review 両面 |
| [post-implementation-defect-classification.md](post-implementation-defect-classification.md) | Spec stage で漏れた defect が Implementation stage で発見された場合の category 分類 (Spec gap = 本 checklist 失敗の indicator) |
| [ideal-implementation-primacy.md](ideal-implementation-primacy.md) | 最上位原則。本 checklist は理想実装達成のための structural verification mechanism |
