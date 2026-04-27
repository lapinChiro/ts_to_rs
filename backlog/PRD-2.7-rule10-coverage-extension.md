# PRD 2.7: I-198 + I-199 + I-200 cohesive batch — framework Rule 改修 (Rule 3/4/10/11/12) + TypeResolver coverage extension + structural enforcement

**Status**: **CLOSED 2026-04-27** (Implementation stage T1〜T15 全 task + formal `/check_job` 4-layer review (initial invocation、9 課題発見) + 9 課題本質 fix (F1〜F10) + Implementation Revision 1 (PropOrSpread Grammar gap) + Revision 2 (cell 15 Prop::Assign critical Spec gap) self-applied integration + Spec gap chain trajectory **5 → 3 → 0 → 1 → 0 → 1 → 0** completion)
**Archive role**: framework lesson source (Q4/Q5/Q6 + Implementation Revision 1/2 + 9-finding Action Items 全 history)、Plan η chain reference (= PRD 2.8/2.9/3 start point)、後続 PRD で Rule 3/4/10/11/12 + audit script CI 化 mechanism を first-class adopter として self-applied 適用する際の引用源
**Plan η position**: Optional pre-Step 3 batch (PRD 2.7 完了後 → PRD 2.8 (I-201-A) → PRD 2.9 (I-202) → PRD 3 ...)
**Architectural concern**: "framework Rule 改修 (Rule 3/4/10/11/12) + 拡張による coverage gap detection 完成 + structural enforcement"
**1 PRD = 1 architectural concern**: ✓

---

## Background

### Spec gap chain (5 件、framework signal)

I-177-B PRD (non-matrix-driven、light spec として起票) の実装中 + 後続 review 中に **5 件の Spec gap が連鎖発見**:

1. **I-177-E** (TypeResolver-Synthetic registry integration): empirical 実装中に発見、prerequisite として独立 PRD 起票・close (2026-04-26)
2. **I-177-F initial 2 site** (resolve_arrow_expr / resolve_fn_expr block_end traversal): T4 verification 中に empirical 発見、独立 PRD 起票・close (2026-04-26)
3. **I-177-A symmetric** (IR shadow emission for post-narrow scope): declaration form の post-fix で E0308 発見、I-177-A scope の symmetric として TODO 起票
4. **I-177-F extended 2 site** (class method + constructor 漏れ): 2 度目 `/check_job deep deep` の grep audit で発見、本 batch scope 編入
5. **I-199 + I-200** (static block + obj literal method coverage): 2 度目 `/check_job deep deep` の Layer 3 で発見、本 batch scope に編入

→ 単一 PRD (I-177-B) の起票時 Cross-axis enumerate 不足が、4 度の review iteration を経て 5 件の Spec gap chain を生成。

### Audit 知見 (2026-04-27)

Spec stage Q1 (AutoAccessor 取り扱い) audit で **decorator framework 自体が ts_to_rs で完全に未実装 = silent drop 状態** を発見:
- `grep "Decorator\|decorator" src/` 結果空
- `grep "ast::Decorator\|decorators:" src/` 結果空
- `doc/grammar/ast-variants.md` に Decorator entry 不在

→ 元 I-201 (AutoAccessor + decorator interaction) を I-201-A (AutoAccessor 単体、L3) + I-201-B (Decorator framework 全体、**L1 silent semantic change**) に分割 ((d) 構造分離)。

### 暗黙 silent drop の追加発見 (2026-04-27)

Step 3 Impact Area Code Review 中、`src/pipeline/type_resolver/expressions.rs:367-369` で **暗黙 silent drop** を追加発見:

```rust
ast::Expr::Object(obj) => {
    for prop in &obj.props {
        match prop {
            ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                ast::Prop::KeyValue(kv) => { /* handle */ }
                ast::Prop::Shorthand(ident) => { /* handle */ }
                _ => {
                    total_explicit_props += 1;  // ← Prop::Method/Getter/Setter/Assign を silent drop
                }
            }
        }
    }
}
```

→ これは Q4 真の ideal (`_` arm 全面禁止) の対象、PRD 2.7 で explicit enumerate に変更必要。

### Framework 失敗 signal の Severity

5 件 Spec gap chain + 暗黙 silent drop 追加発見 = **framework 失敗 signal**。`spec-stage-adversarial-checklist.md` Rule 10 (Cross-axis matrix completeness) を **structural enforcement** で改修必要 (人間判断介在では再発防止不能、5 度の empirical signal で確証)。

---

## Problem Space

### 入力次元 (Dimensions)

本 PRD は **rule 改修 + framework strengthening + TypeResolver coverage** の合成 architectural concern を扱うため、入力次元は複数 layer を持つ:

#### Layer A: AST node iterate target (TypeResolver coverage layer)
- **A1**: `ClassMember` enum (`visit_class_decl` の class body match)
- **A2**: `Prop` enum (ObjectLit Prop iterate、`expressions.rs::ast::Expr::Object` arm)

#### Layer B: variant 現状処理
- **B1**: visited explicitly (handle 経路あり)
- **B2**: `_ => {}` 黙殺 (silent drop)
- **B3**: `_ => {...silent count}` 暗黙 silent drop (= expressions.rs:367-369 pattern)
- **B4**: 経路自体不在 (= ObjectLit visit dispatch に該当 arm なし)

#### Layer C: ast-variants.md spec
- **C1**: Tier 1 (Handled、code 状態と sync 必須)
- **C2**: Tier 2 (Unsupported、honest error report 必須)
- **C3**: Section 不在 (= Grammar gap、doc 完全性違反)

#### Layer D: Rule 10 適用範囲
- **D1**: matrix-driven PRD only (現状)
- **D2**: 全 PRD Mandatory (Q5 真の ideal)

#### Layer E: enforcement mechanism
- **E1**: doc 改修のみ (= 人間判断介在)
- **E2**: skill hard-code (= PRD 起票時 mandatory section)
- **E3**: audit script + CI merge gate (= structural enforcement)

### 組合せマトリクス (Q1〜Q5 全確定事項)

| # | 対象 | Layer A/B/C | Layer D/E | Ideal 出力 (確定) | 判定 | Scope |
|---|------|------------|-----------|------------------|------|-------|
| **A1: ClassMember 各 variant** ||||||
| 1 | ClassMember::Method | A1 / B1 / C1 Tier 1 | — | 現状維持 (visit_method_function) | ✓ | regression lock-in test |
| 2 | ClassMember::PrivateMethod | A1 / B1 / C1 Tier 1 | — | 現状維持 | ✓ | regression lock-in test |
| 3 | ClassMember::Constructor | A1 / B1 / C1 Tier 1 | — | 現状維持 (I-177-F で visit_block_stmt 経由済) | ✓ | regression lock-in test |
| 4 | ClassMember::ClassProp | A1 / B1 / C1 Tier 1 | — | 現状維持 (visit_class_prop_init) | ✓ | regression lock-in test |
| 5 | ClassMember::PrivateProp | A1 / B1 / C1 Tier 1 | — | 現状維持 | ✓ | regression lock-in test |
| 6 | **ClassMember::StaticBlock** | A1 / **B2** / C1 Tier 1 (spec 違反) | — | **StaticBlock body を visit_block_stmt 経由 walk** (I-177-F symmetric: enter_scope → collect_emission_hints → visit_block_stmt → leave_scope) | **✗ 要 fix** | **本 PRD (I-199)** |
| 7 | **ClassMember::AutoAccessor** | A1 / **B2** / C2 Tier 2 (Q1 (b) 確定) | — | **TypeResolver: 明示 no-op (静的解析 layer abort 不可、Rule 10(d-2) compliance、reason comment 付き empty arm) / Transformer: 既存 `UnsupportedSyntaxError::new("AutoAccessor", aa.span)` で error return (現状維持、`classes/mod.rs:165-171` 既実装、C2 audit で確認 2026-04-27)** + ast-variants.md AutoAccessor entry を Tier 2 (Unsupported, honest error reported via `UnsupportedSyntaxError`) に明示。完全 Tier 1 化は **I-201-A** (decorator なし subset、L3) + **I-201-B** (decorator framework、L1) で別 PRD 達成 | **✓ Transformer 既実装 + ✗ TypeResolver 黙殺 explicit no-op 化 + ✗ doc update** | **本 PRD (I-199)** + I-201-A/B (別 PRD) |
| 8 | ClassMember::TsIndexSignature | A1 / B2 / C2 Tier 2 (filter out) | — | **TypeResolver: 明示 no-op + filter out reason comment (= "型 only、runtime effect なし、Tier 2 filter out") 付き empty arm。Transformer (`classes/mod.rs:164`): 既存 `{}` no-op 維持 (現状実装と整合)** | ✗ 要 fix (`_` arm 黙殺 → explicit no-op 化、D1+D2 修正 2026-04-27) | 本 PRD (Rule 10(d) application) |
| 9 | ClassMember::Empty | A1 / B2 / C2 Tier 2 (no-op) | — | **TypeResolver: 明示 no-op + no-op reason comment (= "空 member、no-op で正") 付き empty arm。Transformer (`classes/mod.rs:164`): 既存 `{}` no-op 維持** | ✗ 要 fix (`_` arm 黙殺 → explicit no-op 化、D1+D2 修正 2026-04-27) | 本 PRD (Rule 10(d) application) |
| **A2: Prop 各 variant (TypeResolver expressions.rs)** ||||||
| 10 | Prop::KeyValue | A2 / B1 / **C3 Section 不在** | — | 現状維持 + ast-variants.md Prop section 新規追加で Tier 1 (Handled) | **✗ 要 fix (doc)** | **本 PRD (I-200 + Grammar gap fix)** |
| 11 | Prop::Shorthand | A2 / B1 / C3 Section 不在 | — | 現状維持 + Prop section に Tier 1 記載 | ✗ 要 fix (doc) | 本 PRD |
| 12 | **Prop::Method** body | A2 / **B3 (暗黙 silent drop)** / C3 Section 不在 | — | **TypeResolver: visit_method_function 同等の処理 (function-level scope + visit_block_stmt + return type setup)** + Prop section に Tier 1 (TypeResolver visit only) 記載。Transformer 完全 emission は **I-202** (別 PRD、L3) で達成 | **✗ 要 fix** | **本 PRD (I-200 TypeResolver visit)** + I-202 (Transformer emission) |
| 13 | **Prop::Getter** body | A2 / B3 / C3 | — | 同上 (visit_method_function 同等処理) | ✗ 要 fix | 本 PRD + I-202 |
| 14 | **Prop::Setter** body | A2 / B3 / C3 | — | 同上 | ✗ 要 fix | 本 PRD + I-202 |
| 15 | **Prop::Assign** | A2 / B3 / C3 | — | **Implementation Revision 2 (2026-04-27、critical Spec gap fix)**: 当初 NA 認識 + `unreachable!()` 設計だったが、SWC parser empirical observation (`tests/swc_parser_object_literal_prop_assign_test.rs`) で `{ x = expr }` を `Prop::Assign` として **accept** することを確認、`unreachable!()` precondition violation。Tier 2 honest error 化: **TypeResolver (expressions.rs): no-op (静的解析 phase abort 不可) + Transformer (data_literals.rs 3 site: `convert_object_lit` + `convert_discriminated_union_object_lit` + `try_convert_as_hashmap`): `UnsupportedSyntaxError::new("Prop::Assign", ap.span)` 経由 honest error report** + Prop section に Tier 2 (honest error) 明示 + **SWC parser empirical regression lock-in test** (Test 20、SWC parser accept 確認 + 対称 destructuring default は valid 確認) | **✗ 要 fix (Implementation Revision 2、SWC parser empirical で当初 NA 認識を覆し、framework 失敗 signal)** | **本 PRD (Q3 + Implementation Revision 2)** |
| **TypeResolver expressions.rs:367-369 暗黙 silent drop** ||||||
| 16 | `_ => { total_explicit_props += 1; }` (TypeResolver expressions.rs:367-369、暗黙 silent drop) | A2 / B3 | E1 → E3 | **`_` arm 削除 + 全 Prop variant explicit enumerate** (handle 済 KeyValue/Shorthand/Spread + Prop::Method/Getter/Setter は body visit 追加 (cell 12-14、I-200) + Prop::Assign は `unreachable!()` (C3、cell 15)) | ✗ 要 fix | **本 PRD (Q4 Rule 10(d) application)** |
| **Transformer convert_object_lit (data_literals.rs:259-263)** ||||||
| 17 | `_ => Err(anyhow!(...))` (Tier 2 honest だが wildcard + format 不整合) | A2 / B2 (honest、anyhow! format) | E1 → E3 | **`_` arm 削除 + 全 Prop variant explicit enumerate + 既存 `UnsupportedSyntaxError::new("Prop::*", span)` 経由に format 統一 (C6 修正、`anyhow!()` → `UnsupportedSyntaxError`、format 不整合 broken window 解消)** | ✗ 要 fix (Rule 10(d) compliance + format 統一) | **本 PRD (Q4 + C6 application)** |
| **Rule 10 framework 改修 (Q4 + Q5)** ||||||
| 18 | Rule 10(d) AST node enumerate completeness | C1-C3 / D1-D2 / E1-E3 | E3 真の ideal | **`spec-stage-adversarial-checklist.md` Rule 10 に sub-rule (d) 追加** (`_` arm 全面禁止 + 共通 macro + audit script + CI 化) | ✗ 要 fix (Q4) | 本 PRD |
| 19 | Rule 10 全体 Mandatory 化 | D1 → D2 / E1-E3 | E3 真の ideal | **Rule 10 を全 PRD Mandatory + structural reason 明示 (Permitted reasons + Prohibited keywords list) + Cross-axis 軸独立 enumerate + machine-parseable format** | ✗ 要 fix (Q5) | 本 PRD |
| 20 | `prd-template` skill hard-code | E1 → E2 | E2 | **Step 0a / 0b に Rule 10 application 必須 section を hard-code** (空のまま skill 終了不可) | ✗ 要 fix (Q5) | 本 PRD |
| 21 | audit-prd-rule10-compliance.py 新規作成 + CI | E2 → E3 | E3 | **`scripts/audit-prd-rule10-compliance.py` 新規作成 + CI 化 + merge gate** (PRD doc parse + Rule 10 application section + structural reason の prohibited keywords 不在 verify) | ✗ 要 fix (Q5) | 本 PRD |
| 22 | audit-ast-variant-coverage.py 新規作成 + CI | E2 → E3 | E3 | **`scripts/audit-ast-variant-coverage.py` 新規作成 + CI 化 + merge gate** (doc/grammar/ast-variants.md と code の handle 状況 sync verify) | ✗ 要 fix (Q4) | 本 PRD |
| **doc/grammar/ast-variants.md update** ||||||
| 23 | Prop section 新規追加 | C3 → C1/C2 | — | **Prop section 新規追加** (全 7 variant: KeyValue / Shorthand / Method / Getter / Setter / Assign の Tier 分類 + spec-traceable NA reason for Prop::Assign) | ✗ 要 fix | 本 PRD |
| 24 | AutoAccessor entry update | C2 → C2 (Q1 (b) 状態化) | — | AutoAccessor entry を **Tier 2 (Unsupported, Transformer で `UnsupportedSyntaxError::new("AutoAccessor", aa.span)` 経由 honest error report 既実装、I-201-A/B で完全 Tier 1 化予定)** に明示更新 | ✗ 要 fix (doc update のみ、code は既実装) | 本 PRD |
| 25 | Decorator entry 新規追加 (audit driven) | C3 → C2 | — | **Decorator entry 新規追加** (Tier 2 Unsupported、I-201-B で Tier 1 化予定) | ✗ 要 fix (audit driven) | 本 PRD |
| 25.5 | **PropOrSpread section 新規追加** (Implementation stage Revision 1、2026-04-27 T11 実施中発見、Grammar gap fix) | C3 (section 不在) → C1 (Tier 1 Handled) | — | **PropOrSpread section 新規追加** (= Prop section の parent enum、Tier 1 Handled = Spread / Prop(Box<Prop>) の 2 variant、両者既実装で Tier 1 trivial coverage)。section 12 として ObjectPatProp (section 11) の直後に挿入、既存 Prop を section 13 に shift、PropName-Decorator を section 14-20 に shift。matrix cell 16-17 の dispatch enum hierarchy (parent PropOrSpread → child Prop) を doc に reflect、T9/T10 改修対象 file の dispatch arm を audit script (T5) で verify 可能化 | ✗ 要 fix (Implementation stage Revision 1、本 PRD scope 内 fix 完了 2026-04-27) | 本 PRD (T11 scope 内、Spec への逆戻り発動 record は Defect Classification section の Implementation stage Revision 1 entry) |
| **既存 `UnsupportedSyntaxError` mechanism の format 統一 + 適用拡張 (C1 修正 2026-04-27、新規 macro 作成は不要)** ||||||
| 26 | `UnsupportedSyntaxError` format 統一 + 適用拡張 | — | — | **既存 `UnsupportedSyntaxError::new(kind, span)` (`src/transformer/mod.rs:193-219` 定義) を全 Transformer Tier 2 variant arm で統一適用 + 一部 module の `anyhow!()` 経由 format 不整合 (`data_literals.rs:259-263`) を `UnsupportedSyntaxError` に統一**。新規 macro `unsupported_arm!()` 作成は不要 (= DRY 違反、既存 mechanism と機能重複)。 | ✗ 要 fix (format 統一 + 適用拡張) | 本 PRD (C1 + C6 修正) |
| **TypeResolver `_` arm の明示 no-op 化 (C4 修正 2026-04-27、Rule 10(d-2) phase 別役割分担)** ||||||
| 27 | TypeResolver Tier 2 variant の no-op arm | — | — | **TypeResolver (静的解析 phase) は abort 不可、Tier 2 variant arm は明示 no-op (reason comment 付き empty arm) で Rule 10(d-2) compliance 達成**。Transformer (変換 phase) は `UnsupportedSyntaxError` で error return 必須 (cell 26 と integrate)。 | ✗ 要 fix (Rule 10(d-2) wording 改良) | 本 PRD (C4 修正) |
| **Rule 4 doc-first dependency order の structural enforcement (Q6 修正 2026-04-27、3 度目 `/check_job` review で発見した Spec gap × 1 件の framework 改善検討)** ||||||
| 28 | Rule 4 wording 拡張: doc-first dependency order の structural enforcement | C1-C3 / D1-D2 / E1-E3 | E3 真の ideal | **`spec-stage-adversarial-checklist.md` Rule 4 wording 拡張**: PRD 内 doc update task は code 改修 task の **prerequisite** として位置付ける必須 dependency 制約 (= single source of truth の structural 維持)。逆方向 dependency (= doc が code 後 sync) は Rule 4 違反。`scripts/audit-prd-rule10-compliance.py` で Task List dependency chain を parse し、doc update task ID が code 改修 task ID の Depends on に存在することを auto verify (= structural enforcement、人手判断介在排除)。 | ✗ 要 fix (Rule 4 framework 改修、Q6) | 本 PRD (Q6、本 PRD draft 自体の Rule 4 violation = T11 dependency が C1 修正前 wording で残存していた self-evidence、3 度目 review で検出) |

