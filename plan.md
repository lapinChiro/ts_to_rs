# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在のフェーズ: Batch 25 (I-392) Phase 0-8 完了、Phase 9 待ち

Multi-overload callable interface を trait + marker + impl 表現に変換。
PRD Revision 3.3 (フェーズ構成・チェックポイント最適化レビュー反映済)。

### Phase 0 完了内容 (2026-04-12)

- **P0.0**: Baseline 計測 (2297 test, cov 91.63%, Hono 71.5%/58err)
- **P0.1**: IfLet/Match 調査 — IfLet は ternary narrowing で arrow return 位置に発生。
  Match は不発生 (YAGNI)。Phase 6 設計確定
- **P0.2**: Promise 調査 — `RustType::unwrap_promise()` 未存在。Phase 4.2 で新規追加
- **P0.3**: L2/L3/L4 verification — real bug 1 件 (L2-4 indent cosmetic)。Phase 12 で fix
- **P0.4**: Transformer factory method refactor — `spawn_nested_scope` +
  `spawn_nested_scope_with_local_synthetic` 新規作成、12 production サイト移行、
  INV-8 lint script (`scripts/check-transformer-construction.sh`) 作成

### Phase 1 完了内容 (2026-04-12)

- **P1.1**: `Item::Const { vis, name, ty, value }` variant 追加 (fold/visit/generator 対応)
- **P1.2**: `Method::is_async: bool` field 追加 (全 17 構築サイト更新)
- **P1.3**: generator で trait/impl method の `async fn` keyword 出力
- **P1.4**: `function.is_async` → `Method::is_async` propagation + async-class-method fixture
- **P1.5**: `convert_var_decl_module_level` rename + const-safe Lit init (`Num`/`Bool`/`Null`)
  → `Item::Const` 生成。型注釈なしリテラルは `infer_const_type` で型推論。
  String/Regex/BigInt は const-safe でないため skip (follow-up PRD)

### Phase 2 完了内容 (2026-04-12)

- **P2.1**: `CallableInterfaceKind` enum + `classify_callable_interface` 関数
  (unit test 8 件: NonCallable×5 + SingleOverload + MultiOverload + ConstValue)
- **P2.2**: INV-2 lint script (`scripts/check-classify-callable-usage.sh`)
  既存 violation を warning 検出 (Phase 4/9 で修正予定)
- **P2.3**: Pass 2 を non-Var (2a) / Var (2b) に分割。
  Pass 2b は Pass 2a 完了後の snapshot を lookup に使用
- **P2.4**: `collect_decl` Var branch で callable interface arrow →
  `ConstValue { type_ref_name }` 登録。non-callable は従来通り `Function`
- `collection` module を `pub(crate)` に変更 (Phase 4.3 で transformer がアクセス)

### Phase 3 完了内容 (2026-04-12)

- **P3.1-3.3**: `overloaded_callable.rs` 新規作成
  - `compute_widest_params`: 各 position で型 unify + 不在は Option wrap
  - `compute_union_return`: divergent → synthetic union enum、mixed void/non-void → Option
  - `WidestSignature` struct (params, return_type, return_diverges)
  - unit test 9 件

### Phase 4 完了内容 (2026-04-12)

- **P4.1**: `convert_callable_interface_as_trait` — callable interface → `Item::Trait`。
  各 call signature を `call_N` method に展開。snapshot 3件更新、compile_test 3件一時除外
- **P4.2**: `RustType::unwrap_promise()` + `is_promise()` 追加。trait method + class method
  の Promise<T> → T unwrap。async-class-method compile_test 復帰。INV-6 lint script 作成
- **P4.3**: `callable_trait_name_and_args` + `convert_callable_trait_const` skeleton。
  callable interface const を既存 Fn path からバイパスし trait const path にルーティング

### Phase 5 完了内容 (2026-04-12)

- **P5.1**: `used_marker_names` field + `allocate_marker_name` (collision suffix loop) +
  `marker_struct_name` (camelCase/snake_case 対応)。unit test 5 件
- **P5.2**: `Item::Struct` に `is_unit_struct: bool` 追加 (60+ 構築サイト更新)。
  generator で `struct Name;` + `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`。
  unit test 2 件
- **P5.3**: `Expr::StructInit { fields: [] }` → `Name` (unit struct syntax)
- **P5.4**: `convert_callable_trait_const` に widest signature 計算 + marker struct +
  inner fn 生成。deep deep review で critical 修正: inner params を arrow param 名に修正
  (widest 名だと body の変数参照が不一致)。fixture 3 件 (callable-interface 更新 +
  param-rename 新規 + inner 新規)

