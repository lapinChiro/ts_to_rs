# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-13)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 114/158 (72.2%) |
| Hono bench errors | 57 |
| cargo test (lib) | 2383 pass |
| cargo test (integration) | 99 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 89 pass |
| coverage | 91.68% (threshold 90%) |
| clippy | 0 warnings |
| fmt | 0 diffs |

---

## 次のタスク候補

TODO の Tier 1 (L3) クラスタを優先度順にバッチ化。
優先度は `.claude/rules/todo-prioritization.md` に従い、実測値で再評価する。

| 優先度 | クラスタ | イシュー | 根本原因 | Hono 影響 |
|--------|----------|----------|----------|-----------|
| 1 | RC-11 | I-300 + I-301 + I-306 | OBJECT_LITERAL_NO_TYPE | 25 件 |
| 2 | RC-9 | I-311 + I-344 | 型引数推論フィードバック欠如 | — |
| 3 | RC-13 | I-11 + I-238 + I-202 | union/enum 生成品質 | skip 原因 |
| 4 | RC-2 | I-340 + I-217 + I-265 | Generic Clone bound / iterator 所有権 | — |
| 5 | — | I-360 + I-331 | Option narrowing + 暗黙 None | skip 原因 |
| 6 | RC-14 | I-397 | module-level const 変換拡張 | — |
| 7 | — | I-321 | クロージャ Box::new ラップ漏れ | — |
| 8 | RC-5 | I-336 + I-337 | abstract class 変換パス欠陥 | — |
| 9 | RC-12 | I-329 + I-237 | string メソッド変換 | — |
| 10 | — | I-313 | 三項演算子 callee パターン | CALL_TARGET 4 件 |
| 11 | — | I-30 | Cargo.toml 依存追加 (I-183, I-34 のゲート) | — |
| 12 | — | I-182 | コンパイルテスト CI 化 | — |

Tier 2 (L3 残り + L4) 以降は [`TODO`](TODO) 参照。

---

## 設計判断の引継ぎ (後続 PRD 向け)

### `push_type_param_scope` は correct design であり interim ではない (I-387)

PRD 起票時は `push_type_param_scope` を完全削除する想定だったが、実装調査で方針変更:

- `convert_external_type` (外部 JSON ローダ) と `convert_ts_type` (SWC AST コンバータ) は
  独立した 2 つの変換経路。`convert_ts_type` の TypeVar routing を後者が直接流用できない
- `convert_external_type::Named` も scope を参照して TypeVar routing する必要があり、
  scope 自体は「lexical scope management」として残すのが構造的に正しい
- 「interim」だったのは scope を介してフィルタ判定していた `extract_used_type_params` の
  heuristic 部分であり、それは walker-only 実装 (`collect_type_vars`) で完全置換済

**引継ぎ**: scope push を見て「interim 残存では?」と思った場合、上記の判断に立ち戻ること。

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

## リファレンス

- 最上位原則: `.claude/rules/ideal-implementation-primacy.md`
- 優先度ルール: `.claude/rules/todo-prioritization.md`
- TODO 記載標準: `.claude/rules/todo-entry-standards.md`
- TODO 全体: `TODO`
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
