# 型コンテキスト伝播の体系化

## 背景・動機

アーキテクチャレビュー（`report/architecture-review.md`）で以下の問題が特定された:

### 型ヒント伝播の暗黙的な依存

`convert_expr` の `expected: Option<&RustType>` パラメータにより、式変換時に外側のコンテキストから型情報を受け取れる。しかし、型ヒントの伝播は呼び出し側の実装に完全に依存している:

1. **伝播漏れが silent に失敗する**: 新しいコンテキストで `expected` に `None` を渡すと、オブジェクトリテラルの変換が silent に失敗する。コンパイラはこの漏れを検出できない
2. **`convert_expr` に `None` を渡している箇所が `expressions/mod.rs`（33箇所）、`statements/mod.rs`（26箇所）、`classes.rs`（1箇所）の計60箇所ある**: 一部は型情報が利用不可能なケースだが、一部は TypeEnv や TypeRegistry から取得可能な型情報を渡していない
3. **位置情報の欠如**: 「パラメータ位置」「変数宣言位置」「戻り値位置」といった情報が `convert_expr` に伝わらないため、trait 型のラッピング（`&dyn` vs `Box<dyn>`）が呼び出し側に散在している

## ゴール

1. `ExprContext` 構造体を導入し、型ヒントと位置情報を一元的に管理する
2. `convert_expr` の全60箇所の `None` 渡しを精査し、型情報が利用可能なケースで伝播を追加する
3. trait 型ラッピングを `ExprContext` の位置情報に基づいて `convert_expr` 内部で一元的に処理する
4. 新しいコンテキストの追加時に型ヒント伝播の漏れをコンパイル時に検出可能にする

## スコープ

### 対象

- `ExprContext` 構造体の導入（`expected` + `position` を包含）
- `convert_expr` の全呼び出し箇所（`expressions/`, `statements/`, `classes.rs`）の署名変更と `ExprContext` への移行
- 全60箇所の `None` 渡しの精査と、利用可能な型情報の伝播追加
- trait 型ラッピングの `convert_expr` 内部への統合

### 対象外

- 新しい型推論アルゴリズムの導入（戻り値型からの逆方向推論等）
- インポート先関数のパラメータ型解決（TypeRegistry にない型の解決は不可能）

## 設計

### ExprContext 構造体

```rust
/// 式変換のコンテキスト。型ヒントと位置情報を保持する。
#[derive(Debug, Clone)]
pub struct ExprContext<'a> {
    /// 期待される型（外側のコンテキストから伝播）
    pub expected: Option<&'a RustType>,
    /// 式が出現する位置（trait ラッピングの判定に使用）
    pub position: ExprPosition,
}

/// 式が出現する位置。trait 型のラッピング方法を決定する。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExprPosition {
    /// 関数パラメータのデフォルト値、または型注釈の右辺
    Value,
    /// 特に位置が重要でない一般的な式
    General,
}

impl<'a> ExprContext<'a> {
    pub fn none() -> Self {
        Self { expected: None, position: ExprPosition::General }
    }

    pub fn with_expected(expected: &'a RustType) -> Self {
        Self { expected: Some(expected), position: ExprPosition::General }
    }

    pub fn with_expected_and_position(expected: &'a RustType, position: ExprPosition) -> Self {
        Self { expected: Some(expected), position }
    }
}
```

`ExprContext::none()` は現在の `None` と同等だが、明示的な「型情報なし」の表明になる。将来的に `#[must_use]` や lint で `none()` の使用箇所を検出可能。

### convert_expr の署名変更

```rust
// Before:
pub fn convert_expr(expr: &ast::Expr, reg: &TypeRegistry, expected: Option<&RustType>, type_env: &TypeEnv) -> Result<Expr>

// After:
pub fn convert_expr(expr: &ast::Expr, reg: &TypeRegistry, ctx: &ExprContext, type_env: &TypeEnv) -> Result<Expr>
```

### trait ラッピングの統合

現在 `convert_param`、`convert_var_decl`、`convert_fn_decl` で個別に行っている trait ラッピングを、`convert_expr` が返す型を元に ExprPosition に応じてラッピングする方式は採用しない。理由: `convert_expr` は `Expr`（IR 式）を返すが、trait ラッピングは `RustType`（IR 型）に対する操作であり、責務が異なる。

代わりに、trait ラッピングの呼び出し箇所を `ExprContext` に集約する:
- 型注釈を変換する関数（`convert_ts_type` のラッパー）に position 情報を渡し、trait 型を position に応じてラッピングする共通関数を提供する

```rust
/// 型注釈を変換し、位置に応じて trait 型をラッピングする。
pub fn convert_type_for_position(
    ts_type: &TsType,
    position: ExprPosition,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let ty = convert_ts_type(ts_type, &mut Vec::new(), reg)?;
    match position {
        ExprPosition::Value => Ok(wrap_trait_for_value(ty, reg)),
        ExprPosition::General => Ok(ty),
    }
}
```