判定凡例: ✓ (現状 OK、regression lock-in test) / ✗ (修正必要、本 PRD scope) / NA (unreachable + reason 明示) / 別 PRD (I-201-A / I-201-B / I-202 / I-203)

### Cross-cutting Invariants (`spec-stage-adversarial-checklist.md` Rule 8)

| INV# | Property statement | Justification | Verification | Failure detectability |
|------|------------------|---------------|--------------|---------------------|
| INV-1 | doc/grammar/ast-variants.md の各 enum section の Tier 1 / Tier 2 分類が、code の handle 状況と完全 sync する | doc-code drift で進捗評価が ground truth でなくなる、新 variant 追加時の silent miss risk | `scripts/audit-ast-variant-coverage.py` を CI 化 | CI fail (compile error ではなく audit fail) |
| INV-2 | 全 enum match 文で `_ => ` arm を使用しない (= 全 variant explicit enumerate) | silent drop の structural 排除、AST evolution 安全性 | Rust 言語 spec exhaustiveness check (compile error) + `scripts/audit-ast-variant-coverage.py` で `_ =>` 使用箇所 audit | compile error (`_` arm 不在で全 variant 必須) + CI audit fail |
| INV-3 | 全 PRD doc に Rule 10 application section が存在し、structural reason が prohibited keywords を含まない | Rule 10 application の妥協の逃げ道排除、進捗評価 ground truth | `scripts/audit-prd-rule10-compliance.py` を CI 化 | CI fail (PRD merge 不能) |
| INV-4 | `prd-template` skill 起動時、Rule 10 application section が空のまま skill が終了しない | PRD 起票時の Rule 10 application 必須化 | skill workflow 内 verification step | skill execution failure |

### Spec-Stage Adversarial Review Checklist (10-rule)

`spec-stage-adversarial-checklist.md` の 10-rule を本 PRD 内で全項目 verification:

- [x] **Rule 1: Matrix completeness**: 全 27 cell (cell 1-25 + cell 26 (`UnsupportedSyntaxError` 統一) + cell 27 (TypeResolver `_` arm 明示 no-op 化)、D3 修正 2026-04-27) に ideal output 記載 + 判定 (✓/✗/NA) + Scope 確定。空欄 / TBD なし。
- [x] **Rule 2: Oracle grounding**: 各 ✗ cell の ideal output が以下 source で grounding:
  - cell 6/10/11/15 = tsc/tsx runtime stdout (`record-cell-oracle.sh` で 9 件 `.expected` 記録済 2026-04-27)
  - cell 7/12-14/17 = Tier 2 honest error の Transformer behavior (= existing `UnsupportedSyntaxError` mechanism + 既実装 `classes/mod.rs:165-171`)
  - cell 18-22 = `spec-stage-adversarial-checklist.md` Rule 10 wording の logical consistency + Q4/Q5 確定事項
  - cell 23-25 = SWC AST 定義 + TC39 spec (Decorator) + `feedback_no_dev_cost_judgment.md` (prohibited keyword list)
  - cell 26-27 = phase 別役割分担 (TypeResolver static analysis abort 不可 / Transformer 変換 phase) の `pipeline-integrity.md` 整合
- [x] **Rule 3: NA justification**: cell 15 (Prop::Assign) NA reason: 「TS spec で object literal context で parse error、destructuring default context (`ObjectAssignmentPattern`) 限定」(spec-traceable: TS lang spec + SWC parser empirical verify via Test 20)。
- [x] **Rule 4: Grammar consistency**: ast-variants.md の Prop section (現状不在 = Grammar gap) / AutoAccessor entry (現状 Tier 2、本 PRD で Q1 (b) 状態化明示) / Decorator entry (現状不在 = Grammar gap) を本 PRD T11 (= **T8/T9/T10 prerequisite として doc-first**、Action 1 修正 2026-04-27) で update。matrix に reference doc 未記載 variant が存在しない state を T11 完了で達成。
- [x] **Rule 5: E2E readiness**: T0 (Spec stage 内 task) で 9 fixture (cell 6/7/10/11/12/13/14/15/17) + 9 .expected 準備済 2026-04-27 (red 状態 sample verify: cell 6 で silent semantic change + E0423 compile error 顕在化、Implementation stage で T8 改修後 green 化)。
- [x] **Rule 6: Matrix/Design integrity**: matrix Ideal output と Design section emission strategy が token-level に一致 (cell 6 visit_block_stmt 経由 / cell 7 TypeResolver no-op + Transformer UnsupportedSyntaxError / cell 12-14 visit_method_function 同等処理 / cell 15 unreachable!() / cell 17 全 Prop variant explicit + UnsupportedSyntaxError 統一)。Design Section の 3.1 / 3.2 / 3.3 で side-by-side 確認可能。
- [x] **Rule 7: Control-flow exit sub-case completeness — NA (justification 明示)**: 本 PRD scope = **AST traversal coverage** (= TypeResolver / Transformer match 文の dispatch arm completeness) で、control-flow exit dimension (then_exits × else_exits = T/T, T/F, F/T, F/F の 4 sub-case) は **runtime narrow framework の dimension**。本 PRD の static AST traversal とは **構造的 orthogonal** (= AST traversal は parse-time / static-analysis、control-flow exit は runtime narrow emission の dimension)。NA reason は spec-traceable (= AST shape vs control flow の独立性)。
- [x] **Rule 8: Cross-cutting invariant enumeration**: INV-1〜INV-4 を独立 section ("Cross-cutting Invariants") で明示、各 INV について 4 必須項目を全記載:
  - INV-1: doc-code sync (Property: ast-variants.md ↔ code Tier 分類完全 sync / Justification: doc-code drift で進捗評価が ground truth でなくなる + 新 variant 追加時 silent miss risk / Verification: scripts/audit-ast-variant-coverage.py CI 化 / Failure detectability: CI fail audit error)
  - INV-2: 全 enum match 文 `_` arm 不在 (Property: Rust exhaustiveness check 期待状態 / Justification: silent drop の structural 排除 + AST evolution 安全性 / Verification: Rust compile error + audit script `_ =>` detect / Failure detectability: compile error + audit fail)
  - INV-3: 全 PRD doc Rule 10 application section 完全性 (Property: section 存在 + structural reason に prohibited keywords 不在 / Justification: 妥協の逃げ道排除 + 進捗評価 ground truth / Verification: scripts/audit-prd-rule10-compliance.py CI 化 / Failure detectability: CI fail = PRD merge 不能)
  - INV-4: prd-template skill 起動時 Rule 10 section 必須記入 (Property: skill workflow 内 verification step 不可避 / Justification: PRD 起票時の Rule 10 application 必須化 / Verification: skill instruction 内 audit script invocation step / Failure detectability: skill execution failure = PRD draft 不可)
- [x] **Rule 9: Dispatch-arm sub-case alignment — Spec→Impl + Impl→Spec 双方向 verify**:
  - **Spec→Impl**: matrix cell 1-9 (ClassMember 9 variant) ↔ visit_class_body の 9 arm = **1-to-1 確認** (post-T8 改修後)、matrix cell 10-15 (Prop 6 variant) ↔ ast::Expr::Object inner match の 6 arm = **1-to-1 確認** (post-T9 改修後)、Transformer convert_object_lit の Prop 6 arm = **1-to-1 確認** (post-T10 改修後)
  - **Impl→Spec**: T8/T9/T10 実装中に新 dispatch arm を発見した場合、`spec-first-prd.md` の「Spec への逆戻り」手順を発動 (= matrix cell 追加 + ideal output 確定 + ast-variants.md update)
  - 本 PRD core 自体が dispatch-arm sub-case alignment の structural enforcement (Q4 + Q5)、self-applied first-class adopter
- [x] **Rule 10: Cross-axis matrix completeness — 3 step procedure + 8 default axis applicability 完全 enumerate**:
  - **Step 1 Axis enumeration**: 5 layer (A-E) + Cross-axis 3 prompt (I 逆問題 / II 実装 dispatch trace / III 影響伝搬 chain)
  - **Step 2 Orthogonality verification**: Layer A (AST node iterate target) と Layer B (variant 現状処理) が独立 (= variant 自体と現状処理が orthogonal) / Layer C (doc spec) と Layer D (rule 適用範囲) が独立 / Layer E (enforcement mechanism) は他 4 layer の cross-cutting axis として独立
  - **Step 3 Cartesian product expansion**: 27 cell に展開 (matrix 全 cell 完全 enumerate)
  - **8 default axis applicability**: (a) Layer A applicable / (b) matrix cell 1-15 applicable / (c)-(g) NA (= AST traversal coverage と orthogonal、上記 "## Rule 10 Application" section の table 参照、Action 2 修正 2026-04-27 で全 NA reason explicit 化) / (h) Rule 7 で NA 明示済

### Rule 10 Application (Q5 mandatory section)

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - "Layer A: AST node iterate target (ClassMember / PropOrSpread / Prop)"
  - "Layer B: variant 現状処理 (visited / silent drop / 暗黙 silent drop / 経路不在)"
  - "Layer C: ast-variants.md spec (Tier 1 / Tier 2 / Section 不在)"
  - "Layer D: Rule 10 適用範囲 (matrix-driven only / 全 PRD Mandatory)"
  - "Layer E: enforcement mechanism (doc only / skill hard-code / audit script + CI)"
Cross-axis orthogonal direction enumerated: yes
Cross-axis orthogonal directions:
  - "(I) 逆問題視点: structural enforcement の対立 = 人間判断介在 (= Anti-pattern として明示禁止)"
  - "(II) 実装 dispatch trace: ClassMember + PropOrSpread + Prop variant の全 dispatch"
  - "(III) 影響伝搬 chain: silent drop → 進捗評価 ground truth 失墜 → ideal 違反"
