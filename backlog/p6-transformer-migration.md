# P6: Transformer の移行

## 背景・動機

P5 で `TypeResolver` が AST を独立走査し、`FileTypeResolution`（`expr_types`, `expected_types`, `narrowing_events`, `var_mutability`）を事前計算するようになった。現在の Transformer は型解決を自身の走査中にインラインで実行しているため、`FileTypeResolution` を参照する方式に移行する必要がある。

現在の Transformer の問題:

1. **`resolve_expr_type` の直接呼び出し**（`src/transformer/expressions/type_resolution.rs`）: 各式の変換時に型解決を呼ぶ。P5 の `TypeResolver` に移行することで、型推論のバグと変換ルールのバグを分離する
2. **`ExprContext::expected` の手動伝搬**（`src/transformer/expressions/mod.rs`）: 期待型を呼び出し元が手動構築。`expected_types[span]` の lookup に置換する
3. **`TypeEnv` のスコープ管理**（`src/transformer/type_env.rs`）: narrowing のスコープ管理を Transformer が担当。`narrowing_events` の参照に置換する
4. **Generator 内のセマンティック判断**: `.as_str()` 付加（`src/generator/expressions.rs`）、enum 分類（`src/generator/types.rs`）、regex import スキャン（`src/generator/mod.rs`）が Generator に残っている。これらは「TS の意味論を Rust の意味論に変換する」判断であり、Transformer の責務

`report/pipeline-component-design.md` セクション 6.9（Transformer）に基づき移行する。

## ゴール

1. Transformer が `FileTypeResolution` を参照する方式に移行し、自身で型解決を行わない
2. `ExprContext::expected` を `expected_types[span]` lookup に置換し、手動伝搬を排除する
3. `TypeEnv` のスコープ管理を `narrowing_events` 参照に置換する
4. Generator のセマンティック判断を Transformer に移動し、Generator を純粋な IR→テキスト変換にする
5. Unknown フォールバック: `TypeResolver` が Unknown を返した場合は現在のヒューリスティクスで推測する

## スコープ

### スコープ内

- `TransformContext` 構造体の導入:
  ```rust
  struct TransformContext<'a> {
      module_graph: &'a ModuleGraph,
      type_registry: &'a TypeRegistry,
      synthetic_registry: &'a SyntheticTypeRegistry,
      type_resolution: &'a FileTypeResolution,
      file_path: &'a Path,
  }
  ```
- import パス解決の置換:
  - `src/transformer/mod.rs` の `convert_relative_path_to_crate_path` 呼び出しを `ModuleGraph.resolve_import()` lookup に置換（P2 で ModuleGraph を実装済み。ここで統合する）
- `resolve_expr_type` 呼び出しの置換:
  - `src/transformer/expressions/type_resolution.rs` の `resolve_expr_type` 呼び出しを `type_resolution.expr_types[span]` lookup に置換
  - Unknown の場合は既存の `resolve_expr_type` をフォールバックとして呼ぶ（段階的移行）
- `ExprContext::expected` の置換:
  - `src/transformer/expressions/mod.rs` 等で `ExprContext` から期待型を取得する箇所を `type_resolution.expected_types[span]` lookup に置換
- `TypeEnv` スコープ管理の置換:
  - `src/transformer/type_env.rs` の narrowing スコープ管理を `type_resolution.narrowing_events` の範囲チェックに置換
  - narrowing イベントの検索: 現在の位置（Span）が `scope_start..scope_end` に含まれるイベントを検索
- Transformer の合成型直接挿入を削除し、`SyntheticTypeRegistry.all_items()` から取得する形に変更（P4 から移動）
- Generator のセマンティック判断の移動:
  - `.as_str()` 付加判断（`src/generator/expressions.rs`）→ Transformer が IR に `.as_str()` メソッド呼び出しを含める
  - enum variant 分類（`src/generator/types.rs`）→ Transformer が IR に正しい型表現を設定
  - regex import スキャン（`src/generator/mod.rs`）→ Transformer が use 文を IR に含める