パラメータ位置の `&dyn Trait` ラッピングは引き続き `convert_param` / `convert_ident_to_param` が担当する（パラメータの `&self` / `&mut self` 判定と同様にパラメータ固有のロジック）。

### 精査対象: `None` を渡している全60箇所

全箇所を以下のカテゴリに分類する:

**カテゴリ A: 型情報が原理的に利用不可能（None のまま、ExprContext::none() に移行）**
- 二項演算のオペランド（結果型は入力型に依存）
- Await の内部式
- 計算プロパティのインデックス
- スプレッドのソース式
- for ループのイテレータ式
- switch の discriminant
- 条件式の条件部分（boolean コンテキスト）
- unary 演算のオペランド

**カテゴリ B: TypeEnv/TypeRegistry から型情報を取得可能（改善対象）**
- `assignments.rs`: 代入式の右辺 — 代入先の変数型を TypeEnv から取得
- `statements/mod.rs`: conditional assignment の右辺 — 条件変数の型から
- `statements/mod.rs`: switch case の値 — discriminant の型から（string enum マッチング精度向上）
- `classes.rs`: クラスフィールドの初期化式 — フィールドの型注釈から
- `data_literals.rs`: HashMap 値の変換 — expected が `HashMap<K, V>` なら V を伝播
- `member_access.rs`: optional chaining のメソッド呼び出し引数 — メソッドパラメータ型から

**カテゴリ C: 確認のみ（既に正しく伝播されている）**
- `convert_call_args_with_types`: パラメータ型を伝播済み
- `convert_object_lit` 内のフィールド値: struct_fields から伝播済み
- nullish coalescing の右辺: inner type を伝播済み
- return 式: 戻り値型を伝播済み

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/expressions/mod.rs` | `ExprContext` 定義。`convert_expr` の署名変更 |
| `src/transformer/expressions/*.rs` | 全サブモジュールの `convert_expr` 呼び出し箇所を `ExprContext` に移行 |
| `src/transformer/statements/mod.rs` | `convert_expr` 呼び出し箇所を `ExprContext` に移行 + カテゴリ B の改善 |
| `src/transformer/classes.rs` | `convert_expr` 呼び出し箇所を `ExprContext` に移行 + カテゴリ B の改善 |
| `src/transformer/functions/mod.rs` | `convert_expr` 呼び出し箇所を `ExprContext` に移行 |
| `src/transformer/mod.rs` | trait ラッピングの `convert_type_for_position` 導入 |
| テストファイル | `convert_expr` 呼び出しの署名更新 |

## 作業ステップ

**フェーズ A: ExprContext 導入（機能変更なし）**

- [ ] 1: `ExprContext` と `ExprPosition` を定義
- [ ] 2: `convert_expr` の署名を変更。全呼び出し箇所を機械的に `ExprContext::none()` / `ExprContext::with_expected(ty)` に移行。全テスト PASS 確認
- [ ] 3: テストの `convert_expr` 呼び出しも `ExprContext` に移行。全テスト PASS 確認

**フェーズ B: 全60箇所の精査と分類**

- [ ] 4: 全60箇所をカテゴリ A/B/C に分類し、各箇所にコメントで分類理由を記載

**フェーズ C: カテゴリ B の改善**

- [ ] 5: 代入式の右辺に TypeEnv の型情報を伝播（テスト RED → GREEN）
- [ ] 6: switch case の値に discriminant 型を伝播（テスト RED → GREEN）
- [ ] 7: クラスフィールド初期化に型注釈を伝播（テスト RED → GREEN）
- [ ] 8: HashMap 値に expected の value type を伝播（テスト RED → GREEN）
- [ ] 9: optional chaining メソッド引数にパラメータ型を伝播（テスト RED → GREEN）

**フェーズ D: trait ラッピングの整理**

- [ ] 10: `convert_type_for_position` を導入。既存の `wrap_trait_for_value` 呼び出し箇所を統合
- [ ] 11: 全テスト + clippy + fmt PASS 確認

## テスト計画

| テスト | 入力 | 期待出力 |
|-------|------|---------|
| 代入式の型ヒント伝播 | `let x: Config; x = { name: "new" }` | `x = Config { name: "new".to_string() }` |
| switch case の型ヒント | `switch(dir) { case "up": ... }` (dir: Direction enum) | enum バリアント比較 |
| クラスフィールド初期化 | `class Foo { config: Config = { name: "default" } }` | `Config { name: "default".to_string() }` |
| HashMap 値の型伝播 | `{ [key]: { nested: true } }` with expected `HashMap<String, Config>` | 値に Config の型ヒントあり |

## 完了条件

- [ ] `ExprContext` が導入され、`convert_expr` が `ExprContext` を受け取る
- [ ] 全60箇所の `None` 渡しがカテゴリ A/B/C に分類・文書化されている
- [ ] カテゴリ B の全改善対象に型ヒント伝播が実装されている
- [ ] trait ラッピングの呼び出しが `convert_type_for_position` に集約されている
- [ ] 全テスト PASS、clippy 0警告、fmt PASS
