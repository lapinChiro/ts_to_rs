# Complement Narrowing: Transformer 活用設計

**Base commit**: `30e7af2`（uncommitted changes あり — Batch 5b 実装済み）  
**Status**: ✅ 解��済み（Batch 5b 内で実装）。旧 ID I-346/347/348 → 正式 ID I-349/350/351

---

## 問題

TypeResolver が complement NarrowingEvent を正しく記録するが、Transformer が生成する Rust コードでは変数が元の union/Option 型のまま使われ、コンパイルエラーになる。

### 3 つのパターン

#### パターン 1: early return + union (I-347)
```typescript
if (typeof x === "string") { return 0; }
return x;  // x は F64 のはず
```
現在の出力:
```rust
if let StringOrF64::String(x) = x { return 0.0; }
x  // ← StringOrF64 のまま。コンパイルエラー
```

#### パターン 2: early return + Option (I-348)
```typescript
if (x === null) { return "null"; }
return x.trim();  // x は String のはず
```
現在の出力:
```rust
if let Some(x) = x { } else { return "null".to_string(); }
x.trim().to_string()  // ← Option<String> のまま。コンパイルエラー
```

#### パターン 3: if-let else + union (I-346)
```typescript
if (typeof x === "string") { return x.trim(); }
else { return x.toFixed(2); }
```
現在の出力:
```rust
if let StringOrF64::String(x) = x { return x.trim().to_string(); }
else { return x.toFixed(2.0); }  // ← StringOrF64 のまま
```

---

## 設計

### 統一解決策: `if let` → `match` 変換

complement NarrowingEvent がある場合、`Stmt::IfLet` / `Expr::IfLet` の代わりに `Stmt::Match` / `Expr::Match` を生成する。match arm で変数をバインドすることで、各 arm 内で正しい型の変数が使える。

#### early return + union → `let x = match x { ... }`
```rust
let x = match x {
    StringOrF64::String(x) => { return 0.0; }
    StringOrF64::F64(x) => x,
};
return x;  // x は f64
```

#### early return + Option → `let x = match x { ... }`
```rust
let x = match x {
    None => { return "null".to_string(); }
    Some(x) => x,
};
x.trim().to_string()  // x は String
```

#### else + union → `match x { ... }`
```rust
match x {
    StringOrF64::String(x) => { return x.trim().to_string(); }
    StringOrF64::F64(x) => { return format!("{:.2}", x); }
}
```

### 判定ロジックの配置

変換判定は `generate_if_let` / `build_nested_if_let` の呼び出し元（`convert_if_stmt`）で行う。

判定基準:
1. ガードが単一（compound && でない）
2. ガード変数の complement NarrowingEvent が存在する
   - else ブロック: alt_span 内に complement event がある
   - early return: if 後のスコープに complement event がある

条件を満たす場合、`Stmt::IfLet` の代わりに `Stmt::Match` を生成。

### 実装の責務分離

| レイヤー | 責務 |
|----------|------|
| TypeResolver (narrowing.rs) | complement NarrowingEvent の記録 ← **済み** |
| Transformer (control_flow.rs) | if-let → match 判定 + match IR 生成 |
| Transformer (patterns.rs) | complement パターン文字列の解決 |
| Generator | match IR → Rust コード ← **既存** |

新規コードは Transformer のみ。TypeResolver と Generator は変更不要。

### complement パターンの解決

`resolve_if_let_pattern` は positive パターンを返す。complement パターンは以下のように解決:

- **Option**: positive = `Some(x)` → complement = `None`（式: `Some(x) => x`）
- **2-variant union**: positive = `StringOrF64::String(x)` → complement = `StringOrF64::F64(x)`
- **3+ variant union**: positive = `String(x)` → complement = `_ => { ... }` (wildcard)

complement パターン解決は `resolve_if_let_pattern` と対になる関数として `patterns.rs` に追加。

### early return の match 構造

early return パターンでは `let var = match var { ... };` を生成し、後続の文で narrowed 変数を使う:

```rust
// TS: if (guard) { return/throw; } rest...
// →
let var = match var {
    PositivePattern(var) => { return/throw body },
    ComplementPattern(var) => var,
};
// rest... (var は complement 型)
```

`convert_if_stmt` が early return を検出した場合、`Stmt::IfLet` ではなく `Stmt::Let { init: Expr::Match { ... } }` を生成。

### else ブロックの match 構造

```rust
// TS: if (guard) { then } else { else }
// →
match var {
    PositivePattern(var) => { then body },
    ComplementPattern(var) => { else body },
}
```

`convert_if_stmt` が else ブロック + complement narrowing を検出した場合、`Stmt::IfLet` ではなく `Stmt::Match` を生成。

---

## 影響範囲

| ファイル | 変更内容 |
|----------|---------|
| `src/transformer/expressions/patterns.rs` | `resolve_complement_pattern` 追加 |
| `src/transformer/statements/control_flow.rs` | `convert_if_stmt` で match 判定 + `generate_narrowing_match` 追加 |
| `src/transformer/expressions/mod.rs` | `convert_cond_expr` の else ケースでも match 対応（将来） |
| `src/pipeline/type_resolver/narrowing.rs` | 変更なし |
| `src/generator/` | 変更なし（既存の Match 生成を利用） |

---

## 対象外

- compound && ガードでの match（複数変数のネスト match は複雑。if-let のまま）
- `convert_cond_expr`（三項演算子）での complement match（将来拡張として `Expr::Match` 対応可能だが、今回は early return + else block のみ）
