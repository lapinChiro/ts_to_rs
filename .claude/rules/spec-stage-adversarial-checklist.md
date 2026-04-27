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
        - 連番 row range (`| 30-35 | A | ... |` / `| 24-29 | ... |` 等の grouping) 禁止 — 各 cell 独立 row
        - `representative` / `representative cell のみ` / `代表的` / `省略` / `abbreviated` 等の wording 禁止
        - `(各別 cell)` / `(同上)` / `varies` / `(... と同 logic)` 等の placeholder 禁止
      - **(1-3)** Audit verify mechanism: `scripts/audit-prd-rule10-compliance.py` で
        Problem Space matrix table を parse し abbreviation pattern detection
        (上記禁止 list の regex match) で自動検出、merge gate
      - **(1-4) Orthogonality merge legitimacy + Spec-stage structural verify**
        (framework v1.5 source、確定 2026-04-28、I-205 deep review v3 final v3 で v1.4
        rationalization stance を pure ideal stricter form へ revise):
        `D 全` / `B 全` / `Bn-Bm` 等 axis-merge wording は Rule 10 Step 2
        orthogonality merge として **legitimate** (dispatch logic 同一の場合のみ)。
        ただし以下 3 条件 (1-4-a)/(1-4-b)/(1-4-c) を **全 verify** した場合のみ
        Rule 1 (1-2) compliant と認める。defer は禁止 (= Spec stage で全 verify):
        - **(1-4-a) Orthogonality verification statement**: merge cell の adjacent
          text に "orthogonality-equivalent" / "orthogonality-equivalent dispatch" /
          "Rule 10 Step 2 orthogonality merge" 等 explicit justification 記載 +
          **referenced source cell の cell # を明示** (例: "cells 24-28 と
          orthogonality-equivalent")
        - **(1-4-b) Spec-stage structural consistency verify (REVISED v1.5)**:
          referenced source cell が **matrix 内に存在** + 本 cell と source cell の
          Scope 列値が **一致 or compatible** であることを audit script で structural
          verify。Probe を Implementation Stage に defer する v1.4 stance は **revoked**
          (compromise だった)。Spec stage で structural verify pass 必須
        - **(1-4-c) Spec-stage referenced cell symmetry probe**: merge cell が claim
          する dispatch (例: "Tier 2 honest error reclassify") と referenced source
          cell の dispatch が **symmetric (token-level prefix一致)** であることを
          audit script で verify (例: cell 22 "Tier 2 honest error reclassify" claim
          + cell 12 "Tier 2 honest error reclassify" → symmetric prefix match)
        Audit script `audit-prd-rule10-compliance.py` で (1-4-b)(1-4-c) を auto verify
        (新規 `verify_orthogonality_merge_consistency` function、framework v1.5)。
        Lesson source: I-205 deep review v3 final v3 (2026-04-28) で v1.4 (1-4-b)
        Implementation Stage defer が pragmatic compromise = framework integrity 損失
        と判明、(1-4-b)(1-4-c) Spec stage structural verify に revise。
      - Lesson source: I-205 PRD draft v1 (2026-04-27) で matrix abbreviation pattern
        (cells 24-29 / 30-35 / 36-41 grouping、`...` placeholder、`(各別 cell)` 等) を
        許容、第三者 review F1 として発覚 → Rule 1 sub-rule (1-1)(1-2)(1-3) 拡張
        (本 PRD I-205 self-applied integration)。
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
      - Lesson source: I-205 PRD draft v1 (2026-04-27) で tsc observation を
        会話内のみ実施、PRD doc 未 embed → 第三者 review F2 → Rule 2 sub-rule 拡張
        (本 PRD self-applied integration)。