- 既存の全変換テストが通ること

### P5 から引き継いだ残存箇所

`src/transformer/expressions/type_resolution.rs:60` に `SyntheticTypeRegistry::new()` が 1 箇所残存している。これは `resolve_expr_type` 内で `convert_ts_type` を呼ぶ際に一時的な registry を使用しているもの。

P5 では「TypeResolver で `resolve_expr_type` を置き換えることで自然に解消される」と判断し、対応を見送った。P6 で `resolve_expr_type` の呼び出しを `type_resolution.expr_types[span]` lookup に置換すると、`resolve_expr_type` 自体の呼び出し箇所がなくなり、この残存箇所は P8 の不要コード削除で `resolve_expr_type` ごと削除される時点で解消される。

**P6 での確認事項**: `resolve_expr_type` の全 32 箇所の呼び出しを `FileTypeResolution` lookup に置換した後、`resolve_expr_type` がフォールバック以外で呼ばれていないことを grep で確認すること。

### P4 で発生した前提変更

P4 の実装で以下の予定外の変更が行われた。P6 はこれを前提とする:

1. **`convert_expr` に `synthetic: &mut SyntheticTypeRegistry` が追加済み** — P6 で `TransformContext` を導入する際、`synthetic` は `TransformContext` に含めない（`&mut` と `&` の借用ルール矛盾を避けるため）。`synthetic` は引き続き別引数として渡す。P8 の統一パイプラインで Pass 4（TypeResolver）が全ファイル完了後に SyntheticTypeRegistry が不変になった時点で、`TransformContext` に含める
2. **`convert_stmt`, `transform_module_with_path`, `transform_decl` にも `synthetic` が追加済み** — 同上
3. **`resolve_expr_type` の呼び出しが 31 箇所**（PRD 作成時は 32 箇所）
4. **`ExprContext` の参照が 110 箇所** — 全てを `expected_types[span]` lookup に置換する大規模変更

### TransformContext の設計修正

PRD 作成時の設計:
```rust
struct TransformContext<'a> {
    module_graph: &'a ModuleGraph,
    type_registry: &'a TypeRegistry,
    synthetic_registry: &'a SyntheticTypeRegistry,  // ← 不変参照
    type_resolution: &'a FileTypeResolution,
    file_path: &'a Path,
}
```

修正後の設計（P4 の `&mut SyntheticTypeRegistry` との共存）:
```rust
struct TransformContext<'a> {
    module_graph: &'a ModuleGraph,
    type_registry: &'a TypeRegistry,
    type_resolution: &'a FileTypeResolution,
    file_path: &'a Path,
    // synthetic_registry は含めない（&mut が必要なため、別引数で渡す）
    // P8 で統一パイプライン組み立て時に SyntheticTypeRegistry が不変になった時点で含める
}
```

### スコープ外

- `ExprContext` / `TypeEnv` / `resolve_expr_type` の削除（P8 で不要コード削除）
- Generator の構造変更（P7。ここでは Generator からロジックを「移動」するのみ）
- OutputWriter（P7）
- 統一パイプラインの組み立て（P8）
- TypeResolver の改修（P5 で完了済み）
- `synthetic` パラメータの `TransformContext` への統合（P8。SyntheticTypeRegistry が不変になった時点）

## 設計

`report/pipeline-component-design.md` セクション 6.9（Transformer）に準拠。

### TransformContext

```rust
// src/transformer/context.rs（新規）

/// Transformer が参照する不変コンテキスト。
/// synthetic_registry は &mut が必要なため含めない（P8 で統合）。
pub struct TransformContext<'a> {
    pub module_graph: &'a ModuleGraph,
    pub type_registry: &'a TypeRegistry,
    pub type_resolution: &'a FileTypeResolution,
    pub file_path: &'a Path,
}
```

Transformer の各メソッドに `&TransformContext` を引数として追加する。既存の `&TypeRegistry` / `&TypeEnv` / `ExprContext` と並存させ、段階的に置き換える。`synthetic: &mut SyntheticTypeRegistry` は P4 で既に全関数に追加済みなので、引き続き別引数で渡す。

