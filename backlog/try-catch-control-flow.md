# try/catch の制御フロー問題の修正

## 背景・動機

現在の try/catch 変換は即時実行クロージャ `(|| -> Result<(), String> { ... })()` + `match` パターンを使用。この方式には 2 つの問題がある:

1. **break/continue がコンパイル不可**: クロージャ内では break/continue が使えないため、`try { for (...) { break; } } catch` がコンパイル不可
2. **throw の return 変換が型不一致を起こす**: throw を `return Err(...)` に変換するが、関数の返り値型が `Result` でない分岐で型エラー

関連コード: `src/generator/statements.rs` の try/catch 生成（149-192行目付近）、`src/transformer/statements/mod.rs` の throw 変換（340-379行目付近）。

## ゴール

- try/catch 内で break/continue を含むコードがコンパイル可能な Rust を生成する
- throw が条件分岐内のみに存在する関数でも正しく変換される

## スコープ

### 対象

- try/catch のコード生成パターンを、クロージャから labeled block + `Result` パターンに変更
- throw 検出ロジックの改善（分岐内の throw も検出）

### 対象外

- finally ブロックのセマンティクス完全再現（scopeguard パターンは維持）
- 非同期 try/catch

## 設計

### 技術的アプローチ

**try/catch の変換方式変更:**

現在:
```rust
match (|| -> Result<(), String> { try_body; Ok(()) })() {
    Ok(()) => {},
    Err(e) => { catch_body }
}
```

提案（labeled block パターン）:
```rust
let _try_result: Result<(), String> = 'try_block: {
    try_body  // break/continue はそのまま動作
    Ok(())
};
if let Err(e) = _try_result {
    catch_body
}
```

labeled block は Rust 1.65+ で安定化済み。break/continue はブロック外のループに対して有効。

**throw 検出の改善:**

`has_throw_statement` を再帰的に AST を走査するように変更し、条件分岐内の throw も検出する。

### 影響範囲

- `src/generator/statements.rs` — try/catch のコード生成
- `src/ir.rs` — `Stmt::TryCatch` の IR 構造（変更が必要な場合）
- `src/transformer/functions/mod.rs` — throw 検出ロジック
- テストファイル・スナップショット

## 作業ステップ

- [ ] ステップ1（RED）: try/catch 内に break を含むテスト追加（コンパイル不可を確認）
- [ ] ステップ2（GREEN）: generator の try/catch パターンを labeled block に変更
- [ ] ステップ3（RED）: 条件分岐内のみの throw で Result ラッピングされるテスト追加
- [ ] ステップ4（GREEN）: throw 検出の再帰化
- [ ] ステップ5: 既存スナップショット更新
- [ ] ステップ6: Quality check

## テスト計画

- try/catch 内の break → コンパイル可能な Rust
- try/catch 内の continue → コンパイル可能な Rust
- 条件分岐内のみの throw → Result ラッピング
- ネスト try/catch → 正しい labeled block のネスト
- 回帰: 通常の try/catch/finally が変更なく動作

## 完了条件

- try/catch 内の break/continue がコンパイル可能
- 全ての throw パスが検出され、関数に Result ラッピングが適用される
- 全テスト pass、0 errors / 0 warnings