### Phase 6 完了内容 (2026-04-12)

- **P6.0**: `return_wrap_ctx: Option<ReturnWrapContext>` field + `spawn_nested_scope_with_wrap`
  factory。全 factory method で None 伝搬 (INV-8 leak 防止)
- **P6.1**: `return_wrap.rs` 新規作成 — `ReturnWrapContext`, `build_return_wrap_context`,
  `wrap_leaf`, `variant_for`, `unique_option_variant` + unit test 12 件
- **P6.2-P6.4**: `wrap_body_returns`, `wrap_expr_tail` (If/IfLet 対応) を実装。
  ただし inner fn body への return wrap 適用は TypeResolver 型情報不足のため Phase 7 に先送り。
  インフラは `#[allow(dead_code)]` で保持
- **P6.5**: CLI synthetic items 結合修正 (`main.rs::transpile_file` に
  `render_referenced_synthetics_for_file` 追加) + builtin 名前衝突対策
  (fixture `Transformer` → `StringMapper` に変更)

### Phase 9A (前提 + P9.1) 完了、P9.2 待ち

**次: P9.2 (resolve_fn_type_info widest 書き換え + INV-6)**

### Phase 9A 完了内容 (2026-04-13)

**Phase 9 前提**: `return_wrap_ctx` field + `spawn_nested_scope_with_wrap` method を削除。
二相分離アプローチ (P7.0) で scope-based wrapping は不要と確定。
全 Transformer 構築サイト (production + test 13 箇所) から `return_wrap_ctx: None` を除去

**P9.1**: arity validation (INV-4)。`trait_type_args.len() != trait_type_params.len()` で
hard error。`callable-interface-generic-arity-mismatch` error-case fixture + compile_test
skip 追加 + integration test (`expect unsupported`)

### Phase 8 完了内容 (2026-04-13)

**P8.1**: `convert_callable_trait_const` 末尾に `Item::Const` emission 追加。
`const getCookie: GetCookieGetCookieImpl = GetCookieGetCookieImpl;` 形式の
module-level const instance を生成。4 fixture の snapshot 更新

**P8.2**: 変換側統合チェックポイント。
- callable-interface-inner / callable-interface-async の fixture body を単純化
  (Option narrowing I-360 の pre-existing bug 回避、PRD H3 fixture body 制限に準拠)
- compile_test.rs の skip リストから callable-interface 系 6 fixture を全て復帰
  (callable-interface, call-signature-rest, interface-mixed, callable-interface-param-rename,
  callable-interface-inner, callable-interface-async)
- `async-class-method` の `skip_compile_with_builtins` stale entry も修正
  (P4.2 exit criteria の incomplete 分)
- `Box<dyn Fn(` パターンが callable-interface snapshot に残っていないことを確認
- doc comment stale 修正 (`convert_callable_trait_const`)
- 全テスト pass、clippy 0、fmt 0

### Phase 7 完了内容 (2026-04-13)

**P7.0**: 二相分離アプローチ (SWC 型収集 + IR wrapping)。collect_return_leaf_types 新規、
wrap_leaf / wrap_body_returns / wrap_expr_tail を Iterator ベースに変更。
unit test 12 件 (collect 5 + wrap_leaf 2 + infer_variant 5)

**P7.1**: build_delegate_impl / build_delegate_method (Result 型)。
non-divergent → direct return、divergent → match unwrap。variant_for 失敗時 explicit error

**P7.2**: wrap_delegate_arg — bare / Some / Variant / Some+Variant の 4 パターン。
unit test 4 件

**P7.3**: async delegate に .await。compute_union_return で Promise unwrap してから
union 作成。callable-interface-async fixture (single + multi-overload)

**Pipeline Integrity 修正**: CallTarget::Free → BuiltinVariant::Some / UserEnumVariantCtor。
infer_variant_from_expr のマッチ側も修正 + 全 5 分岐の branch coverage test

### 現在の状態 (2026-04-13 Phase 8 完了時)

- **Test count**: 全テスト pass (lib 2365, integration 94, compile 4, E2E 88)
- **Quality**: clippy 0, fmt 0
- **Compile test skip**: callable-interface 系は全て復帰済。async-class-method も復帰済
- **#[allow(dead_code)]**: production code に 0 件 (Phase 9A で return_wrap_ctx / spawn_nested_scope_with_wrap を削除済)
- **Phase 9 設計注記**: P9.1 error-case fixture に compile_test skip 追加が必要。
  P9.2 synthetic 引数の TypeResolver からの伝搬経路を確立する必要あり。
  INV-2 lint script の exit code 変更は全 violation 解消まで保留

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
