# 実装全体のデザインレビュー

**基準コミット**: 27aa39d（未コミットの変更あり: 今回セッションの全変更を含む）

## 概要

15,336 行の Rust ソースコード（テスト含む）を全ファイルレビューした結果、3 段階で整理する:

- **要対応**: あるべき姿から明確に離れている。放置するとバグや保守性の問題を引き起こす
- **改善推奨**: 現状動くが無理をしている。次の関連変更時に改善すべき
- **許容**: 意図的なトレードオフ。現時点では問題なし

---

## 要対応（3 件）

### 1. classes.rs: 5 つのクラス生成関数の重複

**箇所**: `src/transformer/classes.rs` L124-343

`generate_standalone_class`, `generate_parent_class_items`, `generate_child_class`, `generate_abstract_class_items`, `generate_child_of_abstract` の 5 関数が同じ 3 ステップ（struct 生成 → 任意の trait 生成 → impl 生成）を個別に実装している。

**問題**: 新しいクラスパターン（例: abstract + implements）を追加するたびに新関数が必要になり、コピペの温床になっている。各関数間でフィールド統合、trait 生成、impl 生成のロジックが微妙に異なるため、修正漏れが発生しやすい。

**あるべき姿**: 共通のビルダー関数に統一する:
```rust
struct ClassOutput {
    struct_item: Option<Item>,   // abstract class では None
    trait_item: Option<Item>,    // standalone では None
    impl_items: Vec<Item>,       // constructor impl, trait impl 等
}

fn build_class_items(info: &ClassInfo, context: ClassContext) -> Result<ClassOutput>
```

### 2. classes.rs: super() リライトの positional マッピング

**箇所**: `src/transformer/classes.rs` L349-403 (`rewrite_super_constructor`)

`super(arg1, arg2)` の引数を親クラスのフィールドに位置（index）でマッピングしている。親のフィールドが 3 つで super() の引数が 2 つの場合、3 番目のフィールドが未初期化のまま `Self { ... }` に含まれる。

**問題**: 生成された Rust コードがコンパイルエラーになる（フィールド不足）。positional マッピングは TS のコンストラクタ引数とフィールドの対応が偶然一致する場合にのみ正しい。

**あるべき姿**: 引数数とフィールド数の不一致をエラーとして報告するか、`this.field = value` パターンからフィールド名を正確に抽出する（現在も `try_extract_this_assignment` で一部行っているが、super() 経由のフィールド初期化と統合されていない）。

### 3. ir.rs: BinaryOp/UnaryOp の演算子が String 型

**箇所**: `src/ir.rs` L331-345

`BinaryOp { op: String }` と `UnaryOp { op: String }` で演算子を文字列として保持している。generator は検証なしにそのまま出力する（`expressions.rs` L128-129）。

**問題**:
- 型安全性がない。不正な演算子文字列がそのまま Rust コードに出力される
- generator で演算子の優先順位を判定できない（`a + b * c` と `(a + b) * c` の区別ができない）。現在は transformer 側で常に `BinaryOp` のネストを flat に展開しているため顕在化していないが、ネストした場合に誤ったコードを生成する

**あるべき姿**: `enum BinaryOp { Add, Sub, Mul, Eq, ... }` に変更し、generator で優先順位テーブルを持って必要時にカッコを挿入する。

---

## 改善推奨（7 件）

### 4. types/mod.rs: nullable union の Option ラップ未実装

**箇所**: `src/transformer/types/mod.rs` L1071-1073（TODO コメント）

`convert_union_type`（型注記位置）では `T | null` → `Option<T>` が正しく動作するが、`try_convert_general_union`（type alias 位置）では nullable union が `Option` でラップされない。

**影響**: `type MaybeResult = Success | Failure | null` が `Option<enum>` ではなく素の `enum` になる。

### 5. ir.rs: TryCatch のエラー型がハードコード

**箇所**: `src/ir.rs` L266-279, `src/generator/statements.rs` L172

`Stmt::TryCatch` にエラー型のフィールドがなく、generator が `Result<(), String>` をハードコードしている。

**影響**: try body が値を返す場合（`try { return compute(); }`）に `Result<T, String>` の `T` が常に `()` になり、返り値が失われる。

