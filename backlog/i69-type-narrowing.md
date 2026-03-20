# I-69: 型ガード後の型絞り込み（Type Narrowing）

## 背景・動機

TypeScript では `typeof x === "string"` や `x instanceof Foo` の後の分岐で変数の型が自動的に絞り込まれる。現在の変換では、型ガード式自体はコンパイル時定数（`true`/`false`）または `is_none()`/`is_some()` に変換されるが、**分岐内の TypeEnv が更新されない**ため、絞り込まれた型情報が後続の式変換に反映されない。

これにより:
- 型ガード後の分岐で `.to_string()` 等の型依存変換が欠落する
- 型ガード後のメソッド呼び出しで TypeRegistry のルックアップが失敗する
- 影響は typeof/instanceof を使う全てのファイルに波及する

## ゴール

1. `if (typeof x === "string")` の then 分岐で `x` の型が `String` に絞り込まれる
2. `if (x instanceof Foo)` の then 分岐で `x` の型が `Foo` に絞り込まれる
3. `if (x !== undefined)` / `if (x !== null)` の then 分岐で `Option<T>` が `T` に絞り込まれる
4. else 分岐では元の型（または narrowing の補集合）が維持される
5. 上記が TypeEnv のスコープ機構の拡張として実装され、既存のスコープ管理と一貫している

## スコープ

### 対象

- `typeof` ガード: `typeof x === "type"` パターンの検出と then/else 分岐での型更新
- `instanceof` ガード: `x instanceof Foo` パターンの検出と then/else 分岐での型更新
- null/undefined チェック: `x !== null`, `x !== undefined`, `x != null` の検出と `Option<T>` → `T` 絞り込み
- truthiness チェック: `if (x)` で `x: Option<T>` の場合に then 分岐で `T` に絞り込み
- `switch (typeof x)` の各 case での型絞り込み
- ガード条件の否定（`!==` → then で絞り込み、`===` → else で絞り込み）

### 対象外

- ユーザー定義型ガード（`function isFoo(x: any): x is Foo`）— 独立した設計課題
- `in` 演算子ガード（`"prop" in x`）— 未対応構文
- 複合条件（`&&`, `||`）でのガード合成 — 初回スコープ外として段階的に拡張可能

## 設計

### 技術的アプローチ

#### 1. ガードパターンの抽出

if 文の条件式から narrowing パターンを抽出する関数を追加:

```rust
enum NarrowingGuard {
    Typeof { var_name: String, type_name: String },      // typeof x === "string"
    InstanceOf { var_name: String, class_name: String },  // x instanceof Foo
    NonNullish { var_name: String },                       // x !== null/undefined
    Truthy { var_name: String },                           // if (x)
}

fn extract_narrowing_guard(condition: &ast::Expr) -> Option<NarrowingGuard>
```

#### 2. 絞り込み型の決定

```rust
fn narrowed_type(guard: &NarrowingGuard, original: &RustType, reg: &TypeRegistry) -> RustType
```

| ガード | 元の型 | 絞り込み後 |
|--------|--------|-----------|
| `typeof "string"` | any/unknown | `String` |
| `typeof "number"` | any/unknown | `f64` |
| `typeof "boolean"` | any/unknown | `bool` |
| `instanceof Foo` | any/unknown | `Foo` |
| `instanceof Foo` | union containing Foo | `Foo` |
| `!== null/undefined` | `Option<T>` | `T` |
| truthy | `Option<T>` | `T` |

#### 3. TypeEnv への適用

既存の `push_scope()` / `pop_scope()` を活用:

```rust
// if 文の変換時
if let Some(guard) = extract_narrowing_guard(&if_stmt.test) {
    // then 分岐
    type_env.push_scope();
    if let Some(original_ty) = type_env.get(&guard.var_name()) {
        let narrowed = narrowed_type(&guard, original_ty, reg);
        type_env.insert(guard.var_name(), narrowed);
    }
    let then_body = convert_stmt(&if_stmt.cons, reg, ctx, type_env)?;
    type_env.pop_scope();

    // else 分岐（元の型を維持、または補集合）
    // ...
}
```

#### 4. typeof 式の変換改善

現在 `typeof x === "string"` は TypeEnv から `x` の型を解決してコンパイル時定数を返す。narrowing 導入後は:
- 型が既知の場合: 引き続きコンパイル時定数（`true`/`false`）を生成
- 型が `Any`/不明の場合: ランタイムチェックは生成せず、narrowing のみ適用（型注釈としての効果）

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/transformer/statements/mod.rs` | if/switch 文の変換に narrowing ロジックを追加 |
| `src/transformer/type_env.rs` | 必要に応じて narrowing 用ヘルパーを追加 |
| `src/transformer/expressions/patterns.rs` | `extract_narrowing_guard` の実装（既存の typeof/instanceof パターン検出を再利用） |
| テストファイル | 新規テストケース追加 |

## 作業ステップ

- [ ] ステップ1（RED）: typeof ガード後の then 分岐で変数型が絞り込まれるテストを書く
- [ ] ステップ2（RED）: instanceof ガード後のテストを書く
- [ ] ステップ3（RED）: null/undefined チェック後の Option 展開テストを書く
- [ ] ステップ4（GREEN）: `extract_narrowing_guard` を実装
- [ ] ステップ5（GREEN）: `narrowed_type` を実装
- [ ] ステップ6（GREEN）: if 文の変換に narrowing 適用を追加
- [ ] ステップ7（GREEN）: switch (typeof x) の各 case での narrowing を追加
- [ ] ステップ8（REFACTOR）: 既存の typeof/instanceof 変換ロジックとの整理
- [ ] ステップ9: E2E スナップショットテスト

## テスト計画

### 単体テスト

- `if (typeof x === "string") { x.trim() }` → then 分岐で `x: String`
- `if (typeof x === "number") { x + 1 }` → then 分岐で `x: f64`
- `if (x instanceof Foo) { x.bar() }` → then 分岐で `x: Foo`
- `if (x !== null) { x.method() }` → then 分岐で `Option<T>` → `T`
- `if (x) { x.method() }` → truthy で `Option<T>` → `T`
- else 分岐では元の型が維持されることの確認
- 型が既知の場合にコンパイル時定数（`true`/`false`）の生成が維持される
- ネストした if での累積的な narrowing

### E2E テスト

- typeof/instanceof ガードを含む TS ファイルの変換スナップショット

## 完了条件

- 全テストパターンが GREEN
- typeof/instanceof ガード後の分岐で正確な型情報が TypeEnv に設定される
- 既存の typeof/instanceof のコンパイル時定数生成が壊れていない
- `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- `cargo fmt --all --check` が通る
- `cargo test` が全パス
- `cargo llvm-cov` のカバレッジ閾値を満たす
