# Batch 5b: narrowing 残課題 調査レポート

**Base commit**: `0ec121c`  
**調査日**: 2026-04-04  
**対象イシュー**: I-215, I-213, I-214, I-256

---

## 1. 現状の narrowing アーキテクチャ

### パイプライン

```
any_enum_analyzer → TypeResolver → FileTypeResolution → Transformer
```

1. **any_enum_analyzer** (`src/pipeline/any_enum_analyzer.rs`): `any`/`unknown` 型パラメータに対して typeof/instanceof を走査し、合成 enum を生成
2. **TypeResolver** (`src/pipeline/type_resolver/narrowing.rs:25-111`): if 条件からガードを検出し、NarrowingEvent を記録
3. **FileTypeResolution** (`src/pipeline/type_resolution.rs:177-184`): position ベースで narrowed_type を返す
4. **Transformer**: 
   - `get_expr_type` (`expressions/type_resolution.rs:38-57`): 変数アクセス時に narrowed_type を優先参照
   - `resolve_if_let_pattern` (`expressions/patterns.rs:398-434`): ガードから if-let パターンを生成
   - `convert_if_stmt` (`statements/control_flow.rs:20-106`): compound && ガードを分解してネスト if-let 生成

### 現在サポートされるガード

| ガード | positive scope | alternate scope | complement |
|--------|---------------|-----------------|------------|
| `typeof x === "string"` | consequent: String | — | **未対応 (I-213)** |
| `typeof x !== "string"` | alternate: String | — | **未対応 (I-213)** |
| `typeof x === "object"` | **未対応 (I-215)** | — | 未対応 |
| `typeof x === "function"` | **未対応 (I-215)** | — | 未対応 |
| `x !== null` | consequent: T (unwrap Option) | — | **未対応 (I-213)** |
| `x === null` | alternate: T (unwrap Option) | — | **未対応 (I-213)** |
| `x instanceof Foo` | consequent: Named(Foo) | — | **未対応 (I-213)** |
| `if (x)` (truthy) | consequent: T (unwrap Option) | — | **未対応 (I-213)** |
| `&&` compound | consequent のみ | — | 未対応 |

### 設計上の重要な性質

- NarrowingEvent は常に **positive type** （変数が確実にそのスコープでその型である）を記録
- scope は SWC の byte offset (`scope_start..scope_end`) で管理
- if-let パターンが変数をシャドウするため、consequent 内の型解決は if-let バインディングで自然に narrow される
- 問題は **alternate** (else) スコープ と **early return 後** の narrowing が欠如していること

---

## 2. 各イシューの詳細分析

### I-215: typeof "object"/"function" の narrowed type 未設定

**根本原因**: `extract_typeof_narrowing` (`narrowing.rs:120-125`) が "string"/"number"/"boolean" のみ対応

**影響**: TypeResolver が "object"/"function" の NarrowingEvent を生成しない。Transformer 側の if-let パターン生成 (`resolve_typeof_to_enum_variant`) は "object"→"Object", "function"→"Function" をサポート済みなので、if-let 自体は生成される。しかし、NarrowingEvent がないため TypeResolver のスコープ内型解決（メソッド呼び出しの型推論など）が不正確。

**修正方針**: `extract_typeof_narrowing` で変数の現在の型を `lookup_var` で参照し、union enum の場合はバリアントのデータ型を返す。any-narrowing enum の場合は `RustType::Any` を返す。

**実装ポイント**:
- TypeResolver は `self.registry` を持つため、enum バリアントのデータ型をルックアップ可能
- `variant_name_for_type` (`synthetic_registry.rs:338`) の逆引きが必要: typeof string → variant name → variant data type
- typeof "object" → variant "Object"、typeof "function" → variant "Function" (or "Fn")

### I-213: complement narrowing 未実装

**4 つのサブパターン**:

#### (1) `!==` ガードの排除 narrowing（2 バリアント union）

```typescript
function f(x: string | number) {
    if (typeof x !== "string") {
        x.toFixed(2);  // x は number のはず → 現在は StringOrF64 のまま
    }
}
```

現状: `typeof x !== "string"` → alternate に String を記録。consequent の complement (F64) は未記録。