- [ ] **NA justification + SWC parser empirical observation 必須**:
      - **(3-1)** NA セルの理由が spec-traceable (TS spec 上 syntax error / grammar
        constraint / Rust type system 構造的制約 等) であり、「稀」「多分」「頻度低」等の
        曖昧理由がない
      - **(3-2)** TS spec で "syntax error" / "parse error" / "rejected" と documented
        されていても、**SWC parser が actual に reject するかは empirical 確認必須**
        (= TS spec ≠ SWC parser behavior、SWC parser は寛容 parsing で TS spec 違反
        syntax を AST に含める ケースあり)。NA cell として記載する前に
        `crate::parser::parse_typescript()` を直接呼び実行し、SWC が `Err` を返す or
        期待 AST shape を構築しない事を **empirical lock-in test**
        (`tests/swc_parser_*_test.rs` 等の structural placement) で verify
      - **(3-3)** SWC parser が accept する場合 = NA cell ではなく **Tier 2 honest
        error** に reclassify (= `UnsupportedSyntaxError` 経由 explicit reject、
        `ideal-implementation-primacy.md` Tier 1 silent semantic change リスクを排除、
        `unreachable!()` macro の precondition violation を構造的に防止)
      - **Lesson source**: PRD 2.7 (I-198 + I-199 + I-200 cohesive batch)
        Implementation Revision 2 (2026-04-27) — cell 15 (`Prop::Assign` in object
        literal context、`{ x = expr }`) を当初 NA 認識 (TS spec parse error 前提) で
        `unreachable!()` macro 設計 → SWC parser empirical observation で **accept**
        確認、precondition violation 発覚 → Tier 2 honest error reclassify。framework
        失敗 signal を起点に Rule 3 wording に SWC parser empirical observation 必須
        sub-rule (3-1)〜(3-3) を追加 (本 PRD 2.7 self-applied integration 完成)。
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
        source of truth の structural 違反、Rule 11 (d-3) 整合)。
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
      - **Lesson source**: PRD 2.7 (I-198 + I-199 + I-200 cohesive batch) draft
        自体で T11 (`ast-variants.md` update) が T8/T9/T10 (code 改修) の **後** に
        位置していた = single source of truth 違反 = Rule 4 違反。1 度目 review +
        2 度目 review で未検出、3 度目 `/check_job` review (2026-04-27) で初めて
        Spec gap として発覚。framework 失敗 signal を起点に Rule 4 wording に
        doc-first dependency order の structural enforcement を追加 (PRD 2.7 自体が
        first-class adopter として self-applied verify)。
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
      - Lesson source: I-205 PRD draft v1 (2026-04-27) で T0 (E2E fixture creation) を
        Implementation stage tasks 内に置いた → 第三者 review F8 → Rule 5 sub-rule
        (5-1)(5-2)(5-3)(5-4) 拡張 (本 PRD self-applied integration)。
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
      - Lesson source (Rule 6-1): I-161 SG-2 — matrix が `pred(x)` を ideal とし Design が
        `match` block を ideal としていた spec 内乖離を初回 review で漏らした事例。
      - Lesson source (Rule 6-2/6-3/6-4): I-205 PRD draft v1 (2026-04-27) で B6
        (regular method `obj.x` no-paren) が Out of Scope に記述、matrix で In Scope
        (Tier 2 honest error 化) と矛盾 → 第三者 review F9 → Scope 3-tier hard-code 追加
        (本 PRD self-applied integration)。
- [ ] **Control-flow exit sub-case completeness**: Matrix cell の dimension に
      "body shape" / "branch shape" が含まれる場合、各 branch の **exit-or-fallthrough
      状態** を独立次元として enumerate する。最低 4 sub-case
      (then_exits × else_exits = T/T, T/F, F/T, F/F) を明示 cell として PRD に記載し、
      各 sub-case に対応する E2E fixture と ideal output を spec する。
      "any × any" / "either-exits" / "regardless of else" 等の集約表現は禁止
      (集約は post-implementation の audit で defect を hide する)。
      Verification: matrix table で body / else dimension の cell を抽出し、各 cell の
      row が 4 行に展開されていることを目視確認。
      (Lesson: I-171 T5 SG-T5-DEEP1 — C-5 cell が "any body × any else" で lump され、
      (then_exits=T, else_exits=F) sub-case の post-if narrow materialization 漏れが
      4 度目の iteration で発覚。)
