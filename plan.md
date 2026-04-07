# ts_to_rs 開発計画

## 次のアクション

**I-376** (Batch 11c-fix-2-c): per-file 外部型 stub の構造的重複 — pipeline 段階 dedup。
PRD 未作成、Discovery 必要。`pipeline/mod.rs` Pass 4/5 plumbing で IR 層と直交。

---

## 現在のフェーズ: コンパイル品質 + 設計基盤

フェーズ移行基準: **S1 バグ 0 + ディレクトリコンパイルエラー 0**
現状: S1 残 0 件、ディレクトリコンパイル 157/158

優先度判定は `.claude/rules/todo-prioritization.md` の L1〜L4。L1 → L2 → L3 → L4 の順、
同一レベル内はレバレッジ → 拡大速度 → 修正コストの順。

### 未実施バッチ (L3)

| Batch | イシュー | 根本原因 | 備考 |
|-------|---------|---------|------|
| 11c-fix-2-c | I-376 | per-file 外部型 stub の構造的重複 (pipeline 段階 dedup) | PRD 未作成、Discovery 必要 |
| 11b | I-300 + I-301 + I-306 | OBJECT_LITERAL_NO_TYPE (25 件) | 最大エラーカテゴリ削減 |
| 12 | I-311 + I-344 | 型引数推論フィードバック欠如 | I-344 自動解消 + generic 精度 |
| 13 | I-11 + I-238 + I-202 | union/enum 生成品質 | skip: ternary, ternary-union 他 |
| 15 | I-340 | Generic Clone bound 未付与 | generic コード増に比例 |
| 16 | I-360 + I-331 | Option\<T\> narrowing + 暗黙 None | skip: functions 部分 |
| 17 | I-321 | クロージャ Box::new ラップ漏れ | skip: closures, functions 部分 |
| 18 | I-217 + I-265 | iterator クロージャ所有権 | skip: array-builtin-methods |
| 19 | I-336 + I-337 | abstract class 変換パス欠陥 | 安定 (拡大しない) |
| 20 | I-329 + I-237 | string メソッド変換 | skip: string-methods |
| 21 | I-313 | 三項演算子 callee パターン | CALL_TARGET 4 件 |
| 22 | I-30 | Cargo.toml 依存追加 | I-183, I-34 のゲート |
| 23 | I-182 | コンパイルテスト CI 化 | 回帰検出自動化 |

### L4: 局所的問題

バッチ化は L3 完了後。根本原因クラスタ単位で順次対応。
主要候補: I-322, I-326, I-330, I-332, I-314, I-201, I-209, I-310, I-345, I-342, I-260 他。

### 残存 broken window

- **`Item::StructInit::name: String`** に display-formatted `"Enum::Variant"` 形式が格納される
  (`transformer/expressions/data_literals.rs:90`)。`StructInit` IR に
  `enum_ty: Option<UserTypeRef>` を追加して構造化すべき。**PRD 化候補** (I-381 以降)。

---

## 設計判断 (後続セッションへの引継ぎ)

後続 PRD で考慮すべき設計判断・既知挙動・歴史的経緯。コードコメントや PRD に分散しない事項を集約。

### `PrimitiveType` 9 variant の YAGNI 例外 (I-378)

`src/ir/expr.rs::PrimitiveType` は 9 variant 定義。production で使われるのは `F64` のみ
(`f64::NAN` / `f64::INFINITY`) で残り 8 は variant 網羅テスト経由でのみ参照。

I-378 self-review で「F64 のみに削減」案と「9 variant 維持」案で議論し**維持を採用**。
理由: (1) 基盤型としての概念的完全性、(2) 将来 `i32::MAX` 等で再追加する総コストが現状維持より高い、
(3) variant 網羅テストで dead_code lint 発火しない。

**引継ぎ**: 後続 PRD で primitive associated const を使う際、既存 variant をそのまま利用すべき。
dead code に見えても削除しないこと。

### `switch.rs::is_literal_match_pattern` の意味論微変化 (I-378)

I-378 で判定基準を `name.contains("::")` 文字列マッチから `Expr::EnumVariant` 構造マッチに変更。
`case Color.Red:` (最頻出) は完全等価だが `case Math.PI:` / `case f64::NAN:` のような (TS で稀な)
ケースは guarded match に展開される (旧: 直接 pattern)。Hono 後退ゼロ確認済。

**引継ぎ**: 将来 `case` で primitive const / std const を使う TS fixture を追加する場合、
`is_literal_match_pattern` に `Expr::PrimitiveAssocConst { .. } | Expr::StdConst(_) => true` 追加を検討。
ただし `f64` 値の pattern matching は Rust で unstable のため guarded match の方が安全。

### PRD spec defect の発見パターン (PRD writer 向け)

PRD 作成時の defect 発見事例:

- I-378 D-1/D-2: `is_trivially_pure` / `is_copy_literal` の戻り値を全 variant 実測せず spec を書き defect 発生。
  PRD Background の "実測サイト" 列挙が動的生成 (`format!("{ty}::{var}")` 等) を見落とした。
- I-380 D-1: 削除を伴う walker リファクタリングで「旧/新 walker 並走 property test」を Completion Criteria
  に含めたが、削除タスクと物理矛盾。Hono 158 fixture バイト等価性で代替達成。

**引継ぎ**: PRD writer は (1) 既存 helpers の戻り値を全 variant 実測してから spec を書く、
(2) "実測サイト" 列挙は grep + 動的生成経路の tracer で網羅、(3) 削除を伴うリファクタリングで property
test を criteria 化する場合、削除タスクとの矛盾を事前検証 (推奨は「Hono 全 fixture バイト等価性維持」)
を遵守する。

### 新規 integration test (削除禁止)

`tests/enum_value_path_test.rs` / `tests/math_const_test.rs` / `tests/nan_infinity_test.rs` (I-378 追加) は
`Expr::EnumVariant` / `PrimitiveAssocConst` / `StdConst` 構造化の lock-in テスト。**削除・スキップ禁止**。

---

## ベースライン (2026-04-07、I-380 完了時点)

| 指標 | 値 |
|------|------|
| Hono クリーン | 114/158 (72.2%) |
| エラーインスタンス | 54 |
| コンパイル (file) | 113/158 (71.5%) |
| コンパイル (dir) | 157/158 (99.4%) |
| テスト数 | 2212 (lib) / 2415 (全体) |
| コンパイルテストスキップ | 22 / 21 (builtins なし / あり) |

### 長期ビジョン

| マイルストーン | 指標 |
|---------------|------|
| 変換率 80% | クリーン 126/158 (現在 114) |
| コンパイル率 80% | ファイルコンパイル 126/158 (現在 113) |
| コンパイルテストスキップ 0 | 全 fixture がコンパイル通過 (現在 22 件) |

---

## リファレンス

- 完了履歴: `git log` で参照
- 調査レポート: `report/`
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
- 優先度ルール: `.claude/rules/todo-prioritization.md`
