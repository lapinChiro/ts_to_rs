# Problem Space Analysis (最上位 PRD ルール)

## When to Apply

PRD 作成時の **最初のステップ** として、機能の問題空間を網羅的に enumerate する。
Discovery より前、設計より前、実装より前。

本ルールは全ての PRD 作成・修正作業に **絶対に遵守しなければならない**。
例外はない。

## Core Principle

> **特定の defect を fix するのではなく、機能の問題空間を網羅して本質的に解決する。**

TODO に書かれる defect は、常に問題空間の **氷山の一角**。defect 単体を fix しても、
未認識の edge case は残る。実装後の review 毎に新たな defect が発見される場合、
それは問題空間を事前に enumerate していない証拠 — **構造的欠陥の症状**。

「具体的な defect を fix して動いたから完了」は禁止。「問題空間の全セルに対して ideal
出力が定義され、全セルがテスト lock-in されている」が完了条件。

## Constraints

### 1. 問題空間の enumerate は PRD の必須セクション

PRD 起票時、**Problem Space** セクションを最上位に設け、以下を必須記述する:

```markdown
## Problem Space

### 入力次元 (Dimensions)

機能の出力を決定する入力要素を、独立した次元として列挙する:
- 次元 A: <variant 列挙、省略なし>
- 次元 B: <variant 列挙、省略なし>
- 次元 C: <variant 列挙、省略なし>
- ...

### 組合せマトリクス

全次元の直積を表形式で enumerate する。unreachable なセルは「Not Applicable」
として明示し、理由を記載する。

| A × B × C | Ideal 出力 | 現状 | 判定 | 本 PRD Scope? |
|-----------|----------|------|------|--------------|
| ...       | ...      | ...  | ✓/✗/要調査 | Yes/No/別 PRD |

### 未確定セル (Discovery で解消)

判定「要調査」のセルを列挙し、Discovery でユーザーに確認する ideal 出力を明示。
```

不明なセル・省略されたセル・「多分 OK」と推測したセルがあれば、PRD は未完成と扱う。

### 2. 次元の列挙は「網羅」が要件

典型的な次元例 (変換系機能の場合):
- **AST shape**: 関係する全 AST 種別を SWC の AST 定義から enumerate
  (Lit / Ident / Member(Computed vs Ident) / Call / OptChain / TsAs / TsNonNull /
  TsTypeAssertion / Arrow / Fn / Cond / Await / Unary / Bin / New / Paren / Seq /
  Array / Object / Tpl / ...)
- **TS type**: 関係する全 RustType variant を enumerate
  (Option / T primitive / Any / Unknown / TypeVar / Vec / HashMap / Tuple /
  Struct Named / Enum Named / Fn / DynTrait / Regex / ...)
- **Outer context**: expression が置かれる可能性のある全 context
  (return / var decl + annotation / var decl no annotation / assign target /
  call arg / destructuring default / class field init / ternary branch /
  match arm body / spread / template literal expr / await operand / ...)
- **TS strict 設定**: strictNullChecks / noUncheckedIndexedAccess etc. が挙動を
  変える場合、次元として追加

「代表的な variant のみ」「よくある組合せのみ」は **禁止**。組合せ爆発が発生する
場合でも、scope-out 判断は明示的 (別 PRD に分割、unreachable の justification 等)。

### 3. テストは問題空間マトリクスから導出する

テスト追加の基準を「実装した分岐」から「マトリクスのセル」に転換する:
- 各セルに対し **少なくとも 1 つ** のテストを対応させる (unit / integration / E2E のいずれか)
- 「✓ 現状で OK」のセルも regression lock-in test を書く (未来の変更から保護)
- 「✗ 修正対象」のセルは fix 後の ideal 出力を assert するテストを書く
- 「別 PRD」のセルは現状を文書化する test を書き、PRD コメントで future work を link

### 4. PRD 完了条件に「マトリクス全セルカバー」を含める