Structural reason for matrix absence: "N/A (matrix-driven PRD)"
```

---

## Goal

完了時に達成される observable behavior:

1. **`spec-stage-adversarial-checklist.md` Rule 10 が真の ideal wording で update 済**:
   - sub-rule (d): `_` arm 全面禁止 + 共通 macro + audit script + CI 化
   - sub-rule (e): Mandatory 化 + structural reason 明示 + Cross-axis 軸独立 enumerate + machine-parseable format + skill hard-code + audit script CI merge gate

2. **`prd-template` skill に Rule 10 application 必須 section が hard-code 済**: PRD 起票時 Step 0a / 0b で必須記入、空のまま skill 終了不可

3. **`scripts/audit-ast-variant-coverage.py` 新規作成 + CI 化 + merge gate 設定済**: ast-variants.md と code の handle 状況 sync を自動 verify

4. **`scripts/audit-prd-rule10-compliance.py` 新規作成 + CI 化 + merge gate 設定済**: backlog/*.md を parse + Rule 10 application section 存在 + prohibited keywords 不在を自動 verify

5. **TypeResolver `visit_class_body` の StaticBlock visit 追加** (cell 6) + ClassMember 全 variant explicit enumerate (cell 8 / 9 = TsIndexSignature / Empty を明示 no-op + reason comment 化、cell 27 で C4 統合) + AutoAccessor を TypeResolver 明示 no-op (cell 7、Transformer 既存 `UnsupportedSyntaxError` 維持、D4 修正 2026-04-27)

6. **TypeResolver expressions.rs:331+ Object expr の Prop visit を全 variant explicit enumerate** (cell 10-15) + Prop::Method/Getter/Setter body resolve 経路追加 (cell 12-14) + Prop::Assign は `unreachable!()` macro 呼び出し (cell 15、NA cell の bug detection mechanism、D5 修正 2026-04-27)

7. **Transformer convert_object_lit (data_literals.rs) の Prop arm を全 variant explicit enumerate** (cell 17) + `_ => Err(anyhow!(...))` を `UnsupportedSyntaxError::new("Prop::*", span)` 経由に format 統一 (C1 + C6) + Prop::Assign は `unreachable!()` (C3)

8. **`doc/grammar/ast-variants.md` に Prop section 新規追加** (全 7 variant Tier 分類) + AutoAccessor entry update + Decorator entry 新規追加

9. **既存 `UnsupportedSyntaxError` mechanism (`src/transformer/mod.rs:193-219`) を全 Transformer Tier 2 variant arm で統一適用** (C1 修正、新規 macro 作成は不要、format 統一 + DRY 達成) + **NA cell は `unreachable!()` macro 呼び出し** (C3 修正、bug detection mechanism)

10. **本 PRD 自体が Rule 10 application section + audit script を first-class adopter として self-applied**

11. **regression lock-in tests + SWC parser empirical test (cell 15) を全 ✓ NA cell に配置**

12. **Hono bench 0 regression** (clean 111 / errors 63 unchanged)、cargo test 全 pass、clippy 0 warning、fmt 0 diff

13. **`spec-stage-adversarial-checklist.md` Rule 4 wording 拡張 (Q6、Action 5 修正 2026-04-27)**: doc update task が code 改修 task の prerequisite という **doc-first dependency order を structural enforcement** + `scripts/audit-prd-rule10-compliance.py` で Task List dependency chain を auto verify (= 人手判断介在排除、本 PRD draft 自体の Rule 4 violation を 3 度目 review で発見した Spec gap の framework 改善検討の本質対応)

---

## Scope

### In Scope

- **Framework rule update** (Q4 + Q5):
  - `spec-stage-adversarial-checklist.md` Rule 10 全面 update (sub-rule (d) + (e) 追加)
  - `prd-template` skill の Step 0a / 0b に Rule 10 application 必須 section hard-code
  - `problem-space-analysis.md` cross-axis enumeration の non-matrix-driven 適用 spec 追加
- **Structural enforcement mechanism** (Q4 + Q5):
  - `scripts/audit-ast-variant-coverage.py` 新規作成 + CI 化 + merge gate
  - `scripts/audit-prd-rule10-compliance.py` 新規作成 + CI 化 + merge gate
  - `.github/workflows/ci.yml` audit script step 追加
  - 既存 `UnsupportedSyntaxError` mechanism format 統一 + 適用拡張 (`src/transformer/mod.rs:193-219` 既定義、新規 macro 作成は不要、C1 修正)
- **TypeResolver coverage extension** (I-199 + I-200):
  - `src/pipeline/type_resolver/visitors.rs::visit_class_body` の StaticBlock visit 追加 + ClassMember 全 variant explicit enumerate
  - `src/pipeline/type_resolver/expressions.rs:331+ ast::Expr::Object` arm の Prop 全 variant explicit enumerate + Prop::Method/Getter/Setter body resolve 経路追加 (visit_method_function 同等処理)
- **Transformer coverage extension** (Q4 application):
  - `src/transformer/expressions/data_literals.rs::convert_object_lit` の Prop 全 variant explicit enumerate
- **Documentation update**:
  - `doc/grammar/ast-variants.md` Prop section 新規追加 + AutoAccessor entry update + Decorator entry 新規追加
- **Test additions**:
  - regression lock-in tests for ✓ cells (cell 1-5, 8-11)
  - new feature tests for ✗ → ✓ cells (cell 6, 12-14)
  - SWC parser empirical regression test for NA cell (cell 15)
  - audit script tests (cell 21, 22)

### Out of Scope (別 PRD で対応、(d) 構造分離 pattern)

- **AutoAccessor 完全 Tier 1 化** (Rust 等価 emission、struct field + getter/setter pair) → **I-201-A** (decorator なし subset、L3、PRD 2.8) + **I-201-B** (decorator framework 全体、L1、PRD 7)
- **Object literal Prop::Method/Getter/Setter Tier 1 化** (Transformer 完全 emission、anonymous struct strategy) → **I-202** (L3、PRD 2.9)
- **Codebase 全体の `_` arm refactor** (本 PRD で touch 範囲外の既存 `_` arm) → **I-203** (audit driven priority、PRD 2.7 batch close 後の早期 audit)
- **decorator framework 完全変換** (TC39 Stage 3、init/get/set/addInitializer hook) → **I-201-B** (L1 silent semantic change、PRD 7)
- **PRD 3 (I-177 mutation propagation 本体)** → 本 PRD close 後の Plan η chain で進行

---

## Design

### Technical Approach

#### 1. Framework rule update (Q4 + Q5)

##### 1.1 `spec-stage-adversarial-checklist.md` Rule 10 sub-rule (d) 追加 (Q4)

```markdown
- [ ] **AST node enumerate completeness check** (Q4 真の ideal):
      Rust source 内の **全 enum match 文** で以下を verification:
      - **(d-1)** **`_ => ` arm の使用を全面禁止** (compile time exhaustiveness、新 variant 追加時 compile error で全 dispatch fix 強制)
      - **(d-2)** 未対応 variant も explicit enumerate、**phase 別 mechanism で structural enforcement** (C4 修正 2026-04-27):
        - **Transformer (変換 phase)**: 既存 `UnsupportedSyntaxError::new(kind, span)` mechanism (`src/transformer/mod.rs:193-219`) で error return 必須。format 統一 + production-friendly Err return (panic / todo! ではない)、user-facing transparent error は `src/lib.rs:96-97 resolve_unsupported()` 経由 line/col 含む
        - **TypeResolver (静的解析 phase)**: abort 不可 (= 全 conversion path 早期 abort = ideal 違反のため `UnsupportedSyntaxError` 呼出 不可)、**明示 no-op (reason comment 付き empty arm) で記述**。例: `ast::ClassMember::TsIndexSignature(_) => { /* Tier 2 filter out: 型 only、runtime effect なし、no-op (reason: ast-variants.md Tier 2 filter out 参照) */ }`
        - **NA cell (= structurally unreachable)**: `unreachable!()` macro 呼び出し (= bug detection mechanism、AST parser reject 前提の defensive coding。例: SWC parser が object literal context で `Prop::Assign` を reject、もし fire したら parser 仕様変更 bug)
      - **(d-3)** `doc/grammar/ast-variants.md` を **single source of truth** とし、各 enum section の Tier 1 (Handled) / Tier 2 (Unsupported) 分類が code の handle 状況と sync
      - **(d-4)** `scripts/audit-ast-variant-coverage.py` を CI 化、doc-code sync を自動検証 (新 variant 追加時 audit fail で fix 強制、merge gate)
      Lesson source: I-177-F (ClassMember variants 全 enumerate せず static block / auto accessor 漏れ)、I-200 (ObjectLit Prop variants 全 enumerate せず Prop::Method の type-resolve 経路漏れ)
```

##### 1.2 Rule 10 全体 Mandatory 化 (Q5)

```markdown
- [ ] **Rule 10 Mandatory application** (Q5 真の ideal):
      - **(e-1)**: 全 PRD で Rule 10 verification を Mandatory (matrix-driven / non-matrix-driven 区別なし)
      - **(e-2)**: Matrix-driven PRD = 8 default check axis enumerate + Cartesian product matrix 構築
      - **(e-3)**: Non-matrix-driven PRD = matrix 不在の structural reason を spec-traceable に明示
        - **Permitted reasons**: infra で AST input dimension irrelevant / refactor で機能 emission decision なし / pure doc 改修
        - **Prohibited reasons (Anti-pattern、明示禁止 list)**: 「scope 小」「light spec」「pragmatic」「~LOC」「短時間」「manageable」「effort 大」 (`feedback_no_dev_cost_judgment.md` 違反)
      - **(e-4)**: matrix 不在でも Cross-axis 直交軸 (解決軸 symmetric / 対称ケース / context 軸) の独立 enumerate 必須
      - **(e-5)**: structural reason を machine-parseable format で記入 (heading-based 構造 `## Rule 10 Application` / `Matrix-driven: yes/no` / `Structural reason for matrix absence: <reason>`)
      - **(e-6)**: `prd-template` skill に Rule 10 application 必須 section を hard-code (空のまま skill 終了不可)
      - **(e-7)**: `scripts/audit-prd-rule10-compliance.py` を CI 化 + merge gate (PRD doc parse + Rule 10 application section + prohibited keywords 不在 verify)
      - **(e-8)**: `/check_job` Layer 3 (Structural cross-axis) に Rule 10 application verification を integrate
```

##### 1.2.5 `spec-stage-adversarial-checklist.md` Rule 4 wording 拡張 (Q6、Action 5 修正 2026-04-27)

Rule 4 を以下のように拡張 (元 wording 「matrix に reference doc に未記載の variant が存在しない (存在すれば reference doc を先に更新)」に追加):

```markdown
- [ ] **Grammar consistency + doc-first dependency order の structural enforcement (Q6 修正 2026-04-27)**:
      - **(4-1)** matrix に reference doc に未記載の variant が存在しない (存在すれば reference doc を先に更新)
      - **(4-2、新規追加 Q6)**: PRD 内 doc update task (= ast-variants.md / 関連 reference doc 更新 task) は **code 改修 task (= TypeResolver / Transformer / Generator 改修 task) の prerequisite** として位置付ける必須 dependency 制約。code 改修が doc を ground truth として参照する **単方向 dependency** (= doc-first)、doc を code 後に sync する **逆方向 dependency** は **Rule 4 違反** (= single source of truth の structural 違反、Q4 整合)。
      - **(4-3、新規追加 Q6)** Verification mechanism: `scripts/audit-prd-rule10-compliance.py` (T6) で Task List section を parse し、以下を auto verify:
        - PRD doc 内 task の Depends on / Prerequisites を抽出
        - doc update task ID (= ast-variants.md / 関連 reference doc 更新を含む task) を identify
        - code 改修 task ID (= src/ 配下の Rust source 改修を含む task) を identify
        - 各 code 改修 task の Prerequisites に doc update task ID が存在することを check (= doc-first verify)
        - 不在時 audit fail (CI fail = PRD merge 不能)
      - **Lesson source**: PRD 2.7 draft 自体で T11 (`ast-variants.md` update) が T8/T9/T10 (code 改修) の **後** に位置していた = Q4 (single source of truth、INV-1) 違反。1 度目 review + 2 度目 review で未検出、3 度目 `/check_job` review (2026-04-27) で初めて Spec gap として発覚。framework 失敗 signal を起点に Rule 4 wording に doc-first dependency order の structural enforcement を追加 (本 PRD 自体が first-class adopter)。
```

##### 1.3 `prd-template` skill update

Step 0a / 0b に Rule 10 application 必須 section を追加:

```markdown
### 0c. Rule 10 Application (Mandatory、Q5 確定 2026-04-27)

PRD doc 内に以下 section を必須記入:

\`\`\`yaml
Matrix-driven: yes/no
Rule 10 axes enumerated:
  - <axis 1>
  - <axis 2>
  - ...
Cross-axis orthogonal direction enumerated: yes/no
Structural reason for matrix absence: <reason、Permitted reasons から選択 or N/A>
\`\`\`

skill 起動時、上記 section が空のまま skill が終了しない (verification step を追加)。
```

#### 2. Structural enforcement mechanism (Q4 + Q5)

##### 2.1 既存 `UnsupportedSyntaxError` mechanism の format 統一 audit + 拡張 (C1 修正 2026-04-27)

**audit 結果 (Step 3 Impact Area Code Review、第三者 review 2026-04-27)**: ts_to_rs codebase は **既に `UnsupportedSyntaxError` mechanism を実装済**:

- **定義**: `src/transformer/mod.rs:193-219` (struct + `Display` + `std::error::Error` trait impl、signature `UnsupportedSyntaxError::new(kind: &str, span: swc Span)`)
- **既存 use 箇所**: `classes/mod.rs:166` (AutoAccessor)、`statements/nullish_assign.rs:147/183/197/256`、`statements/mod.rs:41`、`functions/destructuring.rs:253/267` 等多数
- **Resolution path**: `src/lib.rs:96-97 resolve_unsupported()` で `UnsupportedSyntax` (line/col 含む user-facing transparent error) に変換
- **Tier 2 honest error mechanism として established**

→ **新規 macro `unsupported_arm!()` 作成は重複** (= DRY violation = `design-integrity.md` higher-level consistency 違反 = ideal 違反)。

**修正方針 (本 PRD 2.7 で確立)**: **既存 `UnsupportedSyntaxError` を全 Transformer Tier 2 variant arm で統一適用** + 一部 module の format 不整合 (例: `data_literals.rs:259-263` の `anyhow!()` 経由) を `UnsupportedSyntaxError` に統一:

```rust
// AS-IS (data_literals.rs:259-263、format 不整合の broken window):
_ => return Err(anyhow!("unsupported object literal property")),

// TO-BE (Q4 application + format 統一、本 PRD 2.7 で確立):
ast::Prop::Method(method_prop) => return Err(
    UnsupportedSyntaxError::new("Prop::Method", method_prop.function.span).into()
),
ast::Prop::Getter(getter_prop) => return Err(
    UnsupportedSyntaxError::new("Prop::Getter", getter_prop.span).into()
),
ast::Prop::Setter(setter_prop) => return Err(
    UnsupportedSyntaxError::new("Prop::Setter", setter_prop.span).into()
),
ast::Prop::Assign(assign_prop) => unreachable!(
    "Prop::Assign in object literal context: SWC parser should reject (NA cell, see PRD 2.7 cell 15). \
     If this fires, SWC parser behavior changed — investigate immediately."
),
// No `_ => ...` arm — Rule 10(d-1) compliance
```

**ideal の三観点**:
- Production-friendly Err return (panic / todo! ではない、user-facing transparent error は `Resolution path` 経由の line/col 含む)
- DRY 達成 (既存 mechanism re-use、新規 macro 不要)
- Format 統一 (全 Transformer Tier 2 error が `UnsupportedSyntaxError` 経由)

**TypeResolver layer の役割分担 (C4 修正 2026-04-27)**: TypeResolver (静的解析 phase) は abort 不可 (= 全 conversion path 早期 abort = ideal 違反のため `UnsupportedSyntaxError` 呼出 不可)。Tier 2 variant の TypeResolver arm は **明示 no-op + reason comment 付き empty arm** で記述 (Rule 10(d-2) compliance、下記 Rule 10 wording 参照)。Tier 2 honest error は **Transformer (変換 phase) で `UnsupportedSyntaxError` を return** する設計に統一 (= pipeline-integrity.md 整合)。

##### 2.2 `scripts/audit-ast-variant-coverage.py` 新規作成

機能:
1. ts_to_rs codebase 全 Rust source の AST match 文を AST parse (e.g., `syn` crate 経由 or regex)
2. 各 match 文の dispatch enum を識別、handle 済 variant を enumerate
3. `doc/grammar/ast-variants.md` の対応 section を parse、Tier 1 (Handled) / Tier 2 (Unsupported) variant を enumerate
4. doc と code の sync 状態を verify:
   - doc Tier 1 variant が code で全 explicit enumerate されているか
   - code の `UnsupportedSyntaxError::new()` 呼び出し variant が doc Tier 2 と sync しているか
   - `_` arm 使用箇所が存在しないか (Rule 10(d-1) compliance)
5. 不一致検出時 audit fail (= CI fail = merge gate)

##### 2.3 `scripts/audit-prd-rule10-compliance.py` 新規作成

機能:
1. `backlog/*.md` を parse
2. 各 PRD doc に `## Rule 10 Application` section が存在するか check
3. machine-parseable format (heading + key: value) を parse
4. `Matrix-driven: yes/no` の値が valid か check
5. `Structural reason for matrix absence: <reason>` が prohibited keywords を含まないか check
6. 不一致検出時 audit fail

##### 2.4 CI integration (`.github/workflows/ci.yml` update)

```yaml
- name: Audit AST variant coverage
  run: python3 scripts/audit-ast-variant-coverage.py

- name: Audit PRD Rule 10 compliance
  run: python3 scripts/audit-prd-rule10-compliance.py
```

両者を merge gate として設定 (= PR merge 前に必須 pass)。

#### 3. TypeResolver coverage extension (I-199 + I-200)

##### 3.1 visitors.rs::visit_class_body の改修 (I-199 + Rule 10(d) application)

