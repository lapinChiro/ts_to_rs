# E2E テストで発見されたコード生成・変換バグの修正

## 背景・動機

E2E ブラックボックステスト（`tests/e2e/`）の構築過程で、TS → Rust 変換結果がコンパイルエラーになる 6 件のバグが発見された（I-52〜I-57）。これらは「コンパイルは通るが結果が異なる」サイレント不具合ではなく、**変換結果がそもそもコンパイルできない**深刻なバグである。

現在の E2E テストスクリプトはこれらのバグを回避するために構文を簡素化しており、テストのカバレッジが本来の設計より狭くなっている。バグを修正し、PRD `e2e-blackbox-test.md` で設計された本来のテストスクリプトに戻すことで、E2E テストの品質を引き上げる。

### バグ一覧

| ID | 分類 | 概要 | 影響 |
|----|------|------|------|
| I-52 | コード生成 | クロージャの返り値型指定時に `{}` 欠落 | `arr.map(...)`, `arr.filter(...)` 全般 |
| I-53 | コード生成 | `arr.contains()` の引数に `&` 欠落 | `arr.includes(x)` の変換全般 |
| I-54 | 変換ロジック | try/catch で return 型関数の catch 後に暗黙の return がない | try/catch を持つ return 型関数全般 |
| I-55 | 変換ロジック | デフォルト引数の呼び出しで引数が足りない | デフォルト引数付き関数の呼び出し全般 |
| I-56 | コード生成 | generator の型情報不足（文字列結合 / println フォーマット） | 文字列結合、console.log に Vec を渡すケース |
| I-57 | 変換ロジック | for ループの Range 変数と f64 変数の型不一致 | for ループ + 数値演算の組み合わせ |

## ゴール

- I-52〜I-57 の 6 件のバグが全て修正されている
- E2E テストスクリプトが本来の設計（PRD `e2e-blackbox-test.md` のテスト計画）に近い内容に拡充されている
- 拡充後のスクリプトで TS と Rust の stdout が完全一致する
- 全テスト pass、clippy 0 警告、fmt 通過

## スコープ

### 対象

- **I-52**: `generate_closure` で返り値型がある場合に式ボディを `{}` で囲む
- **I-53**: `includes` → `contains` 変換で引数を `&` 付きにする
- **I-54**: `convert_try_stmt` で catch 後の制御フロー不完全を修正
- **I-55**: 関数呼び出しでデフォルト引数（`Option<T>`）パラメータに `None` を補完
- **I-56**: 文字列結合の `&` 不足を修正。println フォーマットの型依存選択
- **I-57**: for ループの Range 変数型を f64 文脈に合わせる
- E2E テストスクリプトの拡充（修正後に対応可能になる構文を追加）

### 対象外

- I-56 の完全な解決（IR に式レベルの型情報を付与する大規模リファクタ）は対象外。ここでは文字列結合と println の 2 ケースに限定した局所修正を行う
- 新しい E2E テストカテゴリの追加（既存 8 カテゴリ内の拡充のみ）

## 設計

### I-52: クロージャの `{}` 欠落

**ファイル**: `src/generator/expressions.rs` `generate_closure`

`ClosureBody::Expr` かつ `return_type.is_some()` の場合、式を `{ expr }` で囲む。

```rust
// Before
ClosureBody::Expr(expr) => format!("|{params_str}|{ret_str} {}", generate_expr(expr))

// After
ClosureBody::Expr(expr) => {
    if return_type.is_some() {
        format!("|{params_str}|{ret_str} {{ {} }}", generate_expr(expr))
    } else {
        format!("|{params_str}|{ret_str} {}", generate_expr(expr))
    }
}
```

### I-53: `contains()` の `&` 欠落

**ファイル**: `src/transformer/expressions/mod.rs` `map_method_call`

`includes` → `contains` 変換で引数を `Expr::Ref` でラップする。

```rust
"includes" => Expr::MethodCall {
    object: Box::new(object),
    method: "contains".to_string(),
    args: args.into_iter().map(|a| Expr::Ref(Box::new(a))).collect(),
},
```

