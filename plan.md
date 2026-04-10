# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在のフェーズ: 次タスク選定待ち

backlog/ は空。
次の作業は TODO の優先度に基づき PRD 化して着手する。

---

## 次のタスク候補

TODO の L2/L3 バッチ。優先度は `.claude/rules/todo-prioritization.md` に従い、
実測値で再評価する。

| Batch | イシュー | 根本原因 |
|---|---|---|
| **25** | **I-392** | **overload 最大 params のみ採用 (L1 edge + L2)** |
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

## 完了済プロジェクト

### I-388: TypeCollector resolve 関数統一 (2026-04-10)

`collect_type_alias_fields` / `collect_type_lit_fields` / `resolve_type_ref_fields` の 3 関数を
廃止し、`convert_to_ts_type_info` 起点の 3 パス（TypeLiteral / Intersection / TypeRef）に再構成。

- SWC AST のアドホック解析を排除し、TsTypeInfo を唯一の中間表現に統一
- `resolve_struct_members` を typedef.rs に抽出し DRY 維持（resolve_typedef と collection.rs で共有）
- Utility type alias (`Partial<T>`, `Pick<T, K>` 等) の TypeDef 登録を修正
- Bug-affirming test (`test_type_alias_type_ref_with_utility_type`) を修正
- `TsMethodInfo` に `has_rest` フィールド追加（既存の破棄を修正）
- Intersection method の lossy 変換を排除（`convert_method_info_to_sig` → `resolve_method_sig`）

**指標**: Hono bench regression 0、cargo test 2295 pass、clippy 0 warning
**後続課題**: I-394 (TypeDef::Alias variant 追加)、I-395 (TsMethodInfo type_params 制約情報)

### I-382 解消プロジェクト (2026-04-08 〜 2026-04-10)

`generate_stub_structs` を完全削除し、Pass 5c を user 定義�� import 自動生成に再設計。

- **Phase A**: 調査債務 INV-1〜9 解消
- **Phase B**: PRD I-387 起票
- **Phase C**: IR 構造化 (`RustType::TypeVar` / `Primitive` / `StdCollection` 導入、
  heuristic 削除、interim patch ��去)
- **Phase D**: I-382 本体 (PRD-γ `__type` 是正 → PRD-β 外部型完全化 → PRD-δ stub 削除 + import 生成)

**最終指標** (Phase D 完了時):
- Hono bench regression 0 (clean 114/158, errors 54)
- `generate_stub_structs` grep ヒット 0
- `cargo test --lib` 2275 pass / clippy 0 warning
- dangling refs: stub 機構削除により計測方式自体を廃止

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