- [ ] **Cross-cutting invariant enumeration**: 機能仕様の中に「matrix cell に展開
      できない / 全 cell で同時に成立する必要がある」transversal property が存在しないか
      自問し、存在する場合は PRD に独立 section として `## Invariants` を設けて列挙する。
      各 invariant について以下 4 項目を必須記述:
      - (a) **Property statement**: 1 文で書けるレベルの不変条件
            (例: 「TypeResolver の expr_type と IR の Type が同一 span に対し一致」)
      - (b) **Justification**: なぜこの invariant が必要か
            (この invariant 違反でどんな defect class が発生するか)
      - (c) **Verification method**: 実装後に invariant 成立を verify する具体手順
            (probe / test / static analysis のどれを使うか)
      - (d) **Failure detectability**: invariant 違反が compile error / runtime error /
            silent semantic change のどれで顕在化するか
      候補 invariant カテゴリ (探索 prompt として活用): TypeResolver-IR cohesion /
      並列 emission path symmetry / closure-reassign suppression cohesion /
      scope boundary preservation / mutability propagation。
      - **(8-5)** Audit verify mechanism: matrix-driven PRD で `## Invariants` section
        不在 or 空 (4 項目 (a)(b)(c)(d) のいずれか missing) → audit fail
      - Lesson (Rule 8-1〜8-4): I-171 T5 SG-T5-DEEPDEEP1/2 — matrix cell 列挙では捕捉不能な
        「TypeResolver と IR の同期 invariant」「if-stmt の then/else 並列 emission path の
        symmetry invariant」が後付けで INV-1/2/3 として retroactive に enumerate された経緯。
        前置 enumerate していれば 2 度目 deep iteration の defect は initial review で
        発見可能だった。
      - Lesson (Rule 8-5): I-205 PRD draft v1 (2026-04-27) で `## Invariants` section が
        checklist 内のみ列挙、独立 section 不在 → 第三者 review F6 → audit verify
        mechanism (8-5) 追加 (本 PRD self-applied integration)。
- [ ] **Dispatch-arm sub-case alignment**: Matrix の各 type-dimension は、実装側で
      **branch / dispatch / pattern-match を分けるあらゆる sub-classifier** と一対一の
      粒度で enumerate する。具体例: `Named` を単一 cell として記述する代わりに、実装が
      `is_synthetic_union` flag や `is_always_truthy` 判定で dispatch 分岐するなら
      `Named (synthetic union)` / `Named (always-truthy)` / `Named (other)` の 3 cell に
      分割。
      Verification (両方向の同期):
      - (a) **Spec → Impl**: PRD 確定後、実装着手前に「実装 file の dispatch / match を
            全 enumerate し、matrix cell と 1-to-1 対応するか」を check
      - (b) **Impl → Spec**: 実装中に新しい dispatch arm を追加する必要を発見した場合、
            Spec stage に戻って matrix cell を分割する (`spec-first-prd.md` の
            「Spec への逆戻り」手順を発動)
      実装の dispatch arms と PRD matrix cell が乖離する場合は **Spec gap signal**:
      PRD 起草時に問題空間を網羅していなかった証拠であり、現 PRD scope の rework または
      別 PRD 切り出しを判断する。
      (Lesson: I-171 T5 SG-T5-FRESH1 — `Option<Named other>` を PRD で 1 cell とした
      結果、実装側で synthetic union / 非 synthetic 2 dispatch arm の存在に気付かず、
      4 度目 iteration で `always-truthy 全型対応漏れ` として defect 発覚。)
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
         single source of truth 違反となる、Lesson: PRD 2.7 Implementation stage
         Revision 1 = T11 中に PropOrSpread section 不在を発見、Grammar gap を
         本 PRD scope 内で fix)。
         機能依存で必要な axis を追加するが、上記 9 候補は default check 対象とする。
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
      (Lesson: I-161 T7 三度の `/check_job` iteration — 4 defect 全てが直積 enumeration
      不足に帰着。Defect 1 = trigger conditions × guard variants で Truthy 誤発火 /
      Defect 2 = emission paths × null-check direction で path 3 symmetric coverage 欠落 /
      Defect 3 = test source × structural properties で empty else false-positive /
      Defect 4 = emission form × body shape で Scenario A regression。
      解決軸内 coverage は意識していたが直交軸を見ていなかった。)