**修正方針**: 2 バリアント union の場合、positive type の反対のバリアントを complement type として consequent に記録。

#### (2) instanceof の else narrowing

```typescript
if (x instanceof Dog) { /* Dog */ } 
else { /* NOT Dog → Cat or Bird */ }
```

現在は else 側に NarrowingEvent なし。

#### (3) truthy の else narrowing

```typescript
if (x) { /* x is T */ } 
else { /* x is null/None */ }
```

Transformer は if-let `Some(x)` を生成するため、else 側で `x` は元の `Option<T>` のまま。Rust の if-let では else 側で変数は変更されないが、TS の意味論では `x` は null が保証される。

**重要**: Rust の `if let Some(x) = opt { ... } else { ... }` では、else 内で `opt` は `None` が保証される。しかし TypeResolver のスコープ型解決で `x` の型が `Option<T>` のままだと、メソッド呼び出し解決が `Option` のメソッドと誤認する可能性。

#### (4) early return 後のフロー narrowing

```typescript
if (x === null) { return; }
x.trim();  // x は non-null のはず
```

**最も影響が大きいパターン**。TS の実コードで非常に頻出。

**修正方針**: 
1. `block_always_exits(stmt)` ヘルパーを実装（return/throw/continue/break で終了するか判定）
2. `visit_if_stmt` で、then-block が always-exit かつ narrowing guard があれば、enclosing block の残りのスコープに complement narrowing を記録

**スコープ計算**: if 文の `span.hi` から enclosing block の `span.hi` までが complement narrowing の有効範囲

### I-214: 三項演算子の compound (&&) narrowing

**根本原因**: `convert_cond_expr` (`expressions/mod.rs:167`) が `extract_narrowing_guard`（単数）を呼ぶ

**修正方針**: `extract_narrowing_guards`（複数形、`patterns.rs:601`）を使い、if 文と同様のネスト if-let 生成を行う

**実装ポイント**: 
- `convert_if_stmt` の compound guard ロジック（`control_flow.rs:35-77`）を参考に
- 三項演算子は式なので、`Stmt::IfLet` ではなく `Expr::IfLet` のネストが必要

### I-256: any-narrowing の typeof 検出がトップレベルのみ

**根本原因**: `collect_from_stmt` (`any_narrowing.rs:143-171`) が If/Block/Return/Expr のみ対応

**未対応の Statement**:
- `While` — ループ条件や本体内の typeof
- `For`/`ForOf`/`ForIn` — ループ本体内の typeof
- `Switch` — `switch (typeof x)` パターン
- `Try` — try/catch 内の typeof
- `DoWhile` — 本体内の typeof
- `Labeled` — ラベル付きブロック内の typeof
- `Throw` — throw 式内の typeof

**修正方針**: `TypeResolver::visit_stmt` (`visitors.rs:466-581`) のパターンをミラーして全 statement type をカバー

---

## 3. イシュー間の依存関係と設計の共有

```
I-256 (any-narrowing statement coverage)
  ↓ 独立、他の 3 件と結合なし

I-215 (typeof "object"/"function")
  ↓ complement type 計算の基盤を共有
I-213 (complement narrowing)
  ↓ complement type は I-215 で確立した variant lookup を使用

I-214 (ternary compound &&)
  ↓ complement narrowing が利用可能なら else 側の型も正確に
```

**共通基盤**: I-215 と I-213 は「union enum のバリアントから特定の typeof string に対応するデータ型を取得する」ロジックを共有する。

### 必要な共通ヘルパー

```rust
/// Union enum のバリアント名から RustType data type を逆引きする
fn resolve_variant_data_type(
    enum_name: &str, 
    variant_name: &str, 
    registry: &TypeRegistry, 
    synthetic: &SyntheticTypeRegistry
) -> Option<RustType>

/// typeof string → variant name → data type を解決する
fn resolve_typeof_narrowed_type(
    var_type: &RustType, 
    typeof_str: &str, 
    registry: &TypeRegistry, 
    synthetic: &SyntheticTypeRegistry
) -> Option<RustType>

/// 2-variant union で positive type の complement を返す
fn compute_complement_type(
    var_type: &RustType,
    positive_type: &RustType,
    registry: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) -> Option<RustType>

/// Statement が必ず return/throw/break/continue で終了するか判定
fn block_always_exits(stmt: &ast::Stmt) -> bool
```