```rust
for member in &class.body {
    match member {
        ast::ClassMember::Method(m) => { /* 現状維持 */ }
        ast::ClassMember::PrivateMethod(pm) => { /* 現状維持 */ }
        ast::ClassMember::Constructor(ctor) => { /* 現状維持 */ }
        ast::ClassMember::ClassProp(prop) => { /* 現状維持 */ }
        ast::ClassMember::PrivateProp(pp) => { /* 現状維持 */ }
        ast::ClassMember::StaticBlock(sb) => {
            // I-199 新規: I-177-F symmetric (visit_block_stmt 経由 + scope 管理)
            self.enter_scope();
            let param_pats: Vec<&ast::Pat> = vec![];
            self.collect_emission_hints(&sb.body, &param_pats);
            self.visit_block_stmt(&sb.body);
            self.leave_scope();
        }
        ast::ClassMember::AutoAccessor(_) => {
            // Q1 (b) Tier 2 error report 化 (C1 + C2 + C4 修正 2026-04-27): 完全 Tier 1 化は I-201-A/B 別 PRD
            // TypeResolver (静的解析 phase) は abort 不可 (= 全 conversion path 早期 abort = ideal 違反)、
            // 明示 no-op (Rule 10(d-2) compliance、reason comment 付き empty arm) で記述。
            // Transformer (変換 phase) は既存 `UnsupportedSyntaxError::new("AutoAccessor", aa.span)` で
            // honest error return (`src/transformer/classes/mod.rs:165-171` 既実装、現状維持)。
            // ast-variants.md AutoAccessor entry に Tier 2 (Unsupported, error reported via UnsupportedSyntaxError) と明記。
        }
        ast::ClassMember::TsIndexSignature(_) => {
            // Tier 2 filter out (型 only、runtime effect なし、no-op で正)
            // ast-variants.md に "filter out reason" 明記
        }
        ast::ClassMember::Empty(_) => {
            // Tier 2 no-op (空 member、no-op で正)
            // ast-variants.md に "no-op reason" 明記
        }
        // No `_ => ...` arm — Rule 10(d-1) compliance
    }
}
```

**Note (C4 修正 2026-04-27)**: TypeResolver は静的解析 phase で、`UnsupportedSyntaxError` 等を呼ぶと全 conversion path が abort する (= ideal 違反)。Tier 2 variant は **TypeResolver では no-op (reason comment 付き empty arm) + Transformer で `UnsupportedSyntaxError` 経由 error return** という pattern が natural (= Transformer が actual 変換 path、error 出すべき layer)。これは `pipeline-integrity.md` (transformer/generator 分離) + Rule 10(d-2) phase 別役割分担と整合。

##### 3.2 expressions.rs::ast::Expr::Object の改修 (I-200 + Rule 10(d) application)

```rust
ast::Expr::Object(obj) => {
    let mut explicit_fields: Vec<(String, RustType)> = Vec::new();
    let mut spread_types: Vec<RustType> = Vec::new();
    let mut total_explicit_props = 0u32;

    for prop in &obj.props {
        match prop {
            ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                ast::Prop::KeyValue(kv) => { /* 現状維持 */ }
                ast::Prop::Shorthand(ident) => { /* 現状維持 */ }
                ast::Prop::Method(method_prop) => {
                    // I-200 新規: visit_method_function 同等処理 (function-level scope + visit_block_stmt)
                    total_explicit_props += 1;
                    let span = Span::from_swc(method_prop.function.span);
                    self.visit_function_body_for_prop_method(&method_prop.function, span);
                    // type info 記録 (= function type で expr_types に insert)
                }
                ast::Prop::Getter(getter_prop) => {
                    // I-200 新規: getter body を visit_block_stmt 経由 walk
                    total_explicit_props += 1;
                    if let Some(body) = &getter_prop.body {
                        self.enter_scope();
                        self.visit_block_stmt(body);
                        self.leave_scope();
                    }
                }
                ast::Prop::Setter(setter_prop) => {
                    // I-200 新規: setter body を visit_block_stmt 経由 walk
                    total_explicit_props += 1;
                    if let Some(body) = &setter_prop.body {
                        self.enter_scope();
                        self.visit_param_pat(&setter_prop.param);
                        self.visit_block_stmt(body);
                        self.leave_scope();
                    }
                }
                ast::Prop::Assign(_) => unreachable!(
                    "Prop::Assign in object literal context: SWC parser should reject \
                     (NA cell, see PRD 2.7 cell 15 + Test 20). If this fires, SWC parser \
                     behavior changed — investigate immediately."
                ),
                // No `_ => ...` arm — Rule 10(d-1) compliance
            },
            ast::PropOrSpread::Spread(spread) => { /* 現状維持 */ }
        }
    }
    // ... rest of Object handling
}
```

**C3 修正 (2026-04-27)**: 以前 draft の `total_explicit_props += 1` defensive coding は **silent drop の延長** (count++ 以外の意味のある action なし、type info / narrow event 不在)。NA cell (= structurally unreachable) は `unreachable!()` macro が ideal:
- Rust の `unreachable!()` macro は実行時 panic + bug indicator
- SWC parser が actual reject する前提で defensive (= もし fire したら SWC parser 仕様変更 = bug、即時 investigate 必要)
- Test 20 (SWC parser empirical regression lock-in test) で actual reject を verify、`unreachable!()` の precondition を保証

##### 3.3 Transformer data_literals.rs::convert_object_lit の改修 (Q4 application)

```rust
for prop in &obj_lit.props {
    match prop {
        ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
            ast::Prop::KeyValue(kv) => { /* 現状維持 */ }
            ast::Prop::Shorthand(ident) => { /* 現状維持 */ }
            ast::Prop::Method(method_prop) => {
                return Err(UnsupportedSyntaxError::new("Prop::Method", method_prop.function.span).into())
            }
            ast::Prop::Getter(getter_prop) => {
                return Err(UnsupportedSyntaxError::new("Prop::Getter", getter_prop.span).into())
            }
            ast::Prop::Setter(setter_prop) => {
                return Err(UnsupportedSyntaxError::new("Prop::Setter", setter_prop.span).into())
            }
            ast::Prop::Assign(_) => unreachable!(
                "Prop::Assign in object literal context: SWC parser should reject (NA cell, see PRD 2.7 cell 15 + Test 20). \
                 If this fires, SWC parser behavior changed — investigate immediately."
            ),
            // No `_ => ...` arm — Rule 10(d-1) compliance
        },
        ast::PropOrSpread::Spread(spread_elem) => { /* 現状維持 */ }
    }
}
```

#### 4. Documentation update

##### 4.1 `doc/grammar/ast-variants.md` Prop section 新規追加

```markdown
## NN. Prop (オブジェクトリテラルプロパティ)

### Tier 1 — Handled

| Variant | 処理 |
|---------|------|
| `KeyValue` | `key: value` 形式 (TypeResolver: 値 type-resolve、Transformer: struct field assign or HashMap entry) |
| `Shorthand` | `{ x }` 短縮形 (= `{ x: x }`) (TypeResolver: var lookup、Transformer: struct field assign) |
| `Method` (TypeResolver visit only) | `{ method() {...} }` (TypeResolver: method body visit_block_stmt 経由 walk; Transformer: 完全 Tier 1 化は **I-202** で実施、現状 Tier 2 error report) |
| `Getter` (TypeResolver visit only) | `{ get name() {...} }` (TypeResolver: body walk; Transformer: 同上 I-202) |
| `Setter` (TypeResolver visit only) | `{ set name(v) {...} }` (TypeResolver: body walk; Transformer: 同上 I-202) |

### Tier 2 — Unsupported / NA

| Variant | Status | 備考 |
|---------|--------|------|
| `Method` (Transformer) | Tier 2 (Unsupported, error reported via `UnsupportedSyntaxError`) | I-202 で Tier 1 化予定 |
| `Getter` (Transformer) | Tier 2 (同上) | 同上 |
| `Setter` (Transformer) | Tier 2 (同上) | 同上 |
| `Assign` | NA (parse error context) | TS spec で object literal context で parse error、destructuring default context (`({ x = 1 } = obj)`) のみ valid。`ObjectPatProp::Assign` で別経路 handle |
```

##### 4.2 AutoAccessor entry update

`## 14. ClassMember (クラスメンバー)` section の Tier 2 table:

```markdown
| `AutoAccessor` | Tier 2 (Unsupported, error reported) | TS 5.0+ stable AutoAccessor (`accessor x: T = init`)、I-201-A (decorator なし subset、Tier 1 化) + I-201-B (decorator framework、Tier 1 化) で Tier 1 昇格予定 |
```

##### 4.3 Decorator entry 新規追加

```markdown
## NN. Decorator (デコレータ、TC39 Stage 3 / TS 5.0+ stable)

### Tier 2 — Unsupported

| Variant | Status | 備考 |
|---------|--------|------|
| `Decorator` (`@dec`) | Tier 2 (Unsupported, error reported) | ts_to_rs では未実装 (audit 2026-04-27)、I-201-B で Tier 1 化予定 (init/get/set/addInitializer hook の Rust 等価表現確立) |
```

### Design Integrity Review

`design-integrity.md` checklist:

1. **Higher-level consistency**:
   - PRD 2.7 は **framework layer の改修** (rule + skill + audit script + CI) と **TypeResolver/Transformer の coverage extension** を同時実施
   - 両者は同 architectural concern「framework Rule 10 拡張 + 拡張による coverage gap detection 完成 + structural enforcement」で cohesive
   - 実装 layer (TypeResolver / Transformer) の改修は framework layer (Rule 10) の direct application で higher-level consistency 達成
2. **DRY (knowledge duplication)**:
   - 既存 `UnsupportedSyntaxError` mechanism で「unsupported variant の Err return format」を 1 箇所に集約 (`src/transformer/mod.rs:193-219` 定義 + 全 codebase で reuse、DRY 達成、新規 macro 作成不要)
   - ast-variants.md = single source of truth で Tier 分類を 1 箇所に集約 (DRY 達成)
3. **Orthogonality**:
   - TypeResolver coverage (静的解析 phase) と Transformer coverage (変換 phase) は pipeline-integrity.md 通りに分離
   - Rule 10 改修 (framework layer) と application (実装 layer) は責務分離
4. **Coupling**:
   - audit script は ast-variants.md と code を直接参照、TypeResolver / Transformer 内部 detail には depend しない (loose coupling)
5. **Broken windows**:
   - 既存 codebase 全体の `_` arm 使用箇所は本 PRD scope 外 (= I-203 別 PRD)、broken window 認識済 + (d) 構造分離で対応

→ **Verified, no remaining issues** (broken windows は I-203 で対応)。

### Impact Area

| File | Operation | 内容 |
|------|-----------|------|
| `.claude/rules/spec-stage-adversarial-checklist.md` | Edit | Rule 10 全面 update (sub-rule (d) + (e) 追加) |
| `.claude/rules/problem-space-analysis.md` | Edit | cross-axis enumeration の non-matrix-driven 適用 spec 追加 |
| `.claude/skills/prd-template/SKILL.md` | Edit | Step 0a / 0b に Rule 10 application 必須 section hard-code |
| `src/pipeline/type_resolver/visitors.rs` | Edit | `visit_class_body` の StaticBlock visit 追加 + ClassMember 全 variant explicit enumerate (`_` arm 削除、Tier 2 variant は明示 no-op + reason comment) |
| `src/pipeline/type_resolver/expressions.rs` | Edit | `ast::Expr::Object` arm の Prop 全 variant explicit enumerate + Prop::Method/Getter/Setter body visit (visit_method_function 同等処理) + Prop::Assign は `unreachable!()` (NA cell、C3) + 暗黙 silent drop `_ => { total_explicit_props += 1; }` 削除 (cell 16) |
| `src/transformer/expressions/data_literals.rs` | Edit | `convert_object_lit` の `_ => Err(anyhow!(...))` を全 Prop variant explicit enumerate + `UnsupportedSyntaxError::new("Prop::*", span)` 経由 error return に format 統一 (broken window 解消、cell 17、C1 + C6 統合) + Prop::Assign は `unreachable!()` (cell 15、C3) |
| `doc/grammar/ast-variants.md` | Edit | Prop section 新規追加 + AutoAccessor entry update + Decorator entry 新規追加 |
| `scripts/audit-ast-variant-coverage.py` | Write | 新規 audit script (Q4 ground truth verification) |
| `scripts/audit-prd-rule10-compliance.py` | Write | 新規 audit script (Q5 ground truth verification) |
| `.github/workflows/ci.yml` | Edit | audit script step 追加 + merge gate 設定 |
| `src/pipeline/type_resolver/tests/*.rs` | Edit | regression / new feature unit tests 追加 |
| `src/transformer/expressions/tests/*.rs` | Edit | regression / new feature unit tests 追加 |
| `tests/e2e/scripts/prd-2.7/` (新規 dir) | Write | E2E fixtures (Prop::Method/Getter/Setter body resolve cell の lock-in) |

### Semantic Safety Analysis (M6 修正 2026-04-27 depth 強化)

`type-fallback-safety.md` の 3-step 適用 + silent drop 解消の semantic safety verify mechanism:

#### 1. Type fallback patterns

本 PRD は **型 fallback / 型 approximation / 型 resolution 変更を導入しない**:
- TypeResolver visit 経路追加のみ (cell 6 / 12-14)、型 inference logic は変更なし
- Transformer Tier 2 error format 統一 (cell 17、`anyhow!()` → `UnsupportedSyntaxError`) は error message format 改善のみ、conversion 出力は変更なし
- → **`type-fallback-safety.md` 3-step Verdict**: Not applicable (no type fallback changes)

#### 2. Silent drop 解消の semantic 影響分析 (本 PRD core)

silent drop は型 fallback ではないが、**conversion 結果に semantic 影響**があるため独立に分析:

| Cell | Pre-PRD 2.7 | Post-PRD 2.7 | semantic 影響 |
|------|-----------|-----------|--------------|
| cell 6 (StaticBlock) | TypeResolver で `_ => {}` 黙殺 = static block 内 typeof narrow event 不在 → Transformer で type info 不正確 → silent type widening or compile error | TypeResolver で visit_block_stmt 経由 walk = narrow event push → Transformer で type info 正確 | **silent → correct** (= ideal への更新、regression ではない) |
| cell 12-14 (Prop::Method/Getter/Setter) | TypeResolver expressions.rs:367-369 で `_ => { count++ }` 暗黙 silent drop = body 内 typeof narrow event 不在 → Transformer で error report (既 Tier 2 honest)、但し I-202 完了後の Tier 1 emission で silent type widening risk | TypeResolver で body visit = narrow event push → I-202 completion 時に正確な type info 提供 | **future-proofing**、本 PRD では Transformer 出力変化なし (TypeResolver 内部状態のみ変化) |
| cell 7 (AutoAccessor) | TypeResolver で `_ => {}` 黙殺 + Transformer で `UnsupportedSyntaxError` (既実装) | TypeResolver で明示 no-op (reason comment) + Transformer 既実装維持 | **conversion 出力変化なし** (TypeResolver 黙殺の明示化のみ、Transformer 既 Tier 2 honest 維持) |
| cell 17 (Transformer convert_object_lit) | `_ => Err(anyhow!("unsupported object literal property"))` (Tier 2 honest だが format 不整合) | `Prop::Method/Getter/Setter => Err(UnsupportedSyntaxError::new(...))` (format 統一) | **error message format のみ変化**、Err return 自体は維持 → **user-facing は line/col 含む transparent message に改善** (regression ではない、ideal への更新) |
| cell 15 (Prop::Assign) | TypeResolver `_ => { count++ }` + Transformer `_ => Err(anyhow!(...))` | 両 layer で `unreachable!()` panic | **actual reach 不能 (SWC parser reject)、test 20 で empirical verify**。もし fire したら parser 仕様変更 = bug detection mechanism (= ideal 違反検知) |