### expr_types の lookup パターン

```rust
// 置換前（現在）
let ty = resolve_expr_type(expr, type_env, registry);

// 置換後
let ty = match ctx.type_resolution.expr_types.get(&span_of(expr)) {
    Some(ResolvedType::Known(t)) => t.clone(),
    Some(ResolvedType::Unknown) | None => {
        // フォールバック: 既存ヒューリスティクス
        resolve_expr_type_fallback(expr, ...)
    }
};
```

### expected_types の lookup パターン

```rust
// 置換前（現在）
let expected = expr_ctx.expected.as_ref();

// 置換後
let expected = ctx.type_resolution.expected_types.get(&span_of(expr));
```

### narrowing の lookup パターン

```rust
// 置換前（現在）
let narrowed_ty = type_env.get(&var_name);  // TypeEnv::get で変数の型を取得

// 置換後（FileTypeResolution の narrowed_type メソッドを使用）
let narrowed_ty = ctx.type_resolution.narrowed_type(&var_name, current_pos);
// narrowed_type() は内部で rfind() を使い、最も内側のスコープの narrowing を返す
```

### Generator からの移動項目

| 判断 | 現在の場所 | 移動先（Transformer での表現） |
|------|-----------|-------------------------------|
| `.as_str()` 付加 | `src/generator/expressions.rs` | IR の `Expr::MethodCall` として `.as_str()` を生成 |
| enum variant 分類 | `src/generator/types.rs` | IR の型表現に正しい enum path を設定 |
| regex import | `src/generator/mod.rs` | IR の use 文に `use regex::Regex` を追加 |

### 影響ファイル

- **新規**: `src/transformer/context.rs`
- **変更**: `src/transformer/mod.rs`（TransformContext の導入、メソッドシグネチャの変更）
- **変更**: `src/transformer/expressions/mod.rs`（`ExprContext::expected` → `expected_types` lookup）
- **変更**: `src/transformer/expressions/type_resolution.rs`（`resolve_expr_type` 呼び出しを lookup に置換。関数自体は残す）
- **変更**: `src/transformer/expressions/calls.rs`（期待型・型解決の lookup 変更）
- **変更**: `src/transformer/expressions/member_access.rs`（レシーバ型の lookup 変更）
- **変更**: `src/transformer/expressions/methods.rs`（レシーバ型の lookup 変更）
- **変更**: `src/transformer/expressions/binary.rs`（左右の型 lookup 変更）
- **変更**: `src/transformer/statements/mod.rs`（narrowing の lookup 変更）
- **変更**: `src/transformer/type_env.rs`（narrowing をイベント参照に置換。構造体は残す）
- **変更**: `src/generator/expressions.rs`（`.as_str()` 判断の削除）
- **変更**: `src/generator/types.rs`（enum 分類の削除）
- **変更**: `src/generator/mod.rs`（regex import スキャンの削除）

## 作業ステップ

### Step 1: テスト設計（RED）

1. `TransformContext` を使った Transformer のテスト:
   - FileTypeResolution をモックで構築 → Transformer が lookup のみで動作することを検証
   - Unknown フォールバックのテスト: expr_types に Unknown がある場合にヒューリスティクスが動作する
2. Generator のセマンティック判断移動のテスト:
   - `.as_str()` が IR に含まれることを検証（Generator ではなく Transformer が付加）
   - regex を使うコード → IR に `use regex::Regex` が含まれる

### Step 2: TransformContext の導入（GREEN）

1. `src/transformer/context.rs` に `TransformContext` を定義
2. Transformer の主要メソッドに `&TransformContext` を引数追加（既存引数と並存）
3. 既存テストが通ることを確認（この時点ではまだ lookup を使わない）

### Step 3: expr_types の置換（GREEN）

