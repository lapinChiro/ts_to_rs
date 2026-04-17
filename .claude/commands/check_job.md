今回の開発作業と実装を、第三者の視点で徹底的にレビューしてください。
妥協した実装はありませんか？理由に関わらず、実装および設計、もしくはそれらに付随する確認作業で妥協は絶対に許容しません。
最も理想的でクリーンな実装にすることだけを考えてください。
開発工数や変更規模は判断基準になりません。最も理想的でクリーンな実装だけが正解です。

また、必要十分で高品質な自動テストが実装されていることも確認してください。

## Spec-First Stage Dispatch (SDCDF)

対象 PRD が matrix-driven (`.claude/rules/spec-first-prd.md` 適用対象) の場合、
PRD の現在の stage に応じて review 内容を切り替えてください:

### Spec Stage (Implementation 未着手)

以下の Spec-Stage Adversarial Review Checklist を検証してください:

1. **Matrix completeness**: 全セルに ideal output が記載されているか (空欄/TBD なし)
2. **Oracle grounding**: ✗/要調査 セルの ideal output が tsc observation と cross-reference されているか
3. **NA justification**: NA セルの理由が spec-traceable (syntax error, grammar constraint 等) であり、「稀」「多分」等の曖昧理由がないか
4. **Grammar consistency**: matrix に `doc/grammar/` reference doc に未記載の variant が存在しないか (存在すれば reference doc を先に更新)
5. **E2E readiness**: 各セルに対応する E2E fixture が (red 状態で) 準備されているか

1 つでも未達の項目があれば明確に指摘し、Implementation stage への移行を block してください。

### Implementation Stage (Spec approved 後)

従来の check_job review に加えて:

- 各セルの実装出力が spec の ideal output と一致するかを検証
- spec に定義されていないセルが暗黙に実装されていないかを確認
- 発見した defect を以下の 5 category に分類:
  - **Grammar gap**: reference doc に entry がない variant が関与
  - **Oracle gap**: tsc observation が不十分
  - **Spec gap**: derivable だったが enumerate 漏れ (**framework 失敗 signal**)
  - **Implementation gap**: spec 通りでない実装
  - **Review insight**: spec も実装も正当、新たな気づき
- 各 defect の category 分類は trace に基づく (主観判断ではない)