#### 3. Verification mechanism (silent drop 解消の semantic safety)

silent drop 解消による conversion 結果変化を以下 mechanism で structural verify:

1. **Per-cell E2E fixture diff (T0)**: 各 ✗ cell の TS fixture を pre/post-PRD で `cargo run -- <fixture.ts>` 出力 diff、tsc/tsx golden output と比較。silent → correct (cell 6) / 出力不変 (cell 7) / format 改善 (cell 17) / unreachable verify (cell 15) を per-cell に確認
2. **Hono bench 0 regression (T14)**: clean 111 ± 1 / errors 63 ± 2 範囲内 (bench 非決定性 [I-172] noise variance)。範囲外 = regression、即時 root cause 特定 + fix
3. **Audit script CI 化 (T5 + T7)**: doc-code sync verify で silent drop 復活を継続検出 (新 variant 追加時 audit fail)
4. **`/check_job` 4-layer review (T15)**: Layer 2 (Empirical) で per-cell stdout 確認、Layer 4 (Adversarial trade-off) で pre/post matrix 全 cell 評価

#### 4. Verdict (本 PRD)

- 型 fallback: **Not applicable** (no changes)
- Silent drop 解消: **Safe** (= ideal-implementation-primacy.md compliance、regression なし)
  - silent → correct (cell 6): user code の type info 正確化、TS と Rust の semantic 一致向上
  - error message format 改善 (cell 17): user-facing transparent error、regression なし
  - bug detection (cell 15): actual unreachable property を `unreachable!()` で trap、parser 仕様変更時の immediate detection
- 全 cell で silent semantic change なし (= `conversion-correctness-priority.md` Tier 1 violation なし)、**Hono bench + audit script + per-cell fixture で structural verify**

---

## Task List

### T0: Per-cell E2E fixture (red 状態) 作成 — Spec stage 完了 prerequisite (C5 修正 2026-04-27)

- **Work**: `spec-first-prd.md` Stage 1 artifact #3 必須要件として、本 PRD の全 ✗ cell に対応する TS fixture を `tests/e2e/scripts/prd-2.7/<cell-id>.ts` に作成。各 fixture を `scripts/observe-tsc.sh` に通して tsc/tsx 出力を観測、`scripts/record-cell-oracle.sh` で expected output を記録。Implementation stage 着手時点で全 fixture が **red 状態** (= ts_to_rs 変換結果が expected output と不一致 = matrix の "現状" 列が ✗) であることを確認。
- **対象 cell** (matrix の ✗ cell):
  - cell 6: `prd-2.7/cell-06-static-block-typeof-narrow.ts` (StaticBlock 内 typeof narrow が TypeResolver で event push されることを verify)
  - cell 7: `prd-2.7/cell-07-auto-accessor-honest-error.ts` (Transformer 既実装の `UnsupportedSyntaxError` regression lock-in)
  - cell 10-11: `prd-2.7/cell-10-prop-keyvalue-regression.ts` / `cell-11-prop-shorthand-regression.ts` (既存 KeyValue/Shorthand handle の regression)
  - cell 12-14: `prd-2.7/cell-12-prop-method-typeof-narrow.ts` / `cell-13-prop-getter-typeof-narrow.ts` / `cell-14-prop-setter-typeof-narrow.ts` (TypeResolver visit による narrow event push)
  - cell 15: `prd-2.7/cell-15-prop-assign-na.ts` (SWC parser reject の empirical verify、Test 20 と同じ)
  - cell 17: `prd-2.7/cell-17-transformer-convert-object-lit-error.ts` (Transformer Tier 2 honest error の format 統一後の regression)
- **Completion criteria**: 全 fixture (~7 件) 作成完了、tsc observation log 記録完了、各 fixture が red 状態であることを `cargo run -- <fixture.ts>` 出力 vs tsc 出力の diff で verify。Spec stage 完了 verification の必須 artifact。
- **Depends on**: なし (Spec stage 内 first task)
- **Prerequisites**: `tests/e2e/scripts/prd-2.7/` directory 新規作成 + `scripts/observe-tsc.sh` / `scripts/record-cell-oracle.sh` 既存

### T1: 既存 `UnsupportedSyntaxError` mechanism format 統一 audit + 適用拡張 (C1 修正 2026-04-27、新規 macro 作成は不要)

- **Work**:
  1. **Audit**: `grep -rn "UnsupportedSyntaxError" src/` で既存使用 site を全列挙、`anyhow!()` 経由の Tier 2 error (例: `data_literals.rs:259-263`) を audit
  2. **Format 統一 (C6 application)**: `data_literals.rs:259-263` の `Err(anyhow!("unsupported object literal property"))` を `Err(UnsupportedSyntaxError::new("kind", span).into())` 形式に統一 (broken window 解消)
  3. **適用拡張**: T9 (TypeResolver expressions.rs Object expr) は no-op の明示 (Rule 10(d-2) phase 別役割分担、TypeResolver は abort 不可)、T10 (Transformer convert_object_lit) で新規追加する Tier 2 arm を `UnsupportedSyntaxError` 経由に統一
  4. **NA cell**: cell 15 (Prop::Assign) は `unreachable!()` macro 呼び出し (C3 修正、TypeResolver expressions.rs + Transformer data_literals.rs 両方)
- **Completion criteria**: 既存 `UnsupportedSyntaxError` mechanism と整合、全 Transformer Tier 2 error が `UnsupportedSyntaxError` 経由 (format 統一)、NA cell は `unreachable!()`、unit test (Err return 確認 + `src/lib.rs:96-97 resolve_unsupported()` 経由 line/col 含む user-facing message 確認) 全 pass。新規 macro `unsupported_arm!()` は **作成しない** (C1 修正、DRY 違反回避)。
- **Depends on**: T0 (E2E fixture red 状態確認後)
- **Prerequisites**: なし

### T1.5: Decorator dispatch audit (C7 修正 2026-04-27)

- **Work**: ts_to_rs の TypeResolver / Transformer / narrowing_analyzer 等で `decorators` field (= `class_decl.class.decorators`、`function.decorators`、`method_prop.decorators` 等の SWC AST field) を touch する箇所を `grep -rn "decorators" src/` で全 audit:
  1. silent drop されている location (= `decorators` field を読まずに無視している場所) を全列挙
  2. 各 location について Decorator framework 未実装による silent semantic 影響を分析
  3. silent drop あれば本 PRD scope に編入 (Rule 10(d) application で fix)、Decorator framework 全実装は I-201-B (PRD 7) で達成
- **Completion criteria**: audit report 作成 (`report/PRD-2.7-decorator-dispatch-audit.md` 等)、silent drop 全 location 列挙、各 location の本 PRD scope 編入 / I-201-B deferral を判断、ast-variants.md に Decorator entry 新規追加 (cell 25)。
- **Depends on**: T0
- **Prerequisites**: なし

### T2: `spec-stage-adversarial-checklist.md` Rule 10 + Rule 4 全面 update (Action 5 修正 2026-04-27 で Q6 統合)

- **Work**:
  1. **Rule 10 拡張** (Q4 + Q5): sub-rule (d) AST node enumerate completeness check (`_` arm 全面禁止 + 既存 `UnsupportedSyntaxError` 統一 + audit script CI 化) + sub-rule (e) Mandatory 化 + structural reason 明示 (Permitted reasons + Prohibited keywords list、`feedback_no_dev_cost_judgment.md` 整合)
  2. **Rule 4 拡張** (Q6、Action 5 修正 2026-04-27、新規追加): sub-rule (4-1) (元 wording 維持) + sub-rule (4-2) doc-first dependency order の structural enforcement (= PRD 内 doc update task が code 改修 task の prerequisite) + sub-rule (4-3) audit script による Task List dependency chain auto verify (= 人手判断介在排除)
  3. **Lesson source 全 record**: Rule 10 = I-177-F + I-200 + 5 度 Spec gap chain (Q1-Q5 source) / Rule 4 = PRD 2.7 draft 自体の T11 dependency violation (3 度目 review で発覚した Spec gap、Q6 source、本 PRD self-evidence)
- **Completion criteria**: rule doc update 完了、sub-rule (d) + (e) (Rule 10) + sub-rule (4-1)〜(4-3) (Rule 4) が precise wording で記載、全 Lesson source 明記。
- **Depends on**: T1 (`UnsupportedSyntaxError` 既存 mechanism format が rule 内例示で参照される)
- **Prerequisites**: なし

### T3: `prd-template` skill update (Rule 10 application 必須 section hard-code、M4 修正 2026-04-27)

- **Work**:
  1. `.claude/skills/prd-template/SKILL.md` の Step 0 (Step 0a / 0b の後) に新 step "**Step 0c: Rule 10 Application (Mandatory)**" を追加
  2. PRD doc template に `## Rule 10 Application` section + fenced code block (yaml lang) を必須記入要素として追加 (T6 の machine-parseable format に整合)
  3. **Verification step mechanism (M4 修正)**: skill workflow 内に以下 verification step を追加 (skill instruction 内に明記、Claude Code agent が PRD 起票時に必ず実行):
     - skill の Step 4 (PRD Drafting) 完了直後に **`scripts/audit-prd-rule10-compliance.py <new-prd-path>` を実行**
     - exit code 非 0 (audit fail) の場合、Claude は PRD doc を修正してから skill を closing する (= skill の Verification section に明記、closing 不可 trigger)
     - exit code 0 (audit pass) で skill は closing 可能
  4. skill の Verification section (= skill template の最末尾) に "Rule 10 Application section + audit pass" を追加
- **Completion criteria**: skill update 完了、新 PRD 起票時に Rule 10 application section が auto-template化 + 必須記入 + audit pass まで closing 不可、test fixture (= 故意に Rule 10 application 不在 or prohibited keywords 含む test PRD doc) で skill が audit fail を correctly detect する verification 動作確認。
- **Depends on**: T2 (Rule 10 wording 確定後に skill template に反映), T6 (audit script の CLI invocation interface 確定後に skill verification step 設計)
- **Prerequisites**: なし

### T4: `problem-space-analysis.md` update (cross-axis enumeration の non-matrix-driven 適用 spec)

- **Work**: rule に cross-axis 直交軸 enumerate spec の non-matrix-driven 適用 section を追加。
- **Completion criteria**: rule doc update 完了、non-matrix-driven PRD でも cross-axis 軸独立 enumerate を要求する spec 明記。
- **Depends on**: T2
- **Prerequisites**: なし

### T5: `scripts/audit-ast-variant-coverage.py` 新規作成 (Q4 audit、M1 修正 2026-04-27)

- **Work**: Python script で:
  1. ts_to_rs codebase 全 Rust source の AST match 文を **`tree-sitter-rust` Python binding 経由で AST parse** (M1 修正、Python では `syn` crate は使用不可、`tree-sitter` が現実的選択)。fallback として regex parse もサポート (= simple match 文で十分な精度の場合)
  2. 各 match 文の dispatch enum + handle variant を enumerate
  3. `doc/grammar/ast-variants.md` の対応 section を parse (markdown table parse)、Tier 1 / Tier 2 / NA variant を enumerate
  4. doc-code sync verify (D7 修正 2026-04-27 で precise 化):
     - Tier 1 (Handled) = code で explicit handle (= 値を visit / type-resolve 等 actual 処理あり、function call or non-empty block)
     - Tier 2 (Unsupported) の precise verification:
       - **Transformer (変換 phase)**: `UnsupportedSyntaxError::new("VariantName", span)` 呼び出しを arm 内に持つ (regex / AST match で確認)
       - **TypeResolver (静的解析 phase)**: 明示 no-op (`{ }` empty block、または `{ /* reason: ... */ }` の reason comment 付き empty arm) を持つ。**reason comment 必須** (= comment 不在は audit fail、cell 27 compliance verify mechanism)
     - NA = code で `unreachable!("...")` 呼び出しを arm 内に持つ + message が enum variant 名 + context (e.g., "Prop::Assign in object literal context") を含む
  5. `_` arm 使用箇所を全 detect、本 PRD scope 内は 0 件 (Rule 10(d-1) compliance)、scope 外は I-203 用 detection report 出力
  6. 不一致時 audit fail (exit code 非 0)、PRD file path + 違反 reason (= どの enum / variant / dispatch site で sync 違反か) を stderr に出力
- **Completion criteria**: script 動作確認、self-test pass (本 PRD scope 内全 file で audit pass)、I-203 用 codebase-wide audit report 生成 (= 既存 `_` arm 全列挙、I-203 priority reclassify input)。
- **Depends on**: T1 (`UnsupportedSyntaxError` 経由 statement format が audit logic で参照), T2 (Rule 10 wording 確定)
- **Prerequisites**: `tree-sitter-rust` Python binding install (= `pip install tree-sitter tree-sitter-rust` 等)

### T6: `scripts/audit-prd-rule10-compliance.py` 新規作成 (Q5 audit、M3 修正 2026-04-27)

- **Work**: Python script で:
  1. `backlog/*.md` を全 parse (markdown heading parse)
  2. 各 PRD doc に `## Rule 10 Application` heading section が存在するか check
  3. **machine-parseable format precise spec (M3)**: 以下 fenced code block (yaml lang) で記述された structure を parse:
     ```yaml
     Matrix-driven: yes | no
     Rule 10 axes enumerated:
       - <axis 1 description>
       - <axis 2 description>
       - ...
     Cross-axis orthogonal direction enumerated: yes | no
     Structural reason for matrix absence: <reason text> | N/A (matrix-driven PRD)
     ```
     parse は `pyyaml` 等で yaml load (heading + fenced code block 内 yaml 内容を取得)
  4. `Matrix-driven` の値が `yes` or `no` の literal か check
  5. `Matrix-driven: yes` の場合、`Rule 10 axes enumerated` list が 1 件以上 + `Cross-axis orthogonal direction enumerated: yes` を要求
  6. `Matrix-driven: no` の場合、`Structural reason for matrix absence` が以下 prohibited keywords substring (case-insensitive) を含まないか check:
     - `scope 小`, `scope 狭`, `scope 限`, `light spec`, `pragmatic`, `LOC`, `loc`, `短時間`, `短期間`, `manageable`, `effort 大`, `実装 trivial`, `quick`, `easy`, `simple` (本 list は本 PRD で確定、`feedback_no_dev_cost_judgment.md` 整合)
  7. **Rule 4 doc-first dependency order auto verify (Action 5 修正 2026-04-27 で Q6 統合)**:
     - PRD doc 内 `## Task List` section を parse、各 `### TN: <title>` heading + `- **Depends on**: <list>` + `- **Prerequisites**: <list>` を抽出
     - **doc update task identify**: title or Work field に `ast-variants.md` / `doc/grammar/` / `reference doc` 等 doc update keyword を含む task ID を識別
     - **code 改修 task identify**: title or Work field に `src/` path / `TypeResolver` / `Transformer` / `Generator` / `convert_*` / `visit_*` / `resolve_*` 等 code 改修 keyword を含む task ID を識別
     - **doc-first verify**: 各 code 改修 task の Prerequisites or Depends on に doc update task ID が存在することを check (= 単方向 dependency: doc → code)
     - 不在時 audit fail (= Rule 4 violation、PRD merge 不能)、stderr に "Rule 4 violation: code task `<TN>` lacks prerequisite doc update task `<TM>`" 形式で出力
  8. 不一致時 audit fail (exit code 非 0)、PRD file path + 違反 reason を stderr に出力
