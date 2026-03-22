# リファクタリング後の構造レビュー結果

**基準コミット**: `45fdc01` (リファクタリング完了)
**レビュー日**: 2026-03-20
**注記**: backlog/plan.md の管理ファイル更新（type-context-propagation PRD 完了処理）が未コミット

## 1. 初回レビュー指摘事項の解消確認

| # | 指摘事項 | 結果 | 根拠 |
|---|---------|------|------|
| R1 | registry ↔ transformer の循環依存 | **合格** | `registry.rs` は `transformer::types::convert_ts_type` のみインポート。逆方向の実装依存なし |
| R2 | expressions/mod.rs が分割 | **合格** | `src/transformer/expressions/` に 12 サブモジュール。mod.rs は ~180 行のみ |
| R3 | transformer/mod.rs の責務過多 | **合格** | クラス前処理・継承統合は `classes.rs` に分離。mod.rs は調整のみ |
| R4 | パラメータ変換の重複排除 | **合格** | `convert_function_param_pat`、`convert_param`、`convert_param_pat` の 3 ヘルパーが共通ロジックを担当 |
| R5 | イテレータラッピングの重複排除 | **合格** | `build_iter_method_call` が全イテレータパターンを一元化 |
| R6 | I-XX コメントの自己完結化 | **合格** | `src/`・`tests/` 内に `I-XX` 参照 0 件 |
| R7 | Named 型の構造的表現 | **合格** | `RustType::Ref`・`RustType::DynTrait` が存在。Named の name に `&`/`dyn `/`Box<` を含む箇所 0 件 |
| R8 | register_interface の二重管理 | **合格** | `register_interface` メソッド 0 件。`TypeDef::new_interface` に統一 |
| R9 | 型ヒント伝播の改善 | **合格** | `ExprContext` 導入済み。カテゴリ B 改善 5 件実装（代入、switch case、static prop、HashMap 値、optional chaining args） |

**結果: 9/9 合格**

## 2. ハイレイヤー評価

### H1: 依存方向の一貫性 — 合格

parser → registry/transformer → generator の方向に違反する `use` が 0 件。

- generator は `crate::ir` のみインポート（transformer/registry への依存なし）
- registry は `transformer::types::convert_ts_type` のみインポート（ユーティリティ関数の利用で、実装の循環なし）
- parser は外部クレートのみ依存

### H2: 循環依存の排除 — 合格

モジュールレベルの循環 use が 0 件。

registry → transformer::types（ユーティリティ関数）と transformer → registry（データ型）の関係は型レベルの依存であり、実装の循環ではない。

### H3: API 表面積の最小化 — 不合格

`pub fn` が 73 件。うち約 30 件は transformer モジュール内部でのみ使用されており、`pub(crate)` に制限すべき。

**主な問題:**
- transformer モジュールから外部に公開されるべき関数は `transform_module`・`transform_module_collecting` の 2 件のみだが、35 件が `pub fn`
- `convert_stmt`、`convert_expr`、`convert_fn_decl` 等は crate 内部でのみ使用

## 3. 中間レイヤー評価

### M1: ファイルサイズ — 不合格

600 行を超えるソースファイル（テスト除外）が 11 件:

| ファイル | 行数 | 超過率 |
|---------|------|--------|
| `transformer/statements/mod.rs` | 2647 | 4.4x |
| `transformer/types/mod.rs` | 2259 | 3.8x |
| `transformer/classes.rs` | 1787 | 3.0x |
| `registry.rs` | 1373 | 2.3x |
| `generator/mod.rs` | 1227 | 2.0x |
| `generator/expressions.rs` | 1181 | 2.0x |
| `transformer/functions/mod.rs` | 1122 | 1.9x |
| `ir.rs` | 1108 | 1.8x |
| `generator/statements.rs` | 1021 | 1.7x |
| `external_types.rs` | 746 | 1.2x |
| `transformer/expressions/calls.rs` | 659 | 1.1x |

### M2: DRY — 不合格

`parseInt`/`parseFloat` の変換ロジックが 16 行の同一コードブロック（`calls.rs`）。

### M3: 疎結合 — 不合格

`super::` 直接呼び出しが閾値（5 件）を超えるファイル:
- `calls.rs`: 7 件
- `patterns.rs`: 6 件

### M4: 直交性 — 部分的不合格

