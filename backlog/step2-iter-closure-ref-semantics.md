# Step 2: iterator クロージャ参照型整合と Option 二重ラップ防止

## Background

`build_iter_method_call` (`src/transformer/expressions/methods.rs:35-63`) は全 iterator メソッドに対して `.iter().cloned().method(closure)` パターンを一律適用する。しかし Rust の `Iterator::filter` / `Iterator::find` は closure に `&Self::Item` を渡す（`map`/`any`/`all`/`for_each`/`fold` は `Self::Item` by value）。

この差異により 2 つの問題が発生:

1. **I-011**: `.cloned()` 後の `filter`/`find` closure が `&T` を受け取るが、body は `T` 前提で生成される（例: `|x| x > 4.0` で `&f64 > f64` → コンパイルエラー）
2. **I-012**: `find()` は `Option<T>` を返すが、TypeResolver が返り値型を `Option` と認識せず `Some()` で二重ラップする（`Some(Option<f64>)` → `Option<Option<f64>>`）

compile_test skip: `array-builtin-methods` (全解消), `closures` (I-011 解消、I-020 は別 PRD)

## Goal

- `array-builtin-methods` フィクスチャが compile_test を通過する
- `closures` フィクスチャの filter 関連コンパイルエラーが解消する
- `filter`/`find` の closure body が参照セマンティクスを正しく扱い、全型（Copy/non-Copy）でコンパイル可能なコードを生成する
- TypeResolver が `Array.find()` の返り値型を `Option<T>` と正しく解決する

## Scope

### In Scope

- I-011: `filter`/`find` closure body の参照型整合（`Deref` 挿入）
- I-012: `find()` の `Option` 二重ラップ防止（型解決の修正）
- compile_test skip list から `array-builtin-methods` を除去
- 影響範囲の既存テストギャップ修正
- E2E テストに `find` ケース追加

### Out of Scope

- I-010: Generic Clone bound 付与（独立コードパス）
- I-020: クロージャ返却の `Box::new` ラップ（Step 3）
- I-048: 所有権推論の全体設計

## Design

### Technical Approach

#### I-011: closure body の参照セマンティクス変換

**根本原因**: `build_iter_method_call` が `filter`/`find` の closure パラメータ規約（`&Self::Item`）を考慮していない。

**修正**: `IrFolder` trait を利用した `DerefClosureParams` folder を実装し、closure body 内の parameter ident を `Expr::Deref` でラップする。

```
// 変換前: |x| x > 4.0 && x.field > 0
// 変換後: |x| *x > 4.0 && (*x).field > 0
```

- `*x > 4.0` — 比較: `f64 > f64` ✓
- `(*x).field` — field access: auto-deref と等価 ✓
- `(*x).method()` — method call: auto-deref と等価 ✓
- `func(*x)` — Copy: コピー ✓, non-Copy: コンパイルエラー（silent semantic change ではない）

全コンテキストで `Deref` を挿入する方針。パターン漏れのリスクがなく、non-Copy のムーブ問題はコンパイルエラーとして正しく検出される。

**適用対象**: `filter` と `find` のみ。以下のメソッドは `Self::Item` by value なので不要:
- `map`: `FnMut(Self::Item) -> B`
- `any`/`all`: `FnMut(Self::Item) -> bool`
- `for_each`: `FnMut(Self::Item)`
- `fold`: `FnMut(B, Self::Item) -> B`

**実装場所**: `methods.rs` に `deref_closure_params` 関数を新設。`strip_closure_type_annotations` の後に適用。

#### I-012: TypeResolver の `find()` 返り値型修正

**根本原因**: TypeResolver が `Array.find()` の返り値型を `Option<T>` と認識しない。

2 経路で修正:

1. **extract tool 修正** (`tools/extract-types/src/extractor.ts`): `extractSignature` 関数の return type 取得を `sig.getReturnType()`（TypeScript 型チェッカーの resolved type）から AST declaration node の return type node 経由に変更。TypeScript の型チェッカーはジェネリック interface のメソッド signature で `T | undefined` を `T` に解決してしまうバグがあるため、AST node の `.type` プロパティから `checker.getTypeAtLocation()` で取得する。修正後に `ecmascript.json` を再生成し、`Array.find` の return type が `{ "kind": "union", "members": [{ "kind": "named", "name": "T" }, { "kind": "undefined" }] }` になることを検証。`convert_union_type` が `Undefined` を検出して `Option<TypeVar("T")>` を生成 → instantiation 後 `Option<f64>`。同じ問題を持つ `Array.pop()` も同時に修正される。

2. **TypeResolver intrinsic fallback** (`src/pipeline/type_resolver/call_resolution.rs`): `resolve_method_return_type` で `lookup_method_sigs` が `None`（builtins 未ロード）の場合、`Vec<T>` の well-known method return type を fallback として返す。

```rust
// lookup_method_sigs が None の場合の fallback
fn intrinsic_vec_method_return_type(element: &RustType, method: &str) -> Option<RustType> {
    match method {
        "find" => Some(RustType::Option(Box::new(element.clone()))),
        "some" | "every" | "includes" => Some(RustType::Bool),
        _ => None,
    }
}
```

これにより `convert_expr_with_expected` (`expressions/mod.rs:69`) の既存ガード `matches!(expr_type, Some(RustType::Option(_)))` が正しくトリガーし、`Some()` ラップをスキップする。

### Design Integrity Review