- **Completion criteria**: script 動作確認、self-test pass (本 PRD doc で audit pass + test fixture PRD doc (`tests/audit-prd-fixtures/` 等に good/bad sample 配置) で audit pass / fail 各々 verify)。
- **Depends on**: T2, T3 (Rule 10 wording + skill format 確定後に audit logic 設計)
- **Prerequisites**: `pyyaml` 等の yaml parse library install

### T7: `.github/workflows/ci.yml` update (audit script CI 化 + merge gate、M2 修正 2026-04-27)

- **Work**:
  1. **CI step 追加**: 既存 `.github/workflows/ci.yml` の `cargo test` step の後に以下 step を追加:
     ```yaml
     - name: Audit AST variant coverage (PRD 2.7)
       run: python3 scripts/audit-ast-variant-coverage.py
     - name: Audit PRD Rule 10 compliance (PRD 2.7)
       run: python3 scripts/audit-prd-rule10-compliance.py
     ```
  2. **Merge gate 設定 (M2 修正)**: GitHub branch protection rule で本 2 step を **required check** として設定。**但し本 setting は repo admin permission 必要**。本 PRD scope では:
     - **(a) admin permission あり**: `gh api -X PUT repos/<owner>/<repo>/branches/main/protection/required_status_checks` 等で setting application
     - **(b) admin permission なし**: README に "Audit AST variant coverage / Audit PRD Rule 10 compliance を required check に設定する手順" を documentation し、user に手動 application を依頼
     どちらの path も本 PRD 完了条件に含める。Implementation 時に admin permission の availability を確認、適切な path を選択。
  3. **CI run verification**: 本 PRD PR で両 audit script step が CI で run されて pass することを confirm。
- **Completion criteria**: CI 設定 update 完了、(a) or (b) いずれかの path で merge gate が結果として確立、本 PRD PR で audit step が green。
- **Depends on**: T5, T6
- **Prerequisites**: GitHub repository への admin or write access (本 PR 提出時の前提)

### T7: `.github/workflows/ci.yml` update (audit script CI 化 + merge gate)

- **Work**: CI workflow に T5 + T6 audit script step を追加。merge gate として設定 (= PR merge 前に必須 pass)。
- **Completion criteria**: CI 設定 update 完了、本 PRD PR で両 audit script step が CI で run されて pass。
- **Depends on**: T5, T6
- **Prerequisites**: なし

### T8: TypeResolver `visit_class_body` の改修 (I-199 + Rule 10(d) application)

- **Work**: `src/pipeline/type_resolver/visitors.rs::visit_class_body` の class body match arm を改修:
  1. StaticBlock arm 追加 (visit_block_stmt 経由 walk + scope 管理)
  2. AutoAccessor arm explicit (TypeResolver は no-op、Transformer 上位で error report する設計 noted in 文中 comment)
  3. TsIndexSignature / Empty arm explicit (filter out / no-op reason 明記)
  4. `_` arm 削除
- **Completion criteria**: 改修完了、cargo build pass、unit tests (StaticBlock body の typeof narrow event push 確認 + AutoAccessor / TsIndexSignature / Empty の明示 no-op 確認 + class method/constructor の regression + cell 27 verify = 全 Tier 2 variant arm が `_ => ` ではなく explicit empty arm + reason comment 付き) 全 pass、audit-ast-variant-coverage.py が本 file で audit pass。
- **Depends on**: T1, T5, **T1.5 (Decorator dispatch audit 結果次第で T8 scope 拡大、`class_decl.class.decorators` field の handle 追加が必要な場合あり、D6 修正 2026-04-27)**
- **Prerequisites**: **T11 (`ast-variants.md` の ClassMember section が ground truth、本 task は doc に従って code を sync、Action 1 修正 2026-04-27 doc-first dependency order)**

### T9: TypeResolver `expressions.rs::ast::Expr::Object` の改修 (I-200 + Rule 10(d) application、C3 統合)

- **Work**: `src/pipeline/type_resolver/expressions.rs:331+ ast::Expr::Object` arm の inner match を改修:
  1. Prop::Method arm 追加 (visit_method_function 同等処理 = function-level scope + visit_block_stmt + return type setup)
  2. Prop::Getter arm 追加 (body visit_block_stmt 経由 walk)
  3. Prop::Setter arm 追加 (param_pat visit + body visit_block_stmt 経由 walk)
  4. **Prop::Assign arm**: **`unreachable!()` macro 呼び出し** (C3 修正、NA cell の structurally unreachable property を活かした bug detection。SWC parser が object literal context で reject する前提、もし fire したら parser 仕様変更 = bug、即時 investigate)。message: `"Prop::Assign in object literal context: SWC parser should reject (NA cell, see PRD 2.7 cell 15 + Test 20)"`
  5. `_` arm 削除 (Rule 10(d-1) compliance)
- **Completion criteria**: 改修完了、cargo build pass、unit tests (Prop::Method body の typeof narrow event push 確認 + Prop::Getter/Setter body の similar 確認 + Prop::KeyValue/Shorthand の regression + Prop::Assign は SWC parser 経由で実は AST に含まれないことを Test 20 で confirm) 全 pass、`audit-ast-variant-coverage.py` が本 file で audit pass。
- **Depends on**: T1, T5
- **Prerequisites**: **T11 (`ast-variants.md` の Prop section が ground truth、本 task は doc に従って code を sync、Action 1 修正 2026-04-27 doc-first dependency order)**、visit_method_function は existing function、re-use 可能

### T10: Transformer `convert_object_lit` の改修 (Q4 application)

- **Work**: `src/transformer/expressions/data_literals.rs::convert_object_lit` の inner match を改修:
  1. Prop::Method / Getter / Setter arm 追加 (`UnsupportedSyntaxError::new("Prop::*", span)` 経由 Tier 2 honest error report、C1 修正)
  2. Prop::Assign arm 追加 (`unreachable!()` macro、NA cell defensive coding、C3 修正)
  3. `_ => Err(anyhow!(...))` 既存 wildcard arm の format を `UnsupportedSyntaxError` 経由に統一 (broken window 解消、C6 修正)
  4. `_` arm 削除 (Rule 10(d-1) compliance)
- **Completion criteria**: 改修完了、cargo build pass、unit tests (Prop::Method/Getter/Setter で `UnsupportedSyntaxError` Err return 確認 + Prop::Assign で `unreachable!()` panic 確認 (test では `#[should_panic]` 等で verify) + 既存 KeyValue/Shorthand/Spread の regression + format 統一 (= 全 Tier 2 error message に "[unsupported]" or 同等 format prefix) 確認) 全 pass。
- **Depends on**: T1
- **Prerequisites**: **T11 (`ast-variants.md` の Prop section が ground truth、本 task は doc に従って code を sync、Action 1 修正 2026-04-27 doc-first dependency order)**

### T11: `doc/grammar/ast-variants.md` update — **doc-first single source of truth (Action 1 修正 2026-04-27、Rule 4 compliance + Q4 整合)**

- **Work**:
  1. Prop section 新規追加 (全 7 variant Tier 分類: KeyValue / Shorthand / Method / Getter / Setter / Assign + Tier 1 Handled / Tier 2 Unsupported / NA 区分)
  2. AutoAccessor entry update (Tier 2 honest error reported via `UnsupportedSyntaxError::new("AutoAccessor", aa.span)`、I-201-A/B で完全 Tier 1 化予定 言及)
  3. Decorator entry 新規追加 (Tier 2 Unsupported、I-201-B で Tier 1 化予定 言及、T1.5 audit 結果反映)
- **Position rationale (Action 1 修正)**: 元 PRD draft で T11 Depends on T8/T9/T10 = code 後 doc sync の設計だった (3 度目 `/check_job` review で Rule 4 violation = Spec gap 検出)。`spec-stage-adversarial-checklist.md` Rule 4 「matrix に reference doc に未記載の variant が存在しない (存在すれば reference doc を先に更新)」 + 本 PRD Q4 (= ast-variants.md が single source of truth、INV-1) と整合するため、T11 を T8/T9/T10 の **prerequisite** に位置付け、doc が ground truth として code 改修の reference となる順序に修正。
- **Completion criteria**: doc update 完了、本 file が以下条件 pass:
  - 全 enum section の Tier 1 / Tier 2 / NA 区分が完全 enumerate (= reference doc 完全性、後続 T8/T9/T10 で本 doc を ground truth として code 改修)
  - `audit-ast-variant-coverage.py` script の self-test fixture (T5 完了後) で本 doc が valid Tier 分類を持つことを verify (post-T5 Implementation 時の cross-check)
- **Depends on**: T2 (Rule 10 wording 確定後 Tier 分類 logic を doc に反映), T1.5 (Decorator dispatch audit 結果次第で Decorator entry 内容確定)
- **Prerequisites**: なし

### T12: regression lock-in tests + new feature tests + SWC parser empirical test

- **Work**:
  1. 全 ✓ cell (cell 1-5, 8-11) の regression lock-in unit tests
  2. ✗ → ✓ cell (cell 6, 12-14) の new feature unit tests (StaticBlock body の typeof narrow + Prop::Method/Getter/Setter body の similar)
  3. NA cell (cell 15, Prop::Assign) の SWC parser empirical regression test (`const obj = { x = 1 }` を SWC parser に通して reject されることを verify)
  4. E2E fixture (新規 `tests/e2e/scripts/prd-2.7/` dir): StaticBlock 内の typeof narrow + Prop::Method body の typeof narrow + AutoAccessor honest error の golden output
- **Completion criteria**: 全 test pass、insta snapshot 安定、E2E fixture が `cargo test --test e2e_test` で pass。
- **Depends on**: T8, T9, T10
- **Prerequisites**: なし

### T13: 本 PRD 自体の Rule 10 application section self-applied verification

- **Work**: 本 PRD doc の `## Rule 10 Application` section を audit-prd-rule10-compliance.py で audit、pass 確認 (= 本 PRD が first-class adopter として self-applied)。
- **Completion criteria**: audit script で 本 PRD doc が pass。
- **Depends on**: T6
- **Prerequisites**: なし

### T14: Quality Check + Hono bench regression check

- **Work**: `/quality-check` skill 適用 (cargo build / cargo test / clippy / fmt / file size / coverage)、Hono bench (`./scripts/hono-bench.sh`) 実行、pre/post 差分確認 (clean 111 / errors 63 維持)。
- **Completion criteria**: 全 quality check pass、Hono bench 0 regression。
- **Depends on**: T1〜T13
- **Prerequisites**: なし

### T15: `/check_job` 4-layer review

- **Work**: `/check_job` 起動、4-layer review (Mechanical / Empirical / Structural cross-axis / Adversarial trade-off) を初回 invocation で全実施。発見 defect は post-implementation-defect-classification.md 5 category (Grammar gap / Oracle gap / Spec gap / Implementation gap / Review insight) に trace ベースで分類。
- **Completion criteria**: 全 layer pass、Spec gap = 0 (framework signal なし)、Implementation gap 全 fix、Review insight は TODO 起票。
- **Depends on**: T14
- **Prerequisites**: なし

---

## Test Plan

### Unit tests (T8 / T9 / T10 関連)

#### TypeResolver visitors.rs (T8)

| # | Test name | Cell | Expected |
|---|-----------|------|---------|
| 1 | `test_visit_class_body_static_block_typeof_narrow_pushes_event` | 6 | StaticBlock 内 `typeof Container.config === "string"` で narrow event 1 件 |
| 2 | `test_visit_class_body_auto_accessor_no_op_in_typeresolver` | 7 | AutoAccessor は TypeResolver で no-op (= scope 変更なし、type info 変更なし) |
| 3 | `test_visit_class_body_ts_index_signature_filter_out` | 8 | TsIndexSignature は no-op |
| 4 | `test_visit_class_body_empty_no_op` | 9 | Empty member は no-op |
| 5 | `test_visit_class_body_method_regression` | 1 | 既存 Method handle の regression |
| 6 | `test_visit_class_body_constructor_regression` | 3 | 既存 Constructor handle の regression (I-177-F symmetric) |

#### TypeResolver expressions.rs (T9)

| # | Test name | Cell | Expected |
|---|-----------|------|---------|
| 7 | `test_resolve_object_expr_prop_method_body_visits_block` | 12 | Prop::Method body 内の typeof narrow が visit_block_stmt 経由で event push |
| 8 | `test_resolve_object_expr_prop_getter_body_visits_block` | 13 | Prop::Getter body 同上 |
| 9 | `test_resolve_object_expr_prop_setter_body_visits_block` | 14 | Prop::Setter body 同上 |
| 10 | `test_resolve_object_expr_prop_assign_panics_unreachable` | 15 | Prop::Assign は `unreachable!()` で panic (C3 修正 2026-04-27、SWC parser reject 前提の bug detection。test では `#[should_panic(expected = "Prop::Assign in object literal context")]` で verify) |
| 11 | `test_resolve_object_expr_prop_keyvalue_regression` | 10 | Prop::KeyValue 既存 handle の regression |
| 12 | `test_resolve_object_expr_prop_shorthand_regression` | 11 | Prop::Shorthand 既存 handle の regression |

#### Transformer data_literals.rs (T10)

| # | Test name | Cell | Expected |
|---|-----------|------|---------|
| 13 | `test_convert_object_lit_prop_method_returns_unsupported_err` | 17 | Prop::Method で `UnsupportedSyntaxError::new("Prop::Method", span)` 経由 Err return、`resolve_unsupported()` 経由 line/col 含む user-facing message 確認 |
| 14 | `test_convert_object_lit_prop_getter_returns_unsupported_err` | 17 | Prop::Getter 同上 |
| 15 | `test_convert_object_lit_prop_setter_returns_unsupported_err` | 17 | Prop::Setter 同上 |
| 16 | `test_convert_object_lit_prop_assign_panics_unreachable` | 15 | Prop::Assign は `unreachable!()` で panic (C3 修正、`UnsupportedSyntaxError` ではない。test では `#[should_panic(expected = "Prop::Assign in object literal context")]` で verify) |
| 17 | `test_convert_object_lit_prop_keyvalue_regression` | 10 | Prop::KeyValue 既存 handle の regression |
| 18 | `test_convert_object_lit_prop_shorthand_regression` | 11 | Prop::Shorthand 既存 handle の regression |
| 19 | `test_convert_object_lit_spread_regression` | (Spread) | Spread 既存 handle の regression |

### SWC parser empirical test (T12、M5 修正 2026-04-27 file location 指定)

| # | Test name | File location | Cell | Expected |
|---|-----------|---------------|------|---------|
| 20 | `test_swc_parser_rejects_prop_assign_in_object_literal_context` | **`tests/swc_parser_object_literal_prop_assign_test.rs`** (新規 integration test file) | 15 | `const obj = { x = 1 }` を SWC parser (`swc_ecma_parser::parse_module` 等) に通すと parse error (= NA cell の structural reason empirical verify、Prop::Assign の cell 15 + cell 17 (Transformer) の `unreachable!()` precondition 保証)。assertion: parse 結果が `Err` で、error message が "expected" 相当の syntax error を含む。SWC version は `Cargo.toml` の swc_ecma_parser version で固定、SWC 仕様変更で本 test が fail したら `unreachable!()` の precondition 違反 = code revisit 必要 |

### Audit script tests (T5 / T6)

| # | Test name | Expected |
|---|-----------|---------|
| 21 | `audit-ast-variant-coverage.py self-test (本 PRD 後)` | T8/T9/T10 改修後の本 PRD scope 内全 file で audit pass、`_` arm 0 件 (本 PRD scope 内) |
| 22 | `audit-prd-rule10-compliance.py self-test (本 PRD doc)` | 本 PRD doc で Rule 10 Application section が存在 + structural reason の prohibited keywords 不在で pass |
| 23 | `audit-ast-variant-coverage.py I-203 audit report` | 既存 codebase 全体の `_` arm 使用箇所 detection report 生成 (I-203 PRD の audit driven priority 判定 input) |