### 6. ir.rs: Cast の target が String 型

**箇所**: `src/ir.rs` L409

`Expr::Cast { target: String }` で型変換先が文字列。`RustType` を使うべき。

**影響**: 現在は `as f64` のような単純なキャストのみで問題ないが、複合型へのキャストで不正な出力になりうる。

### 7. ir.rs: Method.body の二重目的

**箇所**: `src/ir.rs` L100-116

`Method.body: Vec<Stmt>` が trait シグネチャ（空 body）とデフォルト実装（非空 body）の両方を表現している。generator は `body.is_empty()` で判定している。

**あるべき姿**: `body: Option<Vec<Stmt>>` にして、`None` = シグネチャ、`Some(stmts)` = 実装を明確に区別する。

### 8. statements/mod.rs: convert_stmt と convert_stmt_list の責務境界

**箇所**: `src/transformer/statements/mod.rs` L30-67 vs L384-419

`convert_stmt_list` が分割代入の展開や for ループの特殊パターンを前処理として行い、`convert_stmt` に fallback する。`convert_stmt` を直接呼ぶと分割代入が処理されない。

**影響**: 内部 API の使い分けを間違えるとバグになる。`convert_stmt` を公開 API にしているのに、一部の文は `convert_stmt_list` 経由でしか正しく変換できない。

### 9. functions/mod.rs: デフォルト引数内のインライン型リテラルの extra_items 破棄

**箇所**: `src/transformer/functions/mod.rs` L241-242

`convert_default_param` 内で再帰的に `convert_param` を呼ぶ際、inner call の `extra_items`（インライン型リテラルから生成される struct）が `_extra` として破棄されている。

**影響**: `function f(x: { a: string } = {})` のようなパターンで struct 定義が失われる。

### 10. generator での意味的処理（VecSpread 展開、tail expression 判定）

**箇所**: `src/generator/statements.rs` L21-30, 131-132, 201-287, `src/generator/expressions.rs` L170-197

`VecSpread` の展開（let 束縛 → 複数文、return → let + return 等）と tail expression の判定（最後の文かどうかで `return` キーワードの有無を決定）が generator で行われている。

**あるべき姿**: これらは意味的な変換であり、transformer の責務。IR レベルで `Item::Fn { tail_expr: Option<Expr> }` のように表現し、generator は単純にフォーマットするだけにする。

---

## 許容（4 件）

### 11. expressions/mod.rs が 1,142 行

TODO に「モジュール分割検討」として記録済み。メソッドマッピング等を別モジュールに分離する候補だが、現時点では単一責務の範囲内。

### 12. Param.ty が Option

クロージャのパラメータで型推論を許容するため。Rust のクロージャ構文と整合している。

### 13. EnumVariant の value/data/fields の共存

discriminated union（serde_tag + fields）、data enum（data）、値 enum（value）が同じ構造体で表現されている。型安全性は低いが、バリアント種別ごとに struct を分けると IR が複雑化する。現時点のスコープでは許容。

### 14. generate_trait_method_sig の body.is_empty() 判定

Issue #7 の generator 側の帰結。IR の `Method.body` を `Option` にすれば解消されるが、現状でもロジックは正しい。

---

## 優先順位

| # | 項目 | 影響 | 工数 |
|---|------|------|------|
| 1 | classes.rs 5 関数の統一 | 保守性・拡張性 | 中 |
| 2 | super() リライトの修正 | 正確性（コンパイルエラー生成） | 小 |
| 3 | BinaryOp/UnaryOp の enum 化 | 正確性（演算子優先順位） | 中 |
| 4 | nullable union の Option ラップ | 機能の一貫性 | 小 |
| 5 | TryCatch のエラー型 | 機能の正確性 | 小 |
| 6 | Cast の target を RustType に | 型安全性 | 小 |
| 7 | Method.body を Option に | IR の明確性 | 小 |
| 8 | convert_stmt/convert_stmt_list 整理 | 保守性 | 中 |
| 9 | デフォルト引数の extra_items 修正 | 正確性 | 小 |
| 10 | VecSpread/tail expr を transformer へ | アーキテクチャ | 大 |