- [ ] **AST node enumerate completeness check** (Q4 source、PRD 2.7 確定):
      Rust source 内の **全 enum match 文** で以下を verification:
      - **(d-1)** **`_ => ` arm の使用を全面禁止** (compile time exhaustiveness、
        新 variant 追加時 compile error で全 dispatch fix 強制)
      - **(d-2)** 未対応 variant も explicit enumerate、**phase 別 mechanism で
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
      - **(d-3)** `doc/grammar/ast-variants.md` を **single source of truth** とし、
        各 enum section の Tier 1 (Handled) / Tier 2 (Unsupported) / NA 分類が
        code の handle 状況と sync (Rule 4 (4-2) doc-first dependency order と整合)
      - **(d-4)** `scripts/audit-ast-variant-coverage.py` を CI 化、doc-code sync を
        自動検証 (新 variant 追加時 audit fail で fix 強制、merge gate)
      - **(d-5)** **Pre-draft ast-variant audit mandatory**: 本 PRD scope の修正対象
        file (Impact Area で列挙) に対し PRD draft 段階で
        `python3 scripts/audit-ast-variant-coverage.py --files <impact-area-files>` を
        run、結果を PRD doc 内 `## Impact Area Audit Findings` section に embed。
        既存 violations 全列挙 + 各 violation について本 PRD scope で fix する判断 or
        I-203 へ defer する判断を spec-traceable に記録。`## Impact Area Audit Findings`
        section 不在 + matrix-driven PRD → audit fail
      - **(d-6) Architectural concern relevance scope** (framework v1.5 source、I-205
        deep review v3 final v3 で v1.4 stance を pure ideal stricter form へ revise、
        確定 2026-04-28):
        Rule 11 (d-1) "`_ =>` 全面禁止" + "1 PRD = 1 architectural concern" の inherent
        tension の resolution:
        - **(d-6-a) Architectural concern relevance principle (REVISED v1.5)**:
          Rule 11 (d-1) は **本 PRD architectural concern に relevant な code path 内**
          の `_ =>` arms に厳格適用。関連 code path = 本 PRD で modify する dispatch
          logic / state mutation / IR construction / control flow を含む arms。
          関連 code path 外の arms (= 本 PRD で touch する file 内でも、別 architectural
          concern に属する arms) は specialized PRD (I-203 codebase-wide) へ defer 可。
          v1.4 の "touched-files-strict" 解釈は revoked、"architectural-concern-relevance"
          解釈に置換 (= "1 PRD = 1 architectural concern" との整合)。
        - **(d-6-b) Spec-stage relevance verify**: 各 defer 対象 `_ =>` arm について、
          以下 2 verification statement を `## Impact Area Audit Findings` section に
          spec-traceable に embed:
          - **(d-6-b-1) Architectural concern orthogonality**: defer 対象 arm が本 PRD
            architectural concern (= PRD title の dispatch / state / IR concern) と
            orthogonal (関連 code path 外) であることを explicit declare
          - **(d-6-b-2) Non-interference probe**: 本 PRD で modify する arms の control
            flow correctness が defer 対象 arm の挙動に **dependent しない** こと
            (= defer arm の bug が本 PRD logic の correctness を破らない) を probe で
            verify、probe location 明記
        両条件を満たさない `_ =>` arm は **本 PRD scope 内で fix 必須** (I-203 defer 不可)。
        Lesson source (v1.4 → v1.5): I-205 deep review v3 final v3 で v1.4 (d-6) の
        "touched-files-strict" stance が "1 PRD = 1 architectural concern" 原則と
        inherent tension を生む rationalization と判明、"architectural-concern-relevance"
        principle に revise (= 真の boundary は touched files ではなく concern relevance、
        理論的にも consistent)。
      Lesson source (d-1〜d-4): I-177-F (ClassMember variants 全 enumerate せず static block /
      auto accessor 漏れ)、I-200 (ObjectLit Prop variants 全 enumerate せず
      Prop::Method の type-resolve 経路漏れ)、PRD 2.7 (I-198 + I-199 + I-200
      cohesive batch、Q4 確定)。
      Lesson source (d-5): I-205 PRD draft v1 (2026-04-27) で本 PRD 修正範囲全 file の
      `_ => ` arm pre-draft audit が未実施、`registry/collection/class.rs:145` の
      Rule 11 d-1 violation が draft 後 review で発覚 → 第三者 review F13 → (d-5) 追加
      (本 PRD self-applied integration)。
