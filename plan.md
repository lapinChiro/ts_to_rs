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

### Phase D タスク概観

| タスク | 内容 | 状態 |
|---|---|---|
| D-0a | PRD-β 起票: `TypeDef::ExternalUnsupported` variant (DOM 型 16 件 + symbol 1 件) | ⏳ |
| D-0b | PRD-γ 起票: `__type` marker → function type 是正 (1 件) | ⏳ |
| D-0c | PRD-δ 起票: Pass 5c 再設計 = `generate_stub_structs` 削除 + user 型 import 生成 | ⏳ |
| D-1 | PRD-A-2 (= I-386) 実装: resolve_type_ref Step 3 + 73 件 bug-affirming test 根絶 | ⏳ |
| D-2 | PRD-β / PRD-γ / PRD-δ 実装 | ⏳ |
| D-3 | `generate_stub_structs` 完全削除 + regression test 追加 | ⏳ |
| D-4 | 最終 quality check + ドキュメント整理 | ⏳ |

### 直近アクション (次セッション開始時)

1. **PRD-β / PRD-γ / PRD-δ 起票** (Phase D 準備): Phase C で確定した IR 構造化基盤上で
   各 PRD の Discovery → 設計 → 起票を順次実施。
2. **PRD-A-2 (I-386) 実装**: backlog/I-386 として既に起票済の resolve_type_ref Step 3 +
   73 件 fixture 整理を実装着手。I-382 本体の前提条件として必要。
3. **Phase D 実装**: 上記 PRD の TDD 実装。各 PRD 完了ごとに probe 再投入で dangling refs
   件数を計測、最終的に 0 化を目指す。

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

## ベースライン (2026-04-08, I-387 Phase C 完了時点)

| 指標 | Phase C 開始時 | Phase C 完了時 |
|---|---|---|
| Hono クリーン | 114/158 (72.2%) | 114/158 (72.2%) |
| エラーインスタンス | 54 | 54 |
| コンパイル (file) | 113/158 (71.5%) | 113/158 (71.5%) |
| コンパイル (dir) | 157/158 (99.4%) | 157/158 (99.4%) |
| テスト数 | 2228 (lib) | 2259 (lib) |
| clippy warning | 0 | 0 |
| dangling refs (probe) | 23 | Phase D 着手時に再計測 |

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