### E2E fixtures (T12 + T0、Action 3 修正 2026-04-27 で全 cell 1-to-1 mapping 完全化)

各 ✗ cell + 関連 cell に対応する E2E fixture の 1-to-1 mapping。T0 で作成済の 9 fixture (`tests/e2e/scripts/prd-2.7/cell-NN-...ts` + `.expected`) を Test entry に明示:

| # | Cell | Fixture path | Description | Type |
|---|------|--------------|-------------|------|
| 24 | 6 | `tests/e2e/scripts/prd-2.7/cell-06-static-block-typeof-narrow.ts` | StaticBlock body 内 local 変数 typeof narrow が visit_block_stmt 経由 walk で narrow event push される、生成 Rust が narrow-aware emission (`if let / match`) で stdout `default-string-narrowed` を出力 (post-T8 改修後、cargo run vs tsc 一致) | E2E (cargo run vs tsc 一致) |
| 25 | 7 | `tests/e2e/scripts/prd-2.7/cell-07-auto-accessor-honest-error.ts` | AutoAccessor を含む TS source の cargo run で Transformer が `UnsupportedSyntaxError::new("AutoAccessor", aa.span)` 経由 honest error を返す (= 既実装、format 統一後の verify) | Tier 2 honest error (E2E 不可、stderr verify) |
| 26 | 10 | `tests/e2e/scripts/prd-2.7/cell-10-prop-keyvalue-regression.ts` | 既存 Prop::KeyValue handle の regression lock-in、cargo run vs tsc stdout 完全一致 (post-改修で behavior 不変) | E2E regression lock-in |
| 27 | 11 | `tests/e2e/scripts/prd-2.7/cell-11-prop-shorthand-regression.ts` | 既存 Prop::Shorthand handle の regression lock-in、cargo run vs tsc stdout 完全一致 | E2E regression lock-in |
| 28 | 12 | `tests/e2e/scripts/prd-2.7/cell-12-prop-method-typeof-narrow.ts` | Prop::Method body 内 typeof narrow (TypeResolver visit 確認、Transformer は I-202 待ちで `UnsupportedSyntaxError::new("Prop::Method", span)` 経由 Err return)、format 統一後の error message verify | Tier 2 honest error (stderr verify) |
| 29 | 13 | `tests/e2e/scripts/prd-2.7/cell-13-prop-getter-typeof-narrow.ts` | Prop::Getter body 内 typeof narrow (TypeResolver visit + Transformer Tier 2 honest error format 統一) | Tier 2 honest error (stderr verify) |
| 30 | 14 | `tests/e2e/scripts/prd-2.7/cell-14-prop-setter-typeof-narrow.ts` | Prop::Setter body 内 typeof narrow (TypeResolver visit + Transformer Tier 2 honest error format 統一) | Tier 2 honest error (stderr verify) |
| 31 | 15 | `tests/e2e/scripts/prd-2.7/cell-15-prop-assign-na.ts` | Prop::Assign NA cell の spec-traceable evidence (destructuring default の valid example + comment で structural reason 明記)、SWC parser empirical reject は Test 20 で別途 verify | Spec-traceable evidence (NA) |
| 32 | 17 | `tests/e2e/scripts/prd-2.7/cell-17-transformer-convert-object-lit-error.ts` | Transformer convert_object_lit の `_ => Err(anyhow!())` を `UnsupportedSyntaxError` 経由に format 統一後の verify | Tier 2 honest error (stderr verify) |

**T0 (Spec stage、red 状態)**: 全 9 fixture + .expected 作成済 (2026-04-27)。Implementation stage で T8/T9/T10 改修後に各 fixture が green 化 (cell 6/10/11) または stderr format 統一 (cell 7/12/13/14/17) または NA 維持 (cell 15)。

**E2E test runner integration**: `tests/e2e_test.rs` に各 fixture の test 関数 (`run_e2e_test("prd-2.7/cell-06-static-block-typeof-narrow")` 等) を Implementation stage T12 で追加。Tier 2 honest error cell (cell 7/12/13/14/17) は stderr + exit code verify を `assert_unsupported_syntax!` 等の helper で integrate (要 helper 追加 or `tests/integration_test.rs` 経由 verify)。

### Hono bench regression check (T14)

- pre-PRD bench: clean 111 / errors 63
- post-PRD bench: clean 111 ± 1 / errors 63 ± 2 (noise variance、bench 非決定性 [I-172] 範囲内)
- 範囲外 = regression、即時 root cause 特定 + fix

---

## Completion Criteria

以下 **全て** を満たすまで本 PRD 完了不可:

1. ✅ Problem Space matrix 全 26 cell に対する handle (✓ regression lock-in / ✗ → fix / NA → reason 明示) が code + doc + test に反映
2. ✅ `spec-stage-adversarial-checklist.md` Rule 10 sub-rule (d) + (e) update 完了
3. ✅ `prd-template` skill に Rule 10 application 必須 section hard-code 完了
4. ✅ `problem-space-analysis.md` cross-axis enumeration spec 追加完了
5. ✅ 既存 `UnsupportedSyntaxError` mechanism format 統一 + 全 PRD 2.7 scope 内 Transformer Tier 2 arm への適用拡張完了 (`data_literals.rs:259-263` の `anyhow!()` 経由 broken window 解消含む、新規 macro 作成は不要、C1 + C6 修正)、NA cell (Prop::Assign) は `unreachable!()` 化 (C3 修正)
6. ✅ TypeResolver `visit_class_body` + `expressions.rs::ast::Expr::Object` の `_` arm 削除 + 全 variant explicit enumerate 完了
7. ✅ Transformer `convert_object_lit` の `_` arm 削除 + 全 variant explicit enumerate 完了
8. ✅ `doc/grammar/ast-variants.md` Prop section 新規追加 + AutoAccessor entry update + Decorator entry 新規追加完了
9. ✅ `scripts/audit-ast-variant-coverage.py` + `scripts/audit-prd-rule10-compliance.py` 新規作成 + CI 化 + merge gate 設定完了
10. ✅ 全 unit tests + SWC parser empirical test + audit script self-test + E2E fixture 全 pass
11. ✅ 本 PRD 自体の `## Rule 10 Application` section が audit script で pass (self-applied verification)
12. ✅ `cargo test` 全 pass、`cargo clippy --all-targets --all-features -- -D warnings` 0 warning、`cargo fmt --all --check` 0 diff
13. ✅ Hono bench 0 regression (clean 111 ± 1 / errors 63 ± 2 範囲内)
14. ✅ `/check_job` 4-layer review 全 layer pass、Spec gap = 0 (framework signal なし)、発見 defect 全 fix or 別 PRD 起票
15. ✅ Defect Classification Summary で Grammar gap / Oracle gap / Spec gap / Implementation gap = 0、Review insight は TODO 起票

**Matrix completeness requirement (最上位完了条件)**: 全 26 cell が ideal 仕様一致 + lock-in test 存在を `/check_job` Layer 2 (Empirical) で verify。1 cell でも未カバーで完了不可。

**Impact estimate verification**: 本 PRD の error count reduction 推定なし (= framework + coverage 改修、Hono bench 0 regression が impact 直接指標)。

---

## Rule 10 Application

```yaml
Matrix-driven: yes
Rule 10 axes enumerated:
  - "Layer A: AST node iterate target (ClassMember / PropOrSpread / Prop)"
  - "Layer B: variant 現状処理 (visited / silent drop / 暗黙 silent drop / 経路不在)"
  - "Layer C: ast-variants.md spec (Tier 1 / Tier 2 / Section 不在)"
  - "Layer D: Rule 10 適用範囲 (matrix-driven only / 全 PRD Mandatory)"
  - "Layer E: enforcement mechanism (doc only / skill hard-code / audit script + CI)"
Cross-axis orthogonal direction enumerated: yes
Cross-axis orthogonal directions:
  - "(I) 逆問題視点: structural enforcement の対立 = 人間判断介在 (= Anti-pattern として明示禁止)"
  - "(II) 実装 dispatch trace: ClassMember + PropOrSpread + Prop variant の全 dispatch"
  - "(III) 影響伝搬 chain: silent drop → 進捗評価 ground truth 失墜 → ideal 違反"
Structural reason for matrix absence: "N/A (matrix-driven PRD)"
```

### 8 default check axis NA reason (Action 2 修正 2026-04-27、Rule 10 三度目 review で明示化)

`spec-stage-adversarial-checklist.md` Rule 10 の **8 default check axis** のうち、本 PRD scope (= AST traversal coverage) で applicable / NA を明示:

| # | Axis | 状態 | Reason |
|---|------|------|--------|
| (a) | Trigger condition (operator / syntax-form) | **Applicable** | Layer A (AST node iterate target = ClassMember / Prop) として enumerate 済 |
| (b) | Operand type variants | **Applicable** | matrix cell 1-15 で各 variant individually enumerate 済 |
| (c) | Guard variant (typeof / equality / instanceof / truthy) | **NA** | 本 PRD は AST traversal coverage (= dispatch arm completeness)、guard variant は runtime narrow framework の dimension で本 PRD scope orthogonal |
| (d) | Body shape (block / expr / single-stmt / empty) | **NA** | 同上、本 PRD は static AST traversal、body shape は runtime emission dimension で orthogonal |
| (e) | Closure-reassign 有無 | **NA** | 本 PRD は visit 経路追加 + dispatch arm coverage、closure-reassign は runtime mutation propagation dimension で orthogonal (PRD 3 = I-177 mutation propagation で扱う) |
| (f) | Early-return 有無 | **NA** | 同上、early-return は narrow emission dimension (PRD 4 = I-177-A で扱う) |
| (g) | Outer emission context (return / assign target / call arg / branch arm) | **NA** | 本 PRD は TypeResolver visit 経路 + Transformer Tier 2 honest error の dispatch coverage、outer emission context は Transformer Tier 1 完全 emission strategy の dimension (= I-202 別 PRD で object literal context 完全 emission を扱う) |
| (h) | Control-flow exit (4 sub-case) | **NA** | Rule 7 で明示 (= AST traversal coverage は control-flow exit dimension と orthogonal) |

**Cross-axis 直交軸 (8 default + 機能依存)**:
- 機能依存 axis (本 PRD 固有): Layer C (doc spec status) / Layer D (rule 適用範囲) / Layer E (enforcement mechanism、structural enforcement vs 人間判断介在)
- 8 default axis のうち applicable な (a) + (b) は Layer A + matrix cell に integrate

→ **本 PRD scope は AST traversal coverage で 5 axis (c-g) が naturally NA**、各々の NA reason は spec-traceable (= 本 PRD は static analysis の dispatch coverage、runtime narrow / mutation / emission strategy は別 PRD で扱う orthogonal concern)。

---

## Defect Classification (Self-applied、D9 修正 2026-04-27)

本 PRD doc draft / 1 度目 review / 2 度目 review で発見した defect を `post-implementation-defect-classification.md` の 5 category に trace ベースで分類 (= self-applied verify、本 PRD 自体が framework rule の first-class adopter):

### Spec stage Discovery 段階で発見

| Category | Count | Defects | Trace |
|----------|-------|---------|-------|
| Grammar gap | 1 | doc/grammar/ast-variants.md に Prop section 不在 (cell 23 で fix) + Decorator entry 不在 (cell 25 で fix) | reference doc check、Prop / Decorator section 不在を audit で確認 |
| Oracle gap | 0 | — | 全 cell ideal output が SWC AST 定義 / TC39 spec / 既存 codebase audit で grounding 済 |
| **Spec gap** | **5 (Q1-Q5 source)** | (1) Synthetic registry integration (I-177-E) / (2) arrow + fn_expr block_end (I-177-F initial) / (3) IR shadow emission (I-177-A symmetric) / (4) class method/constructor (I-177-F extended) / (5) StaticBlock + obj literal method (I-199 + I-200) | I-177-B PRD draft 時 Cross-axis enumeration 不足 + reference doc 完全性不足が root cause (= framework signal、Rule 10(d) + Mandatory 化で structural 解消) |
| Implementation gap | 0 | — | 本 PRD は Spec stage、Implementation gap は T8-T15 完了後の post-implementation review で発見可能 |
| Review insight | 6 (Major M1-M6) | tree-sitter-rust / CI merge gate admin / machine-parseable format / skill verification step / SWC parser test file location / Semantic Safety Analysis depth | 完成度向上の追加 spec、本 PRD scope 内で全対応 |

### 1 度目 review (PRD doc draft 直後) で発見した defect

| Category | Count | Defects | Trace + 修正 |
|----------|-------|---------|-------------|
| Grammar gap | 1 | C7 (Decorator dispatch audit 不足、TypeResolver / Transformer 内 silent drop 検出 task 漏れ) | T1.5 task 追加で fix |
| Oracle gap | 0 | — | — |
| **Spec gap** | **3** | C1 (`UnsupportedSyntaxError` 既存 mechanism 認識漏れ = 新規 macro 重複設計) / C2 (matrix vs Design 矛盾) / C5 (Spec stage E2E fixture 位置付け誤り) | C1: T1 / Design 2.1 / cell 26 全面書き直し / C2: cell 7 + Design 3.1 整合 / C5: T0 task 新規追加 |
| Implementation gap | 3 | C3 (Prop::Assign defensive coding silent drop 延長) / C4 (Rule 10(d-2) wording 一律的、phase 別役割分担明示不足) / C6 (`data_literals.rs` `anyhow!()` 経由 format 不整合 broken window) | C3: `unreachable!()` macro 化 / C4: Rule 10(d-2) wording に Transformer / TypeResolver / NA の 3 mechanism を明示 / C6: `UnsupportedSyntaxError` 統一 |
| Review insight | 6 | M1-M6 | 完成度向上、本 PRD 内 fix |

→ **Spec gap × 3 件**: PRD doc draft 時の Cross-axis enumeration が不十分 (= 既存 mechanism audit / `spec-first-prd.md` strict 遵守 / phase 別役割分担)。

### 2 度目 review で発見した defect (D1-D9)

| Category | Count | Defects | 修正 |
|----------|-------|---------|------|
| Grammar gap | 0 | — | — |
| Oracle gap | 0 | — | — |
| Spec gap | 0 | — (= 1 度目 review の Spec gap × 3 を追加検出する gap なし、framework reinforced) | — |
| Implementation gap | 6 | D1+D2 (cell 8/9 wording C1 修正前残存) / D3 (Rule 1 cell count 26 → 27 sync 漏れ) / D4 (Goal 5 wording C1 修正前残存) / D5 (Goal 6 wording C3 修正前残存) / D6 (T8 Depends on T1.5 漏れ) / D7+D8 (Test Plan の cell 27 audit script verify precision 不足) | 全 fix 済 (cell 8/9 / Rule 1 / Goal 5 / Goal 6 / T8 / T5) |
| Review insight | 1 | D9 (Defect Classification self-applied 不在) | 本 section 新規追加で fix |

→ 2 度目 review で発見した defect は全て Implementation gap (= 修正大規模時の sync 漏れ)。Spec gap が 0 = framework reinforced による spec 完成度の向上を確認。

### 3 度目 review (`/check_job` Spec stage 10-rule 全項目 verification、2026-04-27) で発見した defect

| Category | Count | Defects | 修正 |
|----------|-------|---------|------|
| Grammar gap | 0 | — | — |
| Oracle gap | 0 | — | — |
| **Spec gap** | **1 (= Action 1)** | Action 1: T11 (`ast-variants.md` update) Depends on T8/T9/T10 = doc を code 後 sync = `spec-stage-adversarial-checklist.md` Rule 4 violation + Q4 (single source of truth、INV-1) 違反 | T11 Depends on を T2/T1.5 に変更 + T8/T9/T10 Prerequisites に T11 追加 (doc-first dependency order)、framework 改善検討として Q6 (Rule 4 拡張) を本 PRD scope に integrate |
| Implementation gap | 0 | — | — |
| Review insight | 3 (Action 2-4) | Rule 10 8-axis NA reason explicit 化 / Test entry 24-26 → 24-32 で 1-to-1 mapping 完全化 / Spec Review Checklist Rule 7-10 wording precise verification 詳細化 | 全 fix 済 |

