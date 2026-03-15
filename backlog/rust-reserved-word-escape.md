# Rust 予約語のエスケープ

## 背景・動機

TypeScript のメソッド名や変数名が Rust の予約語と衝突する場合、生成コードがコンパイル不可になる。例: `router.match(...)` は `match` が Rust の予約語のため構文エラー。

Hono コアファイルに `match` メソッドが含まれており（1件）、変換時に無条件にコンパイル不可になる。

## ゴール

- Rust の予約語と衝突する識別子が `r#` プレフィックスでエスケープされる
- メソッド名、変数名、フィールド名の全ての位置でエスケープが適用される

## スコープ

### 対象

- generator の識別子出力箇所に予約語チェックを追加
- メソッド呼び出し（`object.method()`）のメソッド名
- 変数名（`let` 宣言）
- フィールド名（struct フィールド、フィールドアクセス）
- 関数名

### 対象外

- 型名（struct 名、enum 名）— Rust の予約語は小文字で、型名は PascalCase のため衝突しない
- `self`/`Self` のエスケープ — これらは既に `this` → `self` 変換で適切に処理済み

## 設計

### 技術的アプローチ

Rust の予約語リストを定数配列として定義し、識別子出力時にチェックする。

```rust
/// Rust の予約語一覧（strict + reserved keywords）。
const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
    "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do",
    "final", "macro", "override", "priv", "typeof", "unsized", "virtual",
    "yield", "try",
];

/// 識別子が Rust の予約語と衝突する場合に `r#` プレフィックスを付ける。
fn escape_ident(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}
```

generator の以下の箇所で `escape_ident` を適用:
- `generate_expr` の `Expr::Ident`
- `generate_expr` の `Expr::MethodCall` のメソッド名
- `generate_expr` の `Expr::FieldAccess` のフィールド名
- `generate_stmt` の `Stmt::Let` の変数名
- `generate` の `Item::Fn` の関数名
- `generate` の `Item::Struct` のフィールド名

### 影響範囲

- `src/generator/expressions.rs` — `escape_ident` 関数追加、各出力箇所での適用
- `src/generator/mod.rs` — `Item::Fn` / `Item::Struct` のフィールド名
- `src/generator/statements.rs` — `Stmt::Let` の変数名

## 作業ステップ

- [ ] ステップ1（RED）: `match` メソッド呼び出しが `r#match` に変換されるテスト追加
- [ ] ステップ2（GREEN）: `escape_ident` 関数を実装し、メソッド名に適用
- [ ] ステップ3（RED）: `type` 変数名が `r#type` に変換されるテスト追加
- [ ] ステップ4（GREEN）: 変数名・フィールド名・関数名に `escape_ident` を適用
- [ ] ステップ5: 回帰テスト・Quality check

## テスト計画

- `obj.match(x)` → `obj.r#match(x)`
- `let type = 1` → `let r#type = 1`（変数名エスケープ）
- `obj.match` → `obj.r#match`（フィールドアクセス）
- `function match() {}` → `fn r#match() {}`（関数名エスケープ）
- 非予約語の識別子 → エスケープなし
- `self` → エスケープしない（既に `this` → `self` 変換で使用中）
- 回帰: 既存テスト全件

## 完了条件

- Rust 予約語リストが定数として定義されている
- メソッド名、変数名、フィールド名、関数名の全てで予約語エスケープが適用される
- 全テスト pass、0 errors / 0 warnings