1. `resolve_expr_type` の呼び出し箇所を列挙
2. 各箇所を `type_resolution.expr_types[span]` lookup に置換
3. Unknown の場合は既存の `resolve_expr_type` をフォールバックとして呼ぶ
4. 既存テストが通ることを確認

### Step 4: expected_types の置換（GREEN）

1. `ExprContext::expected` の参照箇所を列挙
2. 各箇所を `type_resolution.expected_types[span]` lookup に置換
3. expected_types に存在しない場合は `None` として扱う（現在の挙動と同等）
4. 既存テストが通ることを確認

### Step 5: narrowing の置換（GREEN）

1. `TypeEnv` の narrowing 参照箇所を列挙
2. 各箇所を `type_resolution.narrowing_events` の範囲チェックに置換
3. 既存テストが通ることを確認

### Step 6: Generator のセマンティック判断の移動（GREEN）

1. `.as_str()` 付加: Generator から判断ロジックを削除し、Transformer が IR に `.as_str()` を含める
2. enum 分類: Generator から判断ロジックを削除し、Transformer が IR に正しい型を設定
3. regex import: Generator からスキャンを削除し、Transformer が use 文を IR に追加
4. 既存テストが通ることを確認

### Step 7: 統合テスト + リファクタリング（REFACTOR）

1. Hono ベンチマークで結果が悪化していないことを確認
2. `cargo clippy`, `cargo fmt --check`
3. ドキュメントコメントの整備
4. 不要になった引数・フィールドの整理（ただし削除は P8 で行う）

## テスト計画

| テスト | 検証内容 | 期待結果 |
|--------|---------|---------|
| `test_transform_with_context` | TransformContext 経由の基本変換 | 既存と同じ IR が生成される |
| `test_expr_type_lookup` | expr_types の lookup | `type_resolution.expr_types` から型を取得 |
| `test_expr_type_unknown_fallback` | Unknown 時のフォールバック | 既存ヒューリスティクスが動作 |
| `test_expected_type_lookup` | expected_types の lookup | `type_resolution.expected_types` から期待型を取得 |
| `test_narrowing_event_lookup` | narrowing の範囲チェック | スコープ内のイベントが正しく検索される |
| `test_as_str_in_ir` | `.as_str()` が IR に含まれる | Transformer が付加、Generator は透過的に出力 |
| `test_regex_import_in_ir` | regex import が IR に含まれる | Transformer が use 文を生成 |
| `test_enum_classification_in_ir` | enum 分類が IR に含まれる | Transformer が正しい型を設定 |
| `test_generator_no_semantic_judgment` | Generator にセマンティック判断がない | Generator は IR をそのまま文字列化 |
| 既存スナップショットテスト全体 | 出力の後方互換性 | 全スナップショットが一致（または意図的な改善） |
| 既存テスト全体 | 後方互換性 | `cargo test` が全 GREEN |
| Hono ベンチマーク | 変換品質 | ベンチマーク結果が悪化していない |

## 完了条件

- [ ] `TransformContext`（`module_graph`, `type_registry`, `type_resolution`, `file_path`。`synthetic_registry` は含めない）が導入され、Transformer の主要メソッドが使用している
- [ ] `resolve_expr_type` の直接呼び出しが `type_resolution.expr_types[span]` lookup に置換されている
- [ ] `ExprContext::expected` が `type_resolution.expected_types[span]` lookup に置換されている
- [ ] `TypeEnv` の narrowing スコープ管理が `type_resolution.narrowing_events` 参照に置換されている
- [ ] Unknown フォールバック: TypeResolver が Unknown を返した場合に既存ヒューリスティクスが動作する
- [ ] Generator の `.as_str()` 付加、enum 分類、regex import スキャンが Transformer に移動済み
- [ ] Generator にセマンティック判断が残っていない（IR → テキストの純粋変換のみ）
- [ ] 上記テスト計画の全テストが GREEN
- [ ] `cargo test` で既存テストが全て GREEN（後方互換）
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] Hono ベンチマークで結果が悪化していない
- [ ] pub な型・関数に `///` ドキュメントコメントがある
