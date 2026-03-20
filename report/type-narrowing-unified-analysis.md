# 型 Narrowing 統一設計分析

**基準コミット**: `7756458`（未コミット変更あり）

## これまでの結論の遷移と、なぜ二転三転したか

| 回 | 結論 | 崩れた理由 |
|----|------|-----------|
| 1 | TypeEnv narrowing で解決 | typeof が `false` を生成し、narrowing が効いても分岐が消える |
| 2 | I-203（楽観的 true）で解決 | `true` はランタイムチェックの消失。serde_json::Value なら `is_string()` がある |
| 3 | `serde_json::Value` の `is_string()` で解決 | `Box<dyn Any>` に移行すると捨てる作業になる |
| 4 | I-102（TsValue）を先に解決 | typeof/instanceof の主要ユースケースは `any` ではなく union 型 |
| 5 | union 型の `matches!` で解決 | `matches!` は narrowing（変数再束縛）を含まない。`if let` が必要 |
| 6 | `if let` で統一的に解決 | ← 最新 |

**二転三転の根本原因**: typeof/instanceof の問題を「`any` 型の表現問題」として捉えていた。実際には `any` 型・union 型・`Option` 型の 3 つが同じ問題の異なる側面であり、**個別に解法を探していた**ため、一方の解法が他方を考慮していなかった。

## 本質的な問題の定義

TypeScript では、変数が**複数の型のいずれかである**ことを表現できる:

```typescript
let x: string | number;     // 明示的 union
let y: string | null;        // nullable（union の特殊ケース）
let z: any;                  // 全ての型の union（暗黙）
```

typeof/instanceof/null-check は、**実行時に型を確定し、分岐内でその確定型として変数を使う**操作。

Rust には「複数の型のいずれかである値」を表現する方法が 1 つある: **enum**。

| TS の概念 | 現在の Rust 表現 | 本質 |
|-----------|-----------------|------|
| `string \| number` | `enum StringOrF64` | 2 つの可能性を持つ値 |
| `string \| null` | `Option<String>` | 2 つの可能性を持つ値（enum の特殊ケース） |
| `any` | `serde_json::Value` | **全ての可能性**を持つ値 |

**Option は enum の特殊ケースにすぎない**。3 つとも「複数の可能性を持つ値」であり、narrowing は「可能性を 1 つに絞る」操作。

## 3 つの表現の統一性

### narrowing の操作

| TS | Rust の `if let` |
|----|-----------------|
| `if (x !== null)` | `if let Some(x) = x` |
| `if (typeof x === "string")` | `if let StringOrF64::String(x) = x` |
| `if (x instanceof Foo)` | `if let FooOrBar::Foo(x) = x` |

**全て `if let` + destructuring**。enum の種類が違うだけで、narrowing の機構は同一。

### `any` の問題

`any` は「全ての型の union」。問題は、全ての型の enum を定義できないこと。

しかし、`any` 型の変数が**実際にどう使われるか**を分析すれば、**必要なバリアントだけの enum を生成**できる:

```typescript
function f(x: any): void {
    if (typeof x === "string") { x.trim(); }
    else if (typeof x === "number") { doMath(x); }
}
```

→ 使用分析: `x` は "string" と "number" の typeof チェックを受ける → 必要なバリアント: `String`, `F64`, `Other`

```rust
enum FParamX {
    String(String),
    F64(f64),
    Other(Box<dyn std::any::Any>),
}

fn f(x: FParamX) {
    if let FParamX::String(x) = x { x.trim().to_string(); }
    else if let FParamX::F64(x) = x { do_math(x); }
}
```

**`any` は「使用パターンから推論される union 型」として遅延評価できる。**

## 遅延評価のアーキテクチャ

### 現在のパイプライン

```
TS source → SWC AST → (単一パス変換) → IR → Rust source
```

型は変換時に即座に解決される。`any` は `RustType::Any` → `serde_json::Value` に固定。

### 必要なパイプライン

```
TS source → SWC AST → (Pass 1: 型制約収集) → (Pass 2: 型具体化 + IR 生成) → Rust source
```

Pass 1 で `any` 型の変数に対する typeof/instanceof/メソッド呼び出しを収集し、Pass 2 で最小限の enum を生成。

### Pass 1 で収集する情報

```rust
struct TypeConstraints {
    /// typeof チェックで検出されたプリミティブ型
    typeof_checks: Vec<String>,         // ["string", "number"]
    /// instanceof チェックで検出されたクラス名
    instanceof_checks: Vec<String>,     // ["Foo", "Bar"]
    /// 呼び出されたメソッド（型推論のヒント）
    method_calls: Vec<String>,          // ["trim", "toFixed"]
    /// null/undefined チェックの有無
    has_nullish_check: bool,
}
```

### Pass 2 で生成する型

```rust
fn materialize_any_type(constraints: &TypeConstraints) -> RustType {
    if constraints.is_empty() {
        // typeof/instanceof が使われていない → Box<dyn Any> or serde_json::Value
        RustType::Any
    } else {
        // 使用パターンに基づく enum を生成
        let variants = constraints.to_enum_variants();
        RustType::Named { name: generated_enum_name, type_args: vec![] }
    }
}
```

## 結論: 何を先にやるべきか

### 段階 1: `if let` パターン生成（union 型 + Option 型）

**前提なし。即着手可能。**

- `if (x !== null)` → `if let Some(x) = x`
- `if (typeof x === "string")` で `x: StringOrF64` → `if let StringOrF64::String(x) = x`
- `if (x instanceof Foo)` で `x: FooOrBar` → `if let FooOrBar::Foo(x) = x`

これだけで Hono の typeof/instanceof パターンの大部分が正しく変換される。`any` 型は関係しない。

### 段階 2: `any` 型の遅延評価

段階 1 完了後。

- `any` 型の変数に対する typeof/instanceof 使用パターンを収集
- 使用パターンに基づいて enum を自動生成
- 同じ `if let` パターンで narrowing

### 段階 3: 「楽観的 true」の撤回

段階 2 完了後。

- `any` 型の typeof/instanceof が enum ベースの `if let` に置き換わる
- `true`/`false` のハードコードが不要になる
