# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在のフェーズ: I-382 解消プロジェクト Phase D (I-382 本体実装)

**目的**: Phase C で整備された IR 構造化基盤の上で、`generate_stub_structs` を削除し
Pass 5c を「synthetic_items が参照する user 定義型に対する `use crate::<path>::Type;`
生成」のみに再設計する (I-382 本体)。

**詳細計画**: [`report/i382/master-plan.md`](report/i382/master-plan.md)

### フェーズ構成 (概観)

```
Phase A  ✅  調査 (INV-1〜9 解消、2026-04-08 完了)
       ↓
Phase B  ✅  PRD 起票 (I-387, 2026-04-08 完了)
       ↓
Phase C  ✅  理想実装 (I-387 T1〜T14 全件完了、2026-04-08)
       ↓
Phase D  🔄  I-382 本体 (generate_stub_structs 削除) ← 現在地
```

### Phase C 完了サマリ (I-387, 2026-04-08)

- `RustType` に `TypeVar { name }` / `Primitive(PrimitiveIntKind)` /
  `StdCollection { kind, args }` variant を追加し、`Named` は user 定義型のみを表す
  正規形に昇格。
- `substitute` の legacy `Named{"T"}` 後方互換ブランチを削除し、`TypeVar { name }` を
  型変数 substitution の唯一の正規形に確定。
- `collect_free_type_vars` heuristic と `RUST_BUILTIN_TYPES` 文字列フィルタを構造的に
  削除 (walker ベース `collect_type_vars` で置換)。
- `monomorphize_type_params` に `Some(RustType::TypeVar{..}) => defer` 分岐を追加し、
  チェーン制約 (`U extends T, T extends number`) の段階的解決 semantics を明示化。
- `cargo test --lib` 2259 pass / `cargo clippy` 0 warning / Hono bench regression 0。
- PRD Goal #1〜#9 全達成、I-387 archive 済。

### Phase D 実行順序 (確定, 2026-04-10 更新)

各 PRD は **調査→起票→実装→次の調査** のサイクルで直列実行する。
前の PRD の実装結果が次の PRD の前提を変える可能性があるため、
起票を一括で行わず実測ベースで逐次進める。

PRD 実施順序: **γ → β → δ** (依存関係 + 影響範囲の段階的拡大)

```
Step 0    Probe 再計測                                        ✅ 完了
Step 0.5  `P` 残存調査・解消                                   ✅ 完了
  ↓
Step 1    PRD-γ 調査→起票→実装 (__type, 1 件, 小 scope)        ✅ 完了
  ↓         γ で external 型処理フローを把握 → β の Discovery に有益
Step 2    PRD-β 調査→起票→実装 (DOM 型等, 21 件, 中 scope)     ✅ 完了
  ↓         β で外部型 JSON の参照整合性を確保 → δ の設計に直結
Step 3    PRD-δ 調査→起票→実装 (I-382 本体, 71+1 件, 大 scope)  ← 現在地
  ↓         β/γ で dangling 解消済の状態で generate_stub_structs 削除
Step 4    最終 quality check + ドキュメント整理
```

**γ→β→δ の根拠**:
- δ は β/γ 両方の完了が前提 (dangling refs 0 化に必要) → δ は必ず最後
- γ (小 scope) で「調査→PRD→実装」サイクルを低リスクで検証し、
  `convert_external_type` の仕組みを把握 → β の Discovery 精度向上
- β (中 scope) で `TypeDef` / `external_struct_generator` を変更 → δ の設計に直結

### PRD 間のスコープ決定 (2026-04-10)

