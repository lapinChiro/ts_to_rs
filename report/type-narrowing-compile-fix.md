# 型 Narrowing コンパイルチェック修正方針

**基準コミット**: `7756458`（未コミット変更あり: I-203 修正 + I-69 基盤）

## 問題

### 1. `if (x !== null)` が `if x != None` に変換される

```typescript
function f(x: string | null): void {
    if (x !== null) { console.log(x); }
}
```

現在の出力:
```rust
fn f(x: Option<String>) {
    if x != None {
        println!("{}", x);  // Option<String> は Display 未実装 → コンパイルエラー
    }
}
```

TypeEnv 上で `x` を `String` に narrowing しても、Rust の変数 `x` は `Option<String>` のまま。TypeEnv は変換器内部の概念であり、生成コードの変数型を変えない。

正しい出力:
```rust
fn f(x: Option<String>) {
    if let Some(x) = x {
        println!("{}", x);  // x: String → Display 実装あり → コンパイル成功
    }
}
```

`if let Some(x) = x` は Rust のイディオムであり、変数 `x` を `String` に再束縛する。これにより TypeEnv の narrowing と生成コードの変数型が一致する。

### 2. `typeof x === "string"` が `true` に変換される（x: any）

```typescript
function f(x: any): void {
    if (typeof x === "string") { console.log(x); }
}
```

現在の出力:
```rust
fn f(x: serde_json::Value) {
    if true {  // ランタイムチェックが消失
        println!("{}", x);
    }
}
```

`true` はランタイムチェックの消失であり、サイレントな意味変更。`serde_json::Value` には `is_string()`, `is_number()` 等のメソッドがあり、正しい変換先がある。

正しい出力:
```rust
fn f(x: serde_json::Value) {
    if x.is_string() {
        println!("{}", x);
    }
}
```

## 解法

### A. null check → `if let Some` パターン生成

`try_convert_undefined_comparison` が `x !== null` → `x.is_some()` を返す既存ロジックを拡張し、`convert_if_stmt` で null check を検出したとき `Stmt::If` ではなく `Stmt::IfLet` を生成する。

既存の `convert_if_with_conditional_assignment` は `if (x = expr)` → `if let Some(x) = expr` を生成しており、同じ IR パターンを使える。

IR に `Stmt::IfLet` が存在するか確認が必要。

### B. typeof x (any) → `serde_json::Value` のメソッド呼び出し

`resolve_typeof_match` で `Any` 型に対して `Placeholder` を返す代わりに、新しい `TypeofMatch::RuntimeCheck` を導入し、`serde_json::Value` のメソッド呼び出しを生成する:

| typeof 文字列 | 生成するメソッド |
|--------------|----------------|
| `"string"` | `x.is_string()` |
| `"number"` | `x.is_number()` |
| `"boolean"` | `x.is_boolean()` |
| `"object"` | `x.is_object()` |
| `"undefined"` | `x.is_null()` |

## 依存関係

- A（null check → if let Some）は独立して実装可能
- B（typeof → serde_json メソッド）は A とは独立
- A は I-69 narrowing の効果を直接可視化する（if let Some で変数が再束縛される）
- B は I-203 の「楽観的 true」を正しい実装に置き換える