- [ ] **Rule 10/11 Mandatory application + structural enforcement** (Q5 source、
      PRD 2.7 確定):
      - **(e-1)** 全 PRD で Rule 10 + Rule 11 verification を **Mandatory**
        (matrix-driven / non-matrix-driven 区別なし)
      - **(e-2)** Matrix-driven PRD = Rule 10 9 default check axis enumerate +
        Cartesian product matrix 構築
      - **(e-3)** Non-matrix-driven PRD = matrix 不在の structural reason を
        spec-traceable に明示
        - **Permitted reasons**: infra で AST input dimension irrelevant /
          refactor で機能 emission decision なし / pure doc 改修
        - **Prohibited reasons (Anti-pattern、明示禁止 list)**: 「scope 小」/
          「light spec」/ 「pragmatic」/ 「~LOC」/ 「短時間」/ 「manageable」/
          「effort 大」 (`feedback_no_dev_cost_judgment.md` 違反)
      - **(e-4)** matrix 不在でも Cross-axis 直交軸 (解決軸 symmetric / 対称ケース /
        context 軸) の独立 enumerate 必須
      - **(e-5)** structural reason を machine-parseable format で記入
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
      - **(e-6)** `prd-template` skill に Rule 10 application 必須 section を
        hard-code (空のまま skill 終了不可、Step 0c で verification step invocation)
      - **(e-7)** `scripts/audit-prd-rule10-compliance.py` を CI 化 + merge gate
        (PRD doc parse + Rule 10 application section + prohibited keywords 不在 verify
        + Rule 4 (4-3) doc-first dependency order auto verify)
      - **(e-8)** `/check_job` Layer 3 (Structural cross-axis) に Rule 10 application
        verification を integrate
      Lesson source: PRD 2.7 (Q5 確定 2026-04-27)、`feedback_no_dev_cost_judgment.md`
      整合 (= 開発工数 / LOC / scope size を判断根拠としない)。
- [ ] **Spec Stage Self-Review (skill workflow integrated)** (I-205 source、確定 2026-04-27):
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
      - **(13-5)** **Self-applied integration (PRD 2.7 pattern)**: review で発見された
        framework gap (= 本 checklist の rule で捕捉できなかった defect class) は
        PRD close 時に self-applied integration として skill / rule に improvement
        を組み込む。本 PRD I-205 自身が first-class adopter として improvement を
        逆適用し validate
      - Lesson source: I-205 PRD draft v1 (2026-04-27) で Spec Stage Self-Review が
        systematic 不在、第三者 review で 15 findings 発覚 → 9 RC clusters に集約 →
        framework 改善 + Rule 13 新設 (本 PRD self-applied integration)。
```

## Prohibited

- 1 つでも未達の checkbox がある状態で Implementation stage に移行すること
- checkbox を「[x] にした」だけで内容未検証で済ませること (各 rule の Verification 手順を
  実際に実施すること)
- Rule 6-10 の Lesson source を読まずに「過去の事例だから」と省略判断すること
  (lesson は rule 適用時の判断基準として load-bearing)
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

## Versioning

- **v1.6** (2026-04-28): I-205 PRD deep deep review (iteration v7) で v1.5 framework 内 audit asymmetry を発見、framework symmetry restoration:
  - **Rule 8 (8-c) audit auto-verify NEW**: `verify_invariants_test_contracts` function in `audit-prd-rule10-compliance.py`、各 INV-N entry に `test_invariant_N_*` test fn name reference の structural detection (F-deep-deep-2 fix)
  - **Rule 11 (d-6) audit auto-verify NEW**: `verify_rule11_d6_relevance_compliance` function、Impact Area Audit Findings section の defer rows に対し (d-6-b-1) orthogonality declaration + (d-6-b-2) non-interference probe markers の structural detection (F-deep-deep-1 fix、Rule 1 (1-4) audit との symmetry 確立)
  - **Rule 3 (3-2) inheritance justification clarification**: NA cells の orthogonality-equivalent inheritance claim を Spec stage で structural justify (parser-level context-independence 等の explicit declaration、F-deep-deep-3 fix)
  - **Rule 9 (a) helper test contracts NEW**: Spec stage で helper functions の test contracts (test fn / assertion / probe) を author + stub `#[test] #[ignore]` files 作成 (F-deep-deep-4 fix)
  - Lesson source: I-205 PRD deep deep review (iteration v7、2026-04-28) で v1.5 framework 内 4 audit gaps (Rule 8 (8-c) / Rule 11 (d-6) / Rule 3 (3-2) inheritance / Rule 9 (a) helper test contracts) を発見、**framework rule-audit symmetry principle** 確立 (= 全 rule に対応する audit script auto-verification 整備、verification deferral eliminate)
