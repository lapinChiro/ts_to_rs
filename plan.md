# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在のフェーズ: I-382 解消プロジェクト Phase B (PRD 起票)

**目的**: `generate_stub_structs` を構造的に不要にする理想実装 (`RustType::TypeVar` 変種導入) を
設計するための**不確定性解消 (Investigation Debt)** フェーズ。

今セッションで Cluster 1a (型パラメータ leak 11 件) を解消したが、実装は構造的 patch であり、
単一 IR 設計欠陥 (`RustType::Named` が type variable と named type を区別しない) の症状対処
に留まっている。理想実装に進む前に、影響範囲を絞り込むための調査債務 9 件 (INV-1 〜 INV-9) を
解消する必要がある。

**詳細計画**: [`report/i382/master-plan.md`](report/i382/master-plan.md)

### フェーズ構成 (概観)

```
Phase A (現在地)    調査 (INV-1〜9 解消)
       ↓
Phase B             PRD 起票 (TypeVar refactoring)
       ↓
Phase C             理想実装 (TDD, interim patch 削除)
       ↓
Phase D             I-382 本体 (generate_stub_structs 削除)
```

### 直近アクション

Phase A (調査債務 9 件) は **2026-04-08 完了**。結果は
[`report/i382/phase-a-findings.md`](report/i382/phase-a-findings.md)。

次アクションは Phase B:
1. PRD-TypeVar 起票 (primary 変更点: `type_converter/mod.rs::convert_ts_type`)
2. 並行で PRD-β (`TypeDef::ExternalUnsupported`) / PRD-γ (`__type` 是正) 起票
3. Design Integrity / Semantic Safety / 凝集度レビューを経て確定

---

## I-382 解消後のタスク候補 (参考)

I-382 本体完了後に着手する L3 バッチ。優先度は `.claude/rules/todo-prioritization.md` に従い、
Phase A 完了後の実測値で再評価する。

| Batch | イシュー | 根本原因 |
|---|---|---|
| 11b | I-300 + I-301 + I-306 | OBJECT_LITERAL_NO_TYPE |
| 12 | I-311 + I-344 | 型引数推論フィードバック欠如 |
| 13 | I-11 + I-238 + I-202 | union/enum 生成品質 |
| 15 | I-340 | Generic Clone bound 未付与 |
| 16 | I-360 + I-331 | Option narrowing + 暗黙 None |
| 17 | I-321 | クロージャ Box::new ラップ漏れ |
| 18 | I-217 + I-265 | iterator クロージャ所有権 |
| 19 | I-336 + I-337 | abstract class 変換パス欠陥 |
| 20 | I-329 + I-237 | string メソッド変換 |
| 21 | I-313 | 三項演算子 callee パターン |
| 22 | I-30 | Cargo.toml 依存追加 (I-183, I-34 のゲート) |
| 23 | I-182 | コンパイルテスト CI 化 |

L4 候補と詳細は [`TODO`](TODO) 参照。

---

## 設計判断の引継ぎ (後続 PRD 向け)

### `PrimitiveType` 9 variant の YAGNI 例外 (I-378)

`src/ir/expr.rs::PrimitiveType` は 9 variant 定義で、production で使われるのは `F64` のみ
(`f64::NAN` / `f64::INFINITY`)。I-378 で「9 variant 維持」を採用した。理由: (1) 基盤型と
しての概念的完全性、(2) 将来 `i32::MAX` 等で再追加する総コストが現状維持より高い、
(3) variant 網羅テストで dead_code lint 発火しない。

**引継ぎ**: 後続 PRD で primitive associated const を使う際、既存 variant をそのまま利用すべき。

### `switch.rs::is_literal_match_pattern` の意味論微変化 (I-378)

判定基準を `name.contains("::")` 文字列マッチから `Expr::EnumVariant` 構造マッチに変更。
`case Math.PI:` / `case f64::NAN:` のような (TS で稀な) ケースは guarded match に展開される。
Hono 後退ゼロ確認済。

**引継ぎ**: 将来 `case` で primitive const / std const を使う TS fixture を追加する場合、
`is_literal_match_pattern` に `Expr::PrimitiveAssocConst { .. } | Expr::StdConst(_) => true`
追加を検討。ただし `f64` 値の pattern matching は Rust で unstable のため guarded match が安全。

### 新規 integration test (削除禁止)

`tests/enum_value_path_test.rs` / `tests/math_const_test.rs` / `tests/nan_infinity_test.rs`
(I-378 追加) は `Expr::EnumVariant` / `PrimitiveAssocConst` / `StdConst` 構造化の lock-in
テスト。**削除・スキップ禁止**。

### 残存 broken window

- **`Item::StructInit::name: String`** に display-formatted `"Enum::Variant"` 形式が格納される
  (`transformer/expressions/data_literals.rs:90`)。`StructInit` IR に
  `enum_ty: Option<UserTypeRef>` を追加して構造化すべき。**PRD 化候補**。

---

## ベースライン (2026-04-08)

| 指標 | 値 |
|---|---|
| Hono クリーン | 114/158 (72.2%) |
| エラーインスタンス | 54 |
| コンパイル (file) | 113/158 (71.5%) |
| コンパイル (dir) | 157/158 (99.4%) |
| テスト数 | 2228 (lib) |
| dangling refs (probe) | 23 (Cluster 1a 完全解消後) |

**注**: 上記は現状把握用の指標。最適化目標ではない
(`.claude/rules/ideal-implementation-primacy.md` 参照)。

---

## リファレンス

- 最上位原則: `.claude/rules/ideal-implementation-primacy.md`
- 優先度ルール: `.claude/rules/todo-prioritization.md`
- TODO 記載標準: `.claude/rules/todo-entry-standards.md`
- I-382 マスタープラン: `report/i382/master-plan.md`
- I-382 履歴: `report/i382/history.md`
- セッション発見 TODO: `report/i382/session-todos.md`
- TODO 全体: `TODO`
- 調査レポート: `report/`
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
