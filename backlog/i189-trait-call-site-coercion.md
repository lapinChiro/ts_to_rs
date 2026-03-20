# I-189: trait 型の呼び出し側型強制（Type Coercion）

## 背景・動機

I-187 で trait 型の型注釈変換は完了した。interface（メソッドあり）は位置に応じて `&dyn Trait`（パラメータ）/ `Box<dyn Trait>`（変数・戻り値）に変換される。

しかし、型注釈だけが変換されても **式の側が調整されない** ため、コンパイルエラーが発生する:

```typescript
// TS
function greet(g: Greeter): void { g.hello(); }
const g: Greeter = createGreeter();
greet(g);
```

```rust
// 現在の変換結果（コンパイルエラー）
fn greet(g: &dyn Greeter) { g.hello(); }
let g: Box<dyn Greeter> = create_greeter();  // ← Box::new() が必要
greet(g);                                     // ← &*g が必要
```

これは「型注釈と式が不整合」という構造的な問題であり、trait 型が関与する全ての式位置で一貫して発生する。個別のケースを場当たり的に修正するのではなく、**型強制（type coercion）の統一メカニズム**として解決すべき問題である。

## ゴール

trait 型が期待される式位置で、式に対して適切な型強制が自動適用される:

1. `&dyn Trait` が期待される位置（関数/メソッドの引数）で、値に `&` が付与される
2. `Box<dyn Trait>` が期待される位置（変数初期化・代入・return）で、値が `Box::new()` で包まれる
3. `Box<dyn Trait>` → `&dyn Trait` の変換（所有→参照）で `&*` が付与される
4. 上記が全て `ExprContext` の既存メカニズムの拡張として実装され、個別の式位置ごとのハードコードがない

## スコープ

### 対象

- **引数位置**: 関数呼び出し・メソッド呼び出しの引数で `&dyn Trait` パラメータに渡す際の `&` 自動付与
- **引数位置（Box→Ref）**: `Box<dyn Trait>` 型の変数を `&dyn Trait` パラメータに渡す際の `&*` 自動付与
- **初期化位置**: `Box<dyn Trait>` 型の変数宣言の初期化式に `Box::new()` 自動付与
- **代入位置**: `Box<dyn Trait>` 型の変数への代入式に `Box::new()` 自動付与
- **return 位置**: `Box<dyn Trait>` 戻り値型の関数内の return 式に `Box::new()` 自動付与
- **統一メカニズム**: 上記全てを `convert_expr()` 内の `ExprContext::expected` を用いた型強制として実装

### 対象外

- ネストした呼び出しの戻り値型追跡（I-61 の範囲）
- ジェネリック型パラメータの具体化（I-100 の範囲）
- trait でない interface（フィールドのみ）の処理 — これは struct であり、型強制不要

## 設計

### 技術的アプローチ

`convert_expr()` に `Option<T>` と同様の **型強制パス** を追加する。`Option<T>` の処理（`null` → `None`, リテラル → `Some(literal)`）は既に `ExprContext::expected` を用いて `convert_expr()` 内で行われている。trait 型の強制も同じパターンで実装する。

#### 型強制ルール

`ExprContext::expected` が trait 関連の型を持つとき、`convert_expr()` の結果に後処理を適用する:

| expected | 式の型 | 強制 |
|----------|--------|------|
| `Ref(DynTrait(T))` | 値（非参照） | `Expr::Ref(expr)` — `&expr` |
| `Ref(DynTrait(T))` | `Box<dyn T>` | `Expr::Ref(Expr::Deref(expr))` — `&*expr` |
| `Named("Box", [DynTrait(T)])` | 値（非 Box） | `Expr::FnCall("Box::new", [expr])` |

#### 式の型の判定

強制を適用するには「式が何型を返すか」を知る必要がある。既存の `resolve_expr_type()` を使用する:
- 変数参照 → TypeEnv から取得
- 関数呼び出し → TypeRegistry の戻り値型
- 型が不明 → 強制を適用しない（安全側に倒す）

#### 既存の Box<dyn Fn> 処理との統合

`calls.rs:572-579` の `Box<dyn Fn>` ハンドリングは、本来この統一メカニズムに吸収されるべきである。ただし、`Fn` 型は trait とは異なる IR 表現（`RustType::Fn`）を持つため、今回のスコープでは統合しない。将来的な統合の余地を残す。

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/transformer/expressions/mod.rs` | `convert_expr()` に trait 型強制パスを追加 |
| `src/transformer/expressions/type_resolution.rs` | 必要に応じて `resolve_expr_type()` の拡張 |
| `src/ir.rs` | `Expr::Ref` / `Expr::Deref` バリアントの追加（未存在の場合） |
| `src/generator/expressions.rs` | `Ref` / `Deref` の Rust コード生成 |
| `src/transformer/functions/mod.rs` | return 式の `ExprContext` に戻り値型を設定 |
| テストファイル各種 | 新規テストケースの追加 |

## 作業ステップ

- [ ] ステップ1（RED）: trait 型強制の単体テストを書く — `&dyn Trait` 引数、`Box::new()` 初期化、`&*` 変換の各パターン
- [ ] ステップ2（GREEN）: IR に `Expr::Ref` / `Expr::Deref` を追加し、generator で `&expr` / `*expr` を生成
- [ ] ステップ3（GREEN）: `convert_expr()` に trait 型強制パスを実装 — `ExprContext::expected` が trait 関連型のとき後処理を適用
- [ ] ステップ4（GREEN）: 関数呼び出し引数の型情報伝播を確認・修正 — `convert_call_args_with_types()` が `&dyn Trait` パラメータ型を `ExprContext` として子式に渡すようにする
- [ ] ステップ5（GREEN）: return 文の `ExprContext` に関数の戻り値型を設定
- [ ] ステップ6（REFACTOR）: 型強制ロジックを関数として抽出し、テスト容易性を確保
- [ ] ステップ7: E2E スナップショットテスト — trait を含む TS ファイルの変換結果を検証
- [ ] ステップ8: Hono ベンチマーク実行 — 効果の定量確認（閾値は設けない）

## テスト計画

### 単体テスト

- `&dyn Trait` パラメータへの値渡し → `&expr` に変換される
- `&dyn Trait` パラメータへの `Box<dyn Trait>` 渡し → `&*expr` に変換される
- `Box<dyn Trait>` 変数初期化 → `Box::new(expr)` に変換される
- `Box<dyn Trait>` 変数代入 → `Box::new(expr)` に変換される
- `Box<dyn Trait>` 戻り値型の return → `Box::new(expr)` に変換される
- 非 trait 型では強制が適用されないことの確認
- 既に `&` / `Box::new()` が付いている場合に二重適用されないことの確認

### E2E テスト

- trait 型を含む TS ファイルの変換スナップショット（interface 定義 + 関数定義 + 呼び出し + 変数宣言）

## 完了条件

- 上記全テストパターンが GREEN
- `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- `cargo fmt --all --check` が通る
- `cargo test` が全パス
- 型強制ロジックが `convert_expr()` 内に統一され、個別の式位置にハードコードされていない
- `cargo llvm-cov` のカバレッジ閾値を満たす