- (a) 新しい文字列メソッド追加: 5 ファイルに影響（methods.rs が主だが、calls.rs、member_access.rs 等にも波及）
- (b) 新しい型変換追加: 23 ファイルに影響

### M5: 直交性（困難なケース）

現在の設計の限界として文書化:

| 仮想変更 | 影響ファイル数 | 評価 |
|---------|-------------|------|
| (a) ジェネリクス対応 | 23 ファイル | 高結合 — TypeRegistry/RustType が全モジュールに浸透 |
| (b) ExprContext フィールド追加 | 1 ファイル | **良好** — コンストラクタが mod.rs に集約 |
| (c) 新 IR ノード追加 | 23 ファイル | 高結合 — IR 型が全モジュールに浸透 |

(a)(c) は IR と型レジストリがコードベースの基盤であるため構造的に不可避。(b) は ExprContext の設計が良好であることを示す。

## 4. ローレイヤー評価

### L1: 関数サイズ — 不合格

100 行超の関数が 20 件（617 関数中 3.2%）:

| 関数名 | 行数 | ファイル |
|--------|------|---------|
| `map_method_call` | 390 | expressions/methods.rs |
| `convert_call_expr` | 188 | expressions/calls.rs |
| `try_convert_general_union` | 180 | types/mod.rs |
| `convert_object_lit` | 167 | expressions/data_literals.rs |
| `convert_object_destructuring_param` | 154 | functions/mod.rs |

他 15 件が 104〜150 行。`map_method_call` は 30+ の JS メソッドを dispatch する match 文であり、ヘルパー抽出の余地がある。

### L2: コメント自己完結性 — 合格

`src/`・`tests/` 内に `I-XX` 参照 0 件。

### L3: TODO 妥当性 — 合格

ソース内の TODO コメント 6 件、全て自己完結的な説明を含む。

### L4: エラーハンドリング — 合格

`unwrap()`/`expect()` はテストコード内のみ。ライブラリコードでは `Result` 伝播を使用。

### L5: 型安全性 — 合格

`RustType::Named` の name フィールドに `&`/`dyn `/`Box<` を含む箇所 0 件。

### L6: API 一貫性 — 不合格

`convert_*` 関数のシグネチャパターンが不統一:
- 一部は `expected: Option<&RustType>` を受け取り、一部は受け取らない
- `convert_lit` はパラメータ順序が異なる（`expected` が先）
- `convert_update_expr` は `reg`/`type_env` を受け取らない
- 戻り値型が `Result<Expr>`、`Result<Vec<Expr>>`、`Expr`（Result なし）と混在

偏差はそれぞれ正当な理由があるが（update 式は AST 情報のみで完結等）、統一的なパターンが欠如。

## 5. 新規発見事項

リファクタリングで新たに生じた問題は検出されなかった。

不合格項目は全て**リファクタリング前から存在していた構造的課題**であり、リファクタリングが新たに導入したものではない。

## 6. 総合評価

### 合格項目（15 件）

R1〜R9（9 件）、H1、H2、L2、L3、L4、L5

### 不合格項目（8 件）

| 項目 | 深刻度 | 対応方針 |
|------|--------|---------|
| H3: API 表面積 | 低 | `pub fn` → `pub(crate) fn` の機械的置換。機能に影響なし |
| M1: ファイルサイズ | 中 | statements/mod.rs、types/mod.rs の分割が最優先。他は漸進的対応 |
| M2: DRY | 低 | parseInt/parseFloat の共通化。1 箇所のみ |
| M3: 疎結合 | 低 | calls.rs、patterns.rs の `super::` 呼び出しを re-export で解消可能 |
| M4: 直交性 | 情報 | 型システム・IR の浸透は構造的に不可避。改善よりも文書化が適切 |
| L1: 関数サイズ | 中 | `map_method_call`（390 行）の分割が最優先。他は漸進的対応 |
| L6: API 一貫性 | 低 | 正当な偏差が多い。統一化のコスト対効果は低い |

### 結論

初回レビューの全指摘事項（R1〜R9）は解消済み。リファクタリングが新たな構造的問題を生んでいないことを確認。

不合格項目はいずれもリファクタリング前から存在する構造的課題であり、深刻度「中」の 2 件（M1: ファイルサイズ、L1: 関数サイズ）を TODO に記録する。