| 決定 | 理由 | 影響先 |
|---|---|---|
| **PRD-γ**: `__type` を抽出ツール側 (`extractor.ts`) で構造的に修正 + JSON 再生成 | `__type` は抽出ツールのバグ (anonymous type literal の symbol 名をそのまま出力)。Rust loader 側の `Any` fallback はアドホックな patch であり禁止。root cause は `extractor.ts:434-436` | Rust loader 側の変更不要 |
| **`symbol`**: PRD-γ から除外し **PRD-β** に委譲 | `symbol` は TS primitive (`TypeFlags.ESSymbol`) の抽出漏れであり、compiler internal marker (`__type`) とは異なるカテゴリ。「TS primitive の Rust 表現」は PRD-β (unsupported external types) の責務 | **PRD-β は `symbol` の扱いを含めて設計すること** |

### Phase D タスク一覧

| タスク | Step | 内容 | 状態 |
|---|---|---|---|
| D-0 | 0 | Probe 再計測: Phase C 後の dangling refs 実測 | ✅ |
| D-0.5 | 0.5 | `P` 残存調査・解消 (Cluster 1a regression) | ✅ |
| D-1 | 1 | PRD-γ 調査→起票→実装: `__type` marker 是正 (I-389) | ✅ |
| D-2 | 2 | PRD-β 調査→起票→実装: フィルタ参照整合性 + 外部型完全化 (I-391) | ✅ |
| D-3 | 3 | PRD-δ 調査→起票→実装: Pass 5c 再設計 + `generate_stub_structs` 削除 | ⏳ |
| D-4 | 4 | 最終 quality check + ドキュメント整理 | ⏳ |

### 直近アクション

1. ~~D-1: PRD-γ~~ ✅ 完了 (I-389, 2026-04-10)
   - `extractor.ts::convertType` に `__type` symbol 検出 + 4 パターン展開
   - dangling 23→22
2. ~~D-2: PRD-β~~ ✅ 完了 (I-391, 2026-04-10)
   - `filter.ts` 参照整合性チェック + root types 追加 (BufferSource, TemplateStringsArray 等)
   - `extractor.ts` ESSymbol primitive 認識、`external_types/mod.rs` alias → 空 Struct
   - dangling 22→**1** (残 HTTPException = PRD-δ scope)、excluded_user 72→71
3. **D-3: PRD-δ 調査 (Discovery)** ← 次のアクション
   - I-382 本体: `generate_stub_structs` 削除 + user 型 import 生成

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
| **24** | **I-388** | **TypeCollector / TypeConverter 二重変換経路の乖離** [L2] |

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

## ベースライン (2026-04-08, I-387 Phase C 完了時点)

| 指標 | Phase C 開始時 | Phase C 完了時 |
|---|---|---|
| Hono クリーン | 114/158 (72.2%) | 114/158 (72.2%) |
| エラーインスタンス | 54 | 54 |
| コンパイル (file) | 113/158 (71.5%) | 113/158 (71.5%) |
| コンパイル (dir) | 157/158 (99.4%) | 157/158 (99.4%) |
| テスト数 | 2228 (lib) | 2259 (lib) |
| clippy warning | 0 | 0 |
| dangling refs (probe) | 23 | **1** (2026-04-10、D-0.5 + D-1 + D-2 修正後) |
| テスト数 (extractor) | — | 70 (+12 新規) |
| テスト数 (Rust lib) | 2228 | 2262 (+3 新規) |

**注**: 上記は現状把握用の指標。最適化目標ではない
(`.claude/rules/ideal-implementation-primacy.md` 参照)。

### Phase D Probe 計測推移 (2026-04-10)

詳細: [`report/i382/phase-d-probe.md`](report/i382/phase-d-probe.md)

| Category | Phase A | D-0 初回 | D-0.5 (P修正) | D-1 (__type修正) | D-2 (フィルタ+alias+symbol) |
|---|---|---|---|---|---|
| dangling | 34 | 24 | 23 | 22 | **1** |
| excluded_user | 73 | 72 | 72 | 72 | **71** |

- D-0.5: `P` type param leak 解消 (`collection.rs` scope 欠落修正)
- D-1: `__type` compiler marker 解消 (`extractor.ts` anonymous type literal 展開)
- D-2: フィルタ参照整合性 + alias 変換 + symbol primitive (21 件解消)

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