以下を満たしていない PRD は完了と認めない:
- Problem Space マトリクスの全セルに判定が付いている (✓ / ✗ / NA / 別 PRD)
- 全セルに対応するテストが実装済み (または意図的な NA 理由が PRD に記録済み)
- 実装完了後に **matrix audit pass** を実施し、各セルの実出力が ideal 仕様と一致することを確認

### 5. 敵対的自己レビュー

PRD 完了宣言前に、以下の自問を文書化する:

```markdown
## Matrix Completeness Audit

以下全て「Yes」であることを確認する。1 つでも「No / 不明」があれば完了不可。

- [ ] 機能が関与する全 AST shape を列挙したか?
- [ ] 全 TS type variant (Option, Any, TypeVar, Vec, HashMap, Struct, Enum, Fn,
       Primitive, Unknown, ...) を考慮したか?
- [ ] 全 outer context (return, assign, call arg, destructuring, ternary,
       match arm, spread, template, await, ...) を列挙したか?
- [ ] 上記の直積で未カバーのセルはないか?
- [ ] review agent (`/check_job`) が指摘する可能性のある edge case を先回りして
       考慮したか?
- [ ] 「このケースは稀だから」「多分動く」「時間がないから」を根拠にしたセル
       省略がないか?
```

### 6. Review で未認識セルが発見されたときの扱い

`/check_job` や user review で問題空間から漏れたセルが発見された場合:
1. そのセルが **本来マトリクスにあるべきだったか** を判定する。
2. YES なら: PRD が未完成だった証拠。本 PRD に含めるか、scope out を明示判断する。
   silent に別 TODO に逃がすことは禁止。
3. そのセルの出力を ideal 仕様に沿って fix する / 別 PRD に起票する決定を記録する。
4. 同時に「なぜこのセルを最初の enumerate で見落としたか」を振り返り、次元の
   列挙基準を補強する。

## Prohibited

- **問題空間の enumerate をスキップして Discovery / 設計 / 実装に進むこと**。
- **TODO defect のみを scope として PRD を起票すること** (defect は氷山の一角)。
- **実装した分岐のみのテストで完了とすること**。
- 「代表的な variant のみ」「よくある組合せのみ」「多分大丈夫」を理由にセルを省略すること。
- 「頻度が低い」「稀なケース」を根本解決の代替にすること (頻度は問題空間の尺度ではない)。
- Review で発見された新 edge case を silent に別 TODO に逃がし、マトリクスを
  更新しないこと。
- 組合せ爆発を理由に「サブセットのみテスト」と割り切ること (scope-out するなら
  別 PRD に分割、しないなら全カバー)。
- 「PRD 完了後に次のレビューで判明する」を受け入れること (それは本ルールの失敗)。

## 関連ルール

- `ideal-implementation-primacy.md`: 本ルールはその最上位原則の **実装手段**。
  「理想的な transpiler」は問題空間の網羅なしに達成不可能。
- `prd-completion.md`: 完了条件にマトリクス全カバーを含める。
- `prd-template` skill: Step 0 で本ルールを適用する (Discovery の前)。
- `conversion-correctness-priority.md`: マトリクス各セルで Tier 判定を行う。
- `todo-entry-standards.md`: 本 PRD 外に切り出したセルを TODO 化する際の記載標準。

## Rationale

過去の defect 修正 PRD で、完了宣言 → review → 追加 defect 発見 → 修正 → 再 review →
さらに defect 発見、というサイクルが繰り返された。この pattern は以下に由来する:

1. **反応的実装**: TODO defect を fix するだけで、問題空間を事前に map していない。
2. **テストが実装追従**: 実装した分岐のみテスト。未実装セルは当然テストされず、
   silent bug が残る。
3. **「完了」定義の誤り**: 「reported defect が fix された + テスト pass」= 完了
   としていた。「問題空間全セルが ideal 出力」= 完了ではなかった。
4. **形式仕様の不在**: 「TS X → Rust Y」の形式仕様が存在しない。実装がアドホック。

本ルールは上記を構造的に解消する: 仕様駆動 (問題空間マトリクスが仕様) により
「完了 = 全セルが仕様通り」を達成する。