→ **Spec gap × 1 件 発見** = framework 失敗 signal (= 本 PRD draft + 1 度目 + 2 度目 review で Rule 4 application 不足が 3 度漏れた)。**framework 改善検討 (Q6) を本 PRD scope に integrate** することで本質的解決:
- `spec-stage-adversarial-checklist.md` Rule 4 wording 拡張 (sub-rule (4-2) doc-first dependency order + (4-3) audit script auto verify)
- `scripts/audit-prd-rule10-compliance.py` (T6) で Task List dependency chain auto verify 追加
- 本 PRD 自体が Rule 4 改修の first-class adopter (T11 を T8/T9/T10 prerequisite に位置付け、self-evidence)

### Implementation stage で発見した defect (Spec への逆戻り発動 record、`spec-first-prd.md` 「Spec への逆戻り」)

#### Revision 1 (T11 実施中、2026-04-27)

- **Trigger**: T11 (`doc/grammar/ast-variants.md` update) 実施中、Prop section 新規追加と並行して T9/T10 改修対象 file (`expressions.rs::ast::Expr::Object`、`data_literals.rs::convert_object_lit`) の dispatch enum を audit。`for prop in object_lit.props { match prop { ... } }` の最外層 match 対象が **`PropOrSpread` enum** (parent) であり、`Prop` enum (child) は `PropOrSpread::Prop(Box<Prop>)` 経由でアクセスされることを発見。
- **Defect category (`post-implementation-defect-classification.md` 5 category)**: **Grammar gap** (= reference doc に entry がない variant が関与する defect)
- **Spec gap detail**: 当初 PRD spec の matrix cell 16 wording で「全 Prop variant explicit enumerate (handle 済 KeyValue/Shorthand/**Spread** + ...)」と記載され、`PropOrSpread::Spread` variant を `Prop` variant と同 dispatch level で混在記述していた (= dispatch hierarchy parent/child の不徹底)。matrix cell 自体は正しい intent (= 全 dispatch arm explicit) を持つが、ast-variants.md に PropOrSpread section 不在 = **doc 側 single source of truth 違反 (Q4 violation = Rule 10(d-3))**。
- **Resolution (本 PRD 2.7 scope に編入)**:
  - **`doc/grammar/ast-variants.md` update** (T11 実施分、追加完了 2026-04-27): 新 section 12 "PropOrSpread" 追加 (Tier 1 Handled = Spread / Prop の 2 variant、両 variant 既実装で Tier 1 trivial coverage)、既存 Prop section を section 13 に shift、PropName-Decorator section を 14-20 に shift
  - **Matrix cell 追加 (本 doc Problem Space 組合せマトリクス、cell 25.5 として cell 25 Decorator entry の後に挿入)**: 新 cell "PropOrSpread section 新規追加" を Layer C 補完判定として追加
  - **Audit script (T5) precise**: `audit-ast-variant-coverage.py` の audit 対象 enum に `PropOrSpread` 含む = section 不在の enum を audit script が検出可能 (= Rule 10(d-3) 完全性、本 Revision 1 type の Grammar gap が future PRD で再発しない構造的 mechanism)
  - **Dispatch implementation (T9/T10)**: `expressions.rs::ast::Expr::Object` arm の最外層 match で `PropOrSpread::Spread(spread) => { /* visit spread.expr */ }` + `PropOrSpread::Prop(prop) => match &**prop { ... }` を explicit enumerate (`_` arm 不在、Rule 10(d-1) compliance)、`data_literals.rs::convert_object_lit` も同様
- **Impact on architectural concern**: 本 PRD architectural concern「framework Rule 改修 + 拡張による coverage gap detection 完成 + structural enforcement」の "拡張による coverage gap detection 完成" 部分の natural extension。PropOrSpread section 追加は本 PRD scope の延長で、architectural concern を violate しない (= 1 PRD = 1 architectural concern 維持)。
- **Lesson source for framework**: 当初 spec stage で **dispatch hierarchy (parent/child enum)** の cross-axis enumeration が不十分だった = `spec-stage-adversarial-checklist.md` Rule 10 (Cross-axis matrix completeness) の application 不徹底。本 lesson を T2 update 時の Rule 10 sub-rule (e) axis enumeration default check に追加検討 (= 候補追加 axis: "(j) AST dispatch hierarchy: parent enum + child enum の各 layer を独立 axis として enumerate")。

#### Revision 2 (T12 実施中、2026-04-27、critical Spec gap fix)

- **Trigger**: T12 (SWC parser empirical regression lock-in test、cell 15 reachability empirical verify) 実施中、`tests/swc_parser_object_literal_prop_assign_test.rs` の最初 run で `{ x = expr }` を SWC parser が **accept** する事実を発見。当初 PRD spec の cell 15 NA 認識 (= "SWC parser reject 前提" + `unreachable!()` macro) の precondition violation。
- **Defect category**: **Spec gap** (= reference doc + oracle から derivable だったが matrix で NA と誤認識、= **framework 失敗 signal**)
- **Spec gap detail**: PRD 起票時に SWC parser に対する empirical observation を skip し、TS spec の "TS では parse error" を SWC parser behavior の assumption として採用していた。実際は SWC parser は寛容 parsing で `Prop::Assign` を accept (= TS spec 違反 syntax を AST に含める)、`unreachable!()` の precondition が actual に satisfy されない。これは silent semantic change risk (= ts_to_rs が invalid syntax を silent に別 form に誤変換する可能性) を内包する critical defect。
- **Resolution (本 PRD 2.7 scope に編入、Implementation Revision 2 として fix 完了 2026-04-27)**:
  - **`expressions.rs` (TypeResolver)**: `Prop::Assign(_) => unreachable!(...)` → `Prop::Assign(_) => { total_explicit_props += 1; }` (静的解析 phase abort 不可、no-op で type info 記録のみ)
  - **`data_literals.rs` (Transformer 3 site)**: `convert_object_lit` + `convert_discriminated_union_object_lit` + `try_convert_as_hashmap` の `Prop::Assign(_) => unreachable!(...)` を `Prop::Assign(assign_prop) => return Err(UnsupportedSyntaxError::new("Prop::Assign", assign_prop.span).into())` に変更 (Tier 2 honest error、Q4 application format 統一)
  - **`doc/grammar/ast-variants.md`**: Prop section の Assign entry を NA section → Tier 2 Unsupported に reclassify、honest error reporting via `UnsupportedSyntaxError` 明示
  - **`tests/swc_parser_object_literal_prop_assign_test.rs`**: lock-in test の expectation を reverse (= SWC parser accept verify + destructuring default 別経路 valid 確認)
  - **Matrix cell 15**: NA → Tier 2 (honest error) に reclassify、ideal output wording を update
- **Lesson source for framework**: NA cell justification は **SWC parser empirical observation 必須** (= TS spec "should reject" を assumption として採用しない、SWC parser actual behavior を実 source code で empirical 確認)。`spec-stage-adversarial-checklist.md` Rule 3 (NA justification) wording に "SWC parser empirical observation 必須" を追加検討 (post-PRD 2.7 follow-up)。
- **Severity**: **Critical (= framework 失敗 signal)**。本 Revision 2 を未発見のままで close すると `unreachable!()` の precondition violation が production code に残り、SWC parser 経由で reach した時点で panic crash (= silent ではないが Tier 2 honest error と本来の design 意図の violation)。本 PRD 2.7 self-applied verify mechanism (T13 = audit-prd-rule10-compliance.py) は本 Revision 2 を **未検出** (= matrix cell の reachability empirical verify は audit script の scope 外、SWC parser empirical observation は test runtime のみ verify)。framework 改善余地として "matrix cell の SWC parser empirical observation を spec stage 必須化" を `spec-stage-adversarial-checklist.md` に追加検討。

### 全体 summary (Spec stage Discovery → 1 度目 → 2 度目 → 3 度目 → Implementation Revisions trajectory)

| Stage | Spec gap | Implementation gap | Grammar gap | Review insight |
|-------|---------|-------------------|-------------|----------------|
| Spec stage Discovery (Q1-Q5) | 5 (framework signal) | 0 | 0 | 6 |
| 1 度目 review | 3 (= C1, C2, C5) | 3 (= C3, C4, C6) | 0 | 6 (= M1-M6) |
| 2 度目 review | 0 | 6 (= D1-D6) | 0 | 1 (= D9) |
| **3 度目 review (`/check_job`)** | **1 (= Action 1、framework 失敗 signal)** | **0** | **0** | **3 (= Action 2-4)** |
| **Implementation stage Revision 1 (T11 実施中)** | **0** | **0** | **1 (= PropOrSpread section 不在、本 PRD scope 内 fix 完了)** | **1 (Rule 10 axis 候補追加 lesson)** |
| **Implementation stage Revision 2 (T12 実施中)** | **1 (= cell 15 NA 誤認識、critical framework 失敗 signal)** | **0** | **0** | **1 (Rule 3 NA justification SWC empirical 必須 lesson)** |

→ Spec gap chain は **5 → 3 → 0 → 1 → 0 → 1 → 0** の trajectory (Implementation stage Revision 2 で critical Spec gap 1 件発見、本 PRD scope 内 fix 完了 + 後続 `/check_job` 4-layer review F1 で framework integration 完成 = Spec gap 0 reset)。3 度目 review で +1 = Rule 4 application Spec gap が `/check_job` 4-layer 相当の Spec stage 10-rule full verification で初めて検出 → Q6 framework 改修で再発防止 mechanism 確立。Implementation stage Revision 1 (T11) で Grammar gap 1 件発見 (= PropOrSpread section 不在) → 本 PRD scope 内 fix。**Implementation stage Revision 2 (T12) で critical Spec gap 1 件発見 (= cell 15 NA 誤認識、SWC parser empirical observation skip) → 本 PRD scope 内 fix (Tier 2 honest error 化、ast-variants.md update、lock-in test reverse)**。本 critical Spec gap は当初 spec stage 10-rule + 3 度 `/check_job` review でも検出されず Implementation stage で初めて empirical 顕在化 → **本 PRD `/check_job` 4-layer review (Implementation stage 初回 invocation) で発見 → F1 fix で `spec-stage-adversarial-checklist.md` Rule 3 wording に sub-rule (3-1)/(3-2)/(3-3) (SWC parser empirical observation 必須) を追加完了 (Versioning v1.2)、本 PRD 自体が Rule 3 改修の first-class adopter として self-applied integration achievement、framework 失敗 signal の structural fix completion**。

**Spec gap detail (3 度目 review、framework 失敗 signal)**:
- Defect: 本 PRD draft 時の T11 Depends on T8/T9/T10 = doc を code 後 sync 設計
- Trace: `spec-stage-adversarial-checklist.md` Rule 4 entry 確認 ✓ + Q4 (single source of truth) 確認 ✓、両者の整合 verification を PRD draft + 1 度目 + 2 度目 review で実施せず → enumerate 漏れ = Spec gap
- Framework 改善検討: Rule 4 wording に "doc-first dependency order の structural enforcement + audit script auto verify" を追加 (Q6)、本 PRD scope に integrate (1 PRD = 1 architectural concern の "framework Rule 改修" cohesive concern 内、Q4/Q5/Q6 の 3 rule 改修を統合)
- Self-evidence: 本 PRD 自体が Rule 4 改修の first-class adopter として、T11 の dependency 修正 + Rule 4 application section の追加で self-applied verify

---

## Discovery 確定 (2026-04-27)

- **Q1 (AutoAccessor)**: (b) Tier 2 error report 化 (本 PRD) + (c) 完全 Tier 1 化を I-201-A (decorator なし subset、L3) + I-201-B (decorator framework、**L1 silent semantic change**) で別 PRD 化 ((d) 構造分離)
- **Q2 (Object literal Prop::Method/Getter/Setter)**: (a) Symmetric resolve (TypeResolver visit only、本 PRD) + Transformer 完全 emission を I-202 で別 PRD 化 ((d) 構造分離)
- **Q3 (Prop::Assign)**: NA cell + lock-in test (triple ideal 自動達成、本 PRD 内完結、別 PRD 不要)
- **Q4 (Rule 10(d) AST node enumerate completeness check)**: `_` arm 全面禁止 + 既存 `UnsupportedSyntaxError` mechanism (`src/transformer/mod.rs:193-219`) を Transformer Tier 2 variant arm で統一適用 (C1 修正 2026-04-27、新規 macro 作成は不要 = DRY 違反回避) + TypeResolver は明示 no-op (Rule 10(d-2) phase 別役割分担、C4 修正) + NA cell は `unreachable!()` (C3 修正) + ast-variants.md single source of truth + audit script CI 化 (本 PRD)。既存 codebase 全体の `_` arm refactor は I-203 で別 PRD 化 ((d) 構造分離)
- **Q5 (Rule 10 Mandatory 化)**: 全 PRD Mandatory + structural reason 明示 (Permitted reasons + Prohibited keywords list) + Cross-axis 軸独立 enumerate + machine-parseable format + skill hard-code + audit script CI merge gate (本 PRD)。人間判断介在 0、妥協の逃げ 道 structural 排除
- **Q6 (Rule 4 doc-first dependency order、Action 5 修正 2026-04-27、3 度目 `/check_job` review で発見した Spec gap × 1 件の framework 改善検討の本質対応)**: `spec-stage-adversarial-checklist.md` Rule 4 wording 拡張で **doc update task が code 改修 task の prerequisite** という dependency order を structural enforcement。元 PRD draft 時に T11 (`ast-variants.md` update) が T8/T9/T10 (code 改修) の **後** に位置していた = single source of truth (Q4) 違反 = Rule 4 違反 = Spec gap が 3 度目 review で発覚。本 self-evidence を起点に Rule 4 改修を本 PRD scope に integrate (= 1 PRD = 1 architectural concern の "framework Rule 改修" cohesive concern 内、Q4/Q5 と integrate)、`audit-prd-rule10-compliance.py` で Task List dependency chain auto verify を含む。

---

## References

- [`spec-stage-adversarial-checklist.md`](../.claude/rules/spec-stage-adversarial-checklist.md) — Rule 10 改修対象
- [`problem-space-analysis.md`](../.claude/rules/problem-space-analysis.md) — Cross-axis enumeration spec
- [`prd-template/SKILL.md`](../.claude/skills/prd-template/SKILL.md) — Rule 10 application hard-code 対象
- [`ideal-implementation-primacy.md`](../.claude/rules/ideal-implementation-primacy.md) — 最上位原則
- [`pipeline-integrity.md`](../.claude/rules/pipeline-integrity.md) — TypeResolver / Transformer 分離
- [`design-integrity.md`](../.claude/rules/design-integrity.md) — Design Integrity Review
- [`type-fallback-safety.md`](../.claude/rules/type-fallback-safety.md) — Semantic Safety Analysis
- [`feedback_no_dev_cost_judgment.md`](../../.claude/projects/-home-kyohei-ts-to-rs/memory/feedback_no_dev_cost_judgment.md) — Anti-pattern keyword list の base
- [`feedback_no_compromise_ideal.md`](../../.claude/projects/-home-kyohei-ts-to-rs/memory/feedback_no_compromise_ideal.md) — triple ideal + structural enforcement
- TODO entries: I-198 / I-199 / I-200 / I-201-A / I-201-B / I-202 / I-203
