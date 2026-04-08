# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在のフェーズ: I-382 解消プロジェクト Phase C (I-387 実装)

**目的**: `generate_stub_structs` を構造的に不要にする理想実装。核心的 IR 設計欠陥
(`RustType::Named` が type variable / user type / std type を区別しない) を
`TypeVar` / `Primitive` / `StdCollection` variant 導入で構造的に解決する。

**詳細計画**: [`report/i382/master-plan.md`](report/i382/master-plan.md) /
PRD: [`backlog/I-387-rust-type-structural-refinement.md`](backlog/I-387-rust-type-structural-refinement.md)

### フェーズ構成 (概観)

```
Phase A  ✅  調査 (INV-1〜9 解消、2026-04-08 完了)
       ↓
Phase B  ✅  PRD 起票 (I-387, 2026-04-08 完了)
       ↓
Phase C  🔄  理想実装 (I-387, T1〜T14) ← 現在地
       ↓
Phase D      I-382 本体 (generate_stub_structs 削除)
```

### Phase C 進捗 (2026-04-08 時点)

| タスク | 内容 | 状態 |
|---|---|---|
| T1 | `RustType::TypeVar / Primitive / StdCollection` variant 追加 | ✅ |
| T2 | substitute に TypeVar branch 追加 (legacy Named{"T"} 後方互換は残置) | ✅ |
| T3 | generator に新 variant 生成 + Semantic Safety 等価性テスト | ✅ |
| T4a | `primitive_int_kind_from_name` / `std_collection_kind_from_name` ヘルパー | ✅ |
| T4b | TypeVar routing 有効化 + 下流両対応化 (type_resolver) | ✅ |
| T4c | Primitive/StdCollection routing + 下流両対応化 (transformer) | ✅ |
| T4d | BigInt / Record / Map / Set の構造化 routing | ✅ |
| T5 | c1 (既存 variant 巻戻し) 構築サイト置換 | ✅ |
| T6 | c2 (Primitive/StdCollection) 構築サイト置換 | ✅ |
| T7 | (b) TypeVar 構築サイト置換 (production 完了、test fixture 残) | 🔄 部分完了 |
| T8 | interim patch T2.A-i 処理 (scope push は lexical scope として残置、heuristic は walker に置換) | ✅ |
| T9 | interim patch T2.A-ii 処理 (enter_type_param_scope を lexical scope semantics に relabel) | ✅ |
| T10 | `collect_free_type_vars` heuristic 削除 + `collect_type_vars` walker | ✅ |
| T11 | `extract_used_type_params` を walker-only 実装に置換 | ✅ |
| T12 | 下流 pattern match 更新 | ✅ (T4b/T4c に統合) |
| T13 | plan.md / master-plan.md / history.md 更新 | 🔄 本回更新中 |
| T14 | /quality-check + Hono bench 最終確認 | ⏳ 未実施 |

### 直近アクション (次セッション開始時)

**残作業を優先順に実施**:

1. **T7 残り**: substitute.rs の **Named{"T"} 後方互換ブランチ削除** → 直後に壊れる
   test fixtures (registry/tests/generics.rs 等の `Named{"T"}` 用途) を全て `TypeVar{"T"}` に
   一括置換。
   - 該当箇所: `src/ir/substitute.rs:34-49 fold_rust_type` の 2 本目 `if let Named` ブランチ
   - 壊れるテスト: `src/registry/tests/generics.rs` 等の `ty: RustType::Named{name:"T"}` 系
   - 置換方法: `bulk-edit-safety.md` 準拠でドライラン → レビュー → 実行
2. **T14**: `/quality-check` (cargo fix / fmt / clippy / test) + 最終 `./scripts/hono-bench.sh`
   再実行 (既に regression 0 確認済だが PRD completion criteria 的に必須)
3. **T13 仕上げ**: master-plan.md / history.md を Phase C 完了状態に更新、session-todos.md
   の T-2/T-5/T-6/T-7/T-8 対応項目を削除
4. **PRD I-387 完了処理**: backlog/I-387 を archive、plan.md の「現在地」を Phase D に更新
5. **Phase D 着手**: PRD-β (`TypeDef::ExternalUnsupported`) / PRD-γ (`__type` 是正) /
   PRD-δ (Pass 5c 再設計 = I-382 本体) の起票

### セッション中断ポイント (中断直前の状態)

- **`cargo test --lib`**: 2259 passed / 0 failed
- **`cargo clippy --all-targets`**: 0 warning
- **Hono bench**: clean 114/158, errors 54, compile (dir) 99.4% — **ベースライン維持**
- PRD Goal #4/#7 達成 (interim heuristic 削除、`RUST_BUILTIN_TYPES` 定数削除)
- Goal #9 は production では達成済、意図的に残す Semantic Safety 等価テスト 2 件のみ例外

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

## ベースライン (2026-04-08, I-387 Phase C 実装中)

| 指標 | セッション開始時 | 現在 |
|---|---|---|
| Hono クリーン | 114/158 (72.2%) | 114/158 (72.2%) |
| エラーインスタンス | 54 | 54 |
| コンパイル (file) | 113/158 (71.5%) | 113/158 (71.5%) |
| コンパイル (dir) | 157/158 (99.4%) | 157/158 (99.4%) |
| テスト数 | 2228 (lib) | 2259 (lib) |
| dangling refs (probe) | 23 | 未再計測 (T14 で再計測予定) |

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