- **Higher-level consistency**: `IrFolder` trait は既に `Substitute` 等で使用される IR 変換の標準パターン。新 folder `DerefClosureParams` は同じパターンに従う
- **DRY**: `deref_closure_params` と `strip_closure_type_annotations` は異なる責務（型注釈除去 vs 参照セマンティクス変換）。統合すると凝集度が下がるため分離を維持
- **Orthogonality**: `build_iter_method_call` は chain 構築、`deref_closure_params` は body 変換。責務が明確に分離されている
- **Coupling**: intrinsic fallback は `call_resolution.rs` 内の private 関数。外部モジュールへの依存を追加しない
- **Broken windows**: `methods.rs` のテストが `toString` のみで `filter`/`find`/`map` 等のテストなし → 本 PRD で追加

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/expressions/methods.rs` | `deref_closure_params` 新設、`filter`/`find` に適用 |
| `src/pipeline/type_resolver/call_resolution.rs` | `intrinsic_vec_method_return_type` fallback 追加 |
| `tools/extract-types/src/extractor.ts` | `extractSignature` の return type 取得を AST node 経由に修正 |
| `src/builtin_types/ecmascript.json` | extract tool 再生成により `Array.find`/`Array.pop` 等の return type が修正 |
| `tests/compile_test.rs` | skip list から `array-builtin-methods` 除去 |
| `tests/snapshots/integration_test__array_builtin_methods.snap` | snapshot 更新 |
| `tests/snapshots/integration_test__closures.snap` | snapshot 更新 |
| `tests/e2e/scripts/array_methods.ts` | `find` テストケース追加 |

### Semantic Safety Analysis

本 PRD は type fallback を導入しない。I-012 の修正は型解決の精度を上げる方向（Unknown → Option<T>）であり、type fallback の逆。

- `find` の return type が `T` → `Option<T>` に変更: `Option<T>` は `T` より正確な型情報。`Some()` ラップが除去されるため、生成コードは `Option<Option<T>>` (コンパイルエラー) → `Option<T>` (正しい) に改善
- intrinsic fallback は `lookup_method_sigs` が `None` の場合のみ適用。既存の builtin 解決パスには影響しない
- extract tool 修正による JSON 再生成は `find`/`pop` 以外のメソッドにも影響しうる。ただし変更方向は「`| undefined` の復元」（型精度の向上）であり、`Option` ラップが追加されることでコンパイルエラーが増える可能性はあるが silent semantic change は発生しない。T3 で全テスト pass を検証

**Verdict: Safe。silent semantic change なし。**

## Task List

### T1: `deref_closure_params` 関数の実装とテスト

- **Work**: `src/transformer/expressions/methods.rs` に `deref_closure_params` 関数を新設。`IrFolder` trait を利用し、closure body 内の parameter ident (`Expr::Ident(name)` where `name` matches closure param) を `Expr::Deref(Box::new(Expr::Ident(name)))` に変換する。`ClosureBody::Expr` と `ClosureBody::Block` の両方を処理。`strip_closure_type_annotations` とは別関数として実装
- **Completion criteria**:
  - `deref_closure_params(Expr::Closure { params: [x], body: Expr(BinaryOp(Ident("x"), Gt, NumberLit(4.0))) })` が `Expr::Closure { params: [x], body: Expr(BinaryOp(Deref(Ident("x")), Gt, NumberLit(4.0))) }` を返す
  - nested BinaryOp (`x > 4.0 && x < 10.0`) でも全 parameter ident が Deref される
  - field access (`x.field`) 内の Ident も Deref される (auto-deref で等価)
  - non-closure 式がそのまま返される
  - 単体テスト 4 件以上 pass
- **Depends on**: None

### T2: `filter`/`find` への `deref_closure_params` 適用

- **Work**: `src/transformer/expressions/methods.rs` の `map_method_call` 内、`"filter"` と `"find"` の分岐で `build_iter_method_call` に渡す前に `deref_closure_params` を適用するよう変更。具体的には `args` を `strip_closure_type_annotations` + `deref_closure_params` の両方で変換する。`"map"` | `"filter"` の結合を解除し、`"filter"` を独立分岐にする
- **Completion criteria**:
  - `map_method_call(object, "filter", [Closure(|x| x > 4.0)])` の結果の closure body に `Deref(Ident("x"))` が含まれる
  - `map_method_call(object, "find", [Closure(|x| x > 0.0)])` も同様
  - `map_method_call(object, "map", [Closure(|x| x * 2.0)])` は Deref なし（変更なし）
  - 単体テスト 3 件以上 pass
- **Depends on**: T1

### T3: extract tool の return type 取得修正と JSON 再生成

- **Work**:
  1. `tools/extract-types/src/extractor.ts` の `extractSignature` 関数 (line 274) を修正。`sig.getReturnType()` の代わりに、AST declaration node (`sig.getDeclaration()`) の return type node (`.type` プロパティ) から `checker.getTypeAtLocation(sigDecl.type)` で型を取得する。`sigDecl.type` が存在しない場合は既存の `sig.getReturnType()` にフォールバック。これにより TypeScript 型チェッカーがジェネリック interface メソッドの `T | undefined` を `T` に解決する問題を回避
  2. `tools/extract-types/` で `npm run build && npm run extract` を実行し `ecmascript.json` を再生成
  3. 再生成後の JSON で `Array.find` の return type が `{ "kind": "union", "members": [..., { "kind": "undefined" }] }` になっていることを検証
  4. `Array.pop()` 等の同根問題も同時に修正されていることを確認
- **Completion criteria**:
  - extract tool が `Array.find` の return type を `T | undefined` として抽出する
  - `Array.pop()` の return type も `T | undefined` として抽出される
  - `transpile_with_builtins("function f(a: number[]): number | undefined { return a.find(x => x > 0); }")` の出力に `Some(` が含まれない
  - 既存の builtin テストが pass
  - `cargo test` 全 pass（JSON 変更による regression なし）
- **Depends on**: None

### T4: TypeResolver intrinsic Vec method return type fallback

- **Work**: `src/pipeline/type_resolver/call_resolution.rs` の `resolve_method_return_type` に fallback 追加。`lookup_method_sigs` が `None` を返し、`obj_type` が `RustType::Vec(inner)` の場合、`intrinsic_vec_method_return_type(inner, method_name)` を呼び出す。`intrinsic_vec_method_return_type` は private 関数として同ファイルに追加。対応メソッド: `find` → `Option<T>`, `some`/`every`/`includes` → `Bool`
- **Completion criteria**:
  - `transpile_collecting("function f(a: number[]): number | undefined { return a.find(x => x > 0); }")` の出力に `Some(` が含まれない（builtins なしでも正しく解決）
  - `intrinsic_vec_method_return_type` の単体テスト 4 件 pass（find, some, every, unknown method）
- **Depends on**: None

### T5: snapshot 更新と compile_test unskip

- **Work**:
  1. `cargo test` で snapshot テスト失敗を確認し `cargo insta review` で更新
  2. `tests/compile_test.rs` の `skip_compile` / `skip_compile_with_builtins` 両リストから `"array-builtin-methods"` を除去
  3. `closures` の skip comment から `(I-217)` 参照を除去し `(I-321)` のみに更新
  4. `cargo test -- test_all_fixtures_compile` で `array-builtin-methods` が compile_test 通過を確認
- **Completion criteria**:
  - `array-builtin-methods` が both skip lists から除去されている
  - `cargo test` 全 pass（snapshot 更新済み）
  - `closures` skip comment が正確
- **Depends on**: T1, T2, T3, T4

### T6: E2E テスト拡張

- **Work**: `tests/e2e/scripts/array_methods.ts` に `find` テストケースを追加。`find` の結果を `undefined` チェック付きで出力。`filter` with captured variable のケースも追加
- **Completion criteria**:
  - `cargo test -- test_e2e_array_methods` pass
  - E2E スクリプトに `find` と `filter` (capture) のケースが含まれる
  - TS (tsx) と Rust の stdout が一致
- **Depends on**: T1, T2, T3, T4

### T7: methods.rs 既存テストギャップ修正

- **Work**: `src/transformer/expressions/methods.rs` のテストモジュールに以下を追加:
  - `test_map_method_call_filter_generates_iter_chain`: `filter` が `.iter().cloned().filter().collect()` チェーンを生成
  - `test_map_method_call_find_generates_iter_chain`: `find` が `.iter().cloned().find()` を生成（collect なし）
  - `test_map_method_call_map_generates_iter_chain`: `map` チェーン
  - `test_map_method_call_some_maps_to_any`: `some` → `any` 変換
  - `test_map_method_call_every_maps_to_all`: `every` → `all` 変換
  - `test_map_method_call_unknown_method_passthrough`: 未知メソッドはそのまま
- **Completion criteria**: 6 件の新規テスト pass
- **Depends on**: T2

## Test Plan

| テスト種別 | 対象 | 内容 |
|-----------|------|------|
| Unit | `deref_closure_params` | simple comparison, nested BinaryOp, field access, non-closure passthrough |
| Unit | `map_method_call` filter/find | Deref 挿入確認、chain 構造確認 |
| Unit | `map_method_call` map/some/every | Deref なし確認 |
| Unit | `intrinsic_vec_method_return_type` | find→Option, some→Bool, unknown→None |
| Integration | snapshot | `array-builtin-methods`, `closures` snapshot 更新 |
| Compile | compile_test | `array-builtin-methods` unskip |
| E2E | array_methods | `find`/`filter` の runtime 動作確認 |

## Completion Criteria

- [ ] `cargo test` 全 pass（2393+ lib, 99+ integration, compile, E2E）
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
- [ ] `cargo fmt --all --check` 0 diffs
- [ ] compile_test skip list から `array-builtin-methods` が除去済み
- [ ] `closures` の skip comment が I-011 解消を反映
- [ ] E2E テスト `array_methods` に `find` ケースが含まれ pass
- [ ] snapshot が更新済み