IR に `Expr::Ref` がない場合は追加する。

### I-54: try/catch の return 不完全

**ファイル**: `src/transformer/statements/mod.rs` `convert_try_stmt`

catch ブロック後に `unreachable!()` を追加するか、try/catch 全体を値を返す式に変換する。最小限の修正として `unreachable!()` を追加する方針。

### I-55: デフォルト引数の呼び出し補完

**ファイル**: `src/transformer/expressions/mod.rs` `convert_call_args_with_types`

呼び出し引数の数が `param_types` の数より少ない場合、不足分が `Option<T>` 型なら `None` を補完する。

### I-56: 文字列結合の `&` / println フォーマット

**文字列結合**: `src/generator/expressions.rs` の `BinaryOp::Add` 生成で、RHS が `Ident` または `MethodCall` の場合は `&` を前置する（文字列結合コンテキスト時）。

**println フォーマット**: 当面は `{}` を維持。Vec を console.log に渡すパターンは E2E テストスクリプトで回避し、I-56 として TODO に根本解決を残す。

### I-57: for Range の型不一致

**ファイル**: `src/transformer/statements/mod.rs` `convert_for_stmt`

Range に変換する際、ループ変数の型宣言が `f64` なら Range の境界値も `f64` にキャストする。または、Range の場合はループ変数を暗黙的に整数として扱い、body 内で `as f64` を挿入する。

### 影響範囲

- `src/generator/expressions.rs` — I-52, I-56
- `src/transformer/expressions/mod.rs` — I-53, I-55
- `src/transformer/statements/mod.rs` — I-54, I-57
- `src/ir.rs` — I-53（`Expr::Ref` 追加の場合）
- `tests/e2e/scripts/*.ts` — テストスクリプト拡充

## 作業ステップ

各ステップは TDD（テスト → RED → 修正 → GREEN）で進める。

- [ ] ステップ1: I-52 — クロージャの `{}` 欠落修正
- [ ] ステップ2: I-53 — `contains()` の `&` 欠落修正
- [ ] ステップ3: I-54 — try/catch の return 不完全修正
- [ ] ステップ4: I-55 — デフォルト引数の呼び出し補完
- [ ] ステップ5: I-56 — 文字列結合の `&` 不足修正
- [ ] ステップ6: I-57 — for Range の型不一致修正
- [ ] ステップ7: E2E テストスクリプト拡充（修正済みバグに対応する構文を追加）
- [ ] ステップ8: Quality check

## テスト計画

### 各バグ修正のユニットテスト

| ステップ | テスト | 入力 | 期待出力 |
|---------|--------|------|---------|
| I-52 | `test_generate_closure_expr_body_with_return_type_has_braces` | `\|x: f64\| -> f64 x * 2.0` | `\|x: f64\| -> f64 { x * 2.0 }` |
| I-53 | `test_convert_includes_generates_ref_arg` | `arr.includes(3)` | `arr.contains(&3.0)` |
| I-54 | `test_try_catch_return_type_has_unreachable` | try/catch in return-type fn | catch 後に unreachable or else |
| I-55 | `test_call_with_missing_default_arg_appends_none` | `greet("World")` (2 params) | `greet("World".to_string(), None)` |
| I-56 | `test_string_concat_rhs_string_has_ref` | `a + b` (String + String) | `a + &b` |
| I-57 | `test_for_range_loop_var_matches_body_type` | `for(let i=0;i<10;i++) { sum += i; }` | 型が一致する Range |

### E2E テストスクリプト拡充

修正後、以下を E2E スクリプトに復元・追加:
- `array_ops.ts`: `arr.map(...)`, `arr.filter(...)`, `arr.includes(...)`
- `functions.ts`: デフォルト引数付き関数の呼び出し、クロージャ
- `error_handling.ts`: return 型付き try/catch 関数
- `loops.ts`: for ループ + 数値演算

## 完了条件

- I-52〜I-57 の 6 件のバグが全て修正され、各修正にユニットテストがある
- E2E テストスクリプトが拡充され、修正前に回避していた構文を含んでいる
- 全 E2E テストで TS と Rust の stdout が完全一致
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
