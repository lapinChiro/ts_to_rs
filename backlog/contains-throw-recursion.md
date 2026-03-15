# contains_throw の全構文再帰化

## 背景・動機

`contains_throw` は関数本体に `throw` が含まれるかを判定し、含まれる場合に戻り値型を `Result` でラッピングする。現在は `if`/`block` 内のみ再帰的にスキャンするが、`for`/`while`/`do-while`/`switch` 内の `throw` は検出されない。

検出漏れが発生すると、`throw` が `return Err(...)` に変換されるが関数の戻り値型が `Result` でないためコンパイル不可になる。

## ゴール

- `for`/`while`/`do-while`/`switch`/`labeled` 文内の `throw` が検出される
- ただし `try` ブロック内の `throw` は除外される（`catch` で捕捉されるため、関数の Result ラッピングは不要）

## スコープ

### 対象

- `contains_throw` 関数に `for`/`while`/`do-while`/`switch`/`labeled` のアームを追加
- `try` ブロック内の `throw` を除外するロジック

### 対象外

- `throw` 以外のエラー伝搬パターン（`Promise.reject` 等）
- ネスト関数内の `throw`（別スコープなので対象外が正しい）

## 設計

### 技術的アプローチ

```rust
fn contains_throw(stmts: &[ast::Stmt]) -> bool {
    stmts.iter().any(|stmt| match stmt {
        ast::Stmt::Throw(_) => true,
        ast::Stmt::If(if_stmt) => { /* 既存ロジック */ },
        ast::Stmt::Block(block) => contains_throw(&block.stmts),
        ast::Stmt::While(w) => stmt_contains_throw(&w.body),
        ast::Stmt::DoWhile(dw) => stmt_contains_throw(&dw.body),
        ast::Stmt::For(f) => stmt_contains_throw(&f.body),
        ast::Stmt::ForOf(fo) => stmt_contains_throw(&fo.body),
        ast::Stmt::ForIn(fi) => stmt_contains_throw(&fi.body),
        ast::Stmt::Labeled(l) => stmt_contains_throw(&l.body),
        ast::Stmt::Switch(s) => s.cases.iter().any(|c| contains_throw(&c.cons)),
        // try ブロック内の throw は除外（catch で捕捉されるため）
        // ただし catch ブロック内の throw は検出対象
        ast::Stmt::Try(t) => {
            let catch_has = t.handler.as_ref()
                .is_some_and(|h| contains_throw(&h.body.stmts));
            catch_has
        }
        _ => false,
    })
}

/// 単一文の throw 判定（Block でラップされていない文用）
fn stmt_contains_throw(stmt: &ast::Stmt) -> bool {
    match stmt {
        ast::Stmt::Block(block) => contains_throw(&block.stmts),
        ast::Stmt::Throw(_) => true,
        other => contains_throw(&[other.clone()]),
    }
}
```

### 影響範囲

- `src/transformer/functions/mod.rs` — `contains_throw` 関数の拡張

## 作業ステップ

- [ ] ステップ1（RED）: `for` ループ内の `throw` が検出されるテスト追加
- [ ] ステップ2（GREEN）: `for`/`while`/`do-while`/`for-of` のアーム追加
- [ ] ステップ3（RED）: `switch` 内の `throw` が検出されるテスト追加
- [ ] ステップ4（GREEN）: `switch`/`labeled` のアーム追加
- [ ] ステップ5（RED）: `try` ブロック内の `throw` が除外されるテスト追加
- [ ] ステップ6（GREEN）: `try` ブロック除外ロジック実装
- [ ] ステップ7: Quality check

## テスト計画

- `for` ループ内の `throw` → 検出される（関数が Result ラッピングされる）
- `while` ループ内の `throw` → 検出される
- `switch` case 内の `throw` → 検出される
- `try` ブロック内の `throw` → 除外される（catch で捕捉されるため）
- `catch` ブロック内の `throw` → 検出される（再 throw）
- ネスト関数内の `throw` → 除外される（別スコープ）
- 回帰: 既存の throw テスト

## 完了条件

- 全構文（for/while/do-while/for-of/for-in/switch/labeled）内の throw が検出される
- try ブロック内の throw が適切に除外される
- 全テスト pass、0 errors / 0 warnings