- **v1.3** (2026-04-27): I-205 PRD draft v1 第三者 review で発見された 15 findings の
  9 RC clusters に対する self-applied integration として、Rule 1/2/5/6/8/11 sub-rule 拡張
  + Rule 13 (Spec Stage Self-Review) 新規追加。
  - Rule 1: 単行 wording → sub-rule (1-1)(1-2)(1-3) に拡張 (RC-1、F1 source: matrix
    abbreviation pattern 全面禁止)
  - Rule 2: 単行 wording → sub-rule (2-1)(2-2)(2-3) に拡張 (RC-2、F2 source: PRD doc
    内 `## Oracle Observations` section embed mandatory)
  - Rule 5: 単行 wording → sub-rule (5-1)(5-2)(5-3)(5-4) に拡張 (RC-4、F8 source: Task
    List 2-section split = `## Spec Stage Tasks` + `## Implementation Stage Tasks`)
  - Rule 6: 単行 wording → sub-rule (6-1)(6-2)(6-3)(6-4) に拡張 (RC-5、F9 source: Scope
    section 3-tier hard-code = In Scope + Out of Scope + Tier 2 honest error reclassify)
  - Rule 8: sub-rule (8-5) 追加 (RC-6、F6 source: `## Invariants` section audit verify
    mechanism)
  - Rule 11: sub-rule (d-5) 追加 (RC-8、F13 source: pre-draft ast-variant audit
    mandatory + `## Impact Area Audit Findings` section embed)
  - Rule 13 (新規): Spec Stage Self-Review (RC-9、F10/F11/F14 source: skill workflow Step
    4.5 で 13-rule self-applied verify mandatory + `## Spec Review Iteration Log` embed +
    self-applied integration pattern hard-code)
- **v1.0** (2026-04-25): 5-rule (Matrix completeness / Oracle grounding / NA justification / Grammar consistency / E2E readiness) を `spec-first-prd.md` から本 file に分離、Rule 6-10 を I-178 で追加。
  - Rule 6: Matrix/Design integrity (lesson: I-161 SG-2)
  - Rule 7: Control-flow exit sub-case completeness (lesson: I-171 T5 SG-T5-DEEP1)
  - Rule 8: Cross-cutting invariant enumeration (lesson: I-171 T5 SG-T5-DEEPDEEP1/2)
  - Rule 9: Dispatch-arm sub-case alignment (lesson: I-171 T5 SG-T5-FRESH1)
  - Rule 10: Cross-axis matrix completeness (lesson: I-161 T7 三度 `/check_job` iteration)
- **v1.2** (2026-04-27): PRD 2.7 Implementation stage Revision 2 (cell 15 critical Spec
  gap fix) lesson の self-applied integration として Rule 3 wording 拡張 (sub-rule
  3-1/3-2/3-3 追加、SWC parser empirical observation 必須化)。
  - Rule 3: 単行 wording → sub-rule (3-1)(3-2)(3-3) に拡張
    (Lesson source: PRD 2.7 Implementation Revision 2 — cell 15 NA 誤認識
    `unreachable!()` precondition violation を SWC parser empirical で発覚、Tier 2
    honest error reclassify。同 lesson の framework 改善として Rule 3 wording に
    "SWC parser empirical observation 必須" を追加、TS spec ≠ SWC parser behavior の
    structural enforcement)
- **v1.1** (2026-04-27): PRD 2.7 (I-198 + I-199 + I-200 cohesive batch、Q4/Q5/Q6 確定) で
  Rule 4 拡張 + Rule 10 axis (i) 追加 + Rule 11/12 新規追加。
  - Rule 4: 単行 wording → sub-rule (4-1)(4-2)(4-3) に拡張 (Q6 source、PRD 2.7 draft 自体の
    T11 dependency violation = doc-first 違反を 3 度目 `/check_job` で発覚 → Rule 4 wording
    に doc-first dependency order の structural enforcement + audit script auto verify を追加)
  - Rule 10 (Cross-axis matrix completeness) axis (a)-(h) → (a)-(i) に拡張、新 axis (i)
    "AST dispatch hierarchy: parent enum + child enum の各 layer を独立 axis として
    enumerate" を追加 (Lesson: PRD 2.7 Implementation stage Revision 1 = T11 中に
    PropOrSpread section 不在を発見、Grammar gap を本 PRD scope 内で fix)
  - Rule 11: AST node enumerate completeness check (Q4 source、d-1〜d-4 sub-rule で
    `_` arm 全面禁止 + phase 別 mechanism (Transformer = `UnsupportedSyntaxError`、
    TypeResolver = no-op + reason comment、NA = `unreachable!()`) + ast-variants.md
    single source of truth + audit script CI 化、Lesson: I-177-F + I-200 + PRD 2.7)
  - Rule 12: Rule 10/11 Mandatory application + structural enforcement (Q5 source、
    e-1〜e-8 sub-rule で Mandatory + Permitted/Prohibited reasons + machine-parseable
    format + skill hard-code + audit script CI merge gate、Lesson: PRD 2.7、
    `feedback_no_dev_cost_judgment.md` 整合)