---

## 4. 実装順序の提案

### Phase A: I-256（独立・小規模）
- `collect_from_stmt` に While/For/ForOf/ForIn/Switch/Try/DoWhile/Labeled/Throw を追加
- テスト: 各 statement type でのテスト追加

### Phase B: I-215（共通基盤構築 + typeof "object"/"function"）
- `SyntheticTypeRegistry` / `TypeRegistry` からバリアントデータ型を取得するヘルパー追加
- `extract_typeof_narrowing` を拡張して "object"/"function" 対応
- テスト: typeof "object"/"function" の NarrowingEvent 生成テスト

### Phase C: I-213（complement narrowing）
- `block_always_exits` ヘルパー実装
- `detect_narrowing_guard` を拡張して complement narrowing event を記録:
  - 2-variant union: !== consequent に complement type 記録
  - instanceof else: complement type 記録  
  - truthy else: None 情報（現行 if-let で十分かもしれない）
  - early return: block_always_exits 検出 → 後続スコープに complement narrowing
- テスト: 各パターンの NarrowingEvent 生成 + Transformer 出力検証

### Phase D: I-214（ternary compound &&）
- `convert_cond_expr` を `extract_narrowing_guards` 使用に変更
- ネスト `Expr::IfLet` 生成
- テスト: compound ternary のスナップショットテスト

---

## 5. 追加調査事項（バッチに先行すべきか）

### complement narrowing の型計算の課題

2-variant union は complement が一意に決まるが、3+ variant の場合は complement が union のまま。例:
- `x: string | number | boolean`, `typeof x === "string"` → complement は `number | boolean` (= `F64OrBool` enum)
- この場合、complement として新しい合成 union を生成する必要がある

**対応方針**: 
- 2-variant union: complement type = 他方のバリアントのデータ型
- 3+ variant union: complement type は生成しない（元の union 型のまま）— if-let の else 側で元の型が残るのは Rust 的に自然
- Option: complement は None — else 側では `x` は `Option<T>` のまま、Rust の if-let で自然

### early return と break/continue の区別

- `return` / `throw`: 現在の関数から脱出
- `break`: ループからの脱出（narrowing は loop 外に漏れない）
- `continue`: 次のイテレーションへ（narrowing は同じブロックの後続に効く）

early return narrowing で `break`/`continue` をどこまでサポートするか要検討。初期実装では `return`/`throw` のみでよい。

---

## 6. 既存テスト fixture の確認結果

### 問題のある既存出力

1. **`narrowing-truthy-instanceof.input.rs:51-57`** (`nullCheck`):
```rust
fn nullCheck(x: Option<String>) -> String {
    if let Some(x) = x {
    } else {
        return "null".to_string();
    }
    x  // ← x は Option<String>、String ではない
}
```
→ `x === null` → return → `x` は non-null のはず（early return narrowing: I-213）

2. **`narrowing-truthy-instanceof.input.rs:59-64`** (`typeofNarrowing`):
```rust
fn typeofNarrowing(x: StringOrF64) -> String {
    if let StringOrF64::String(x) = x {
        return x;
    }
    x.toString()  // ← x は StringOrF64 のまま、F64 のはず
}
```
→ typeof === string → return → `x` は F64 のはず（early return + complement narrowing: I-213）

### 既存テストで問題ない出力

- `type-narrowing.input.rs`: すべての Case は if-let バインディングでシャドウされるため、consequent 内の型解決は正しい
- `any-type-narrowing.input.rs`: any-narrowing enum + if-let で正常動作

---

## 7. 結論

4 件のイシューは全て narrowing 基盤の拡張であり、バッチ実行が効率的。I-256 は完全独立、I-215 は I-213 の基盤を共有、I-214 は小規模な拡張。

**最も影響の大きいイシュー**: I-213（complement narrowing）、特に early return パターン。TS の実コードで非常に頻出するパターンであり、これなしではコンパイル可能な Rust コードの生成率が大幅に制限される。

**バッチ外で先行すべきイシューの有無**: なし。RC-1 クラスタの 4 件は全て Batch 5 の基盤上に構築され、他のクラスタへの依存はない。
