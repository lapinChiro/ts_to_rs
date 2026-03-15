# try/catch の IR 展開 — Stmt::TryCatch 廃止

## 背景・動機

`Stmt::TryCatch` は `Expr::VecSpread` と同じ構造の割れ窓である。Generator が labeled block + scopeguard + throw→break 書き換えという意味的変換を行っており、「Generator は IR を 1:1 レンダリングするだけ」の原則に違反している。

この割れ窓が原因で、try body 内の `break`/`continue` が正しく動作しない（S-3 Critical）。labeled block `'try_block: { ... }` が break/continue のスコープを奪い、外側のループに到達できない。

## ゴール

- `Stmt::TryCatch` を IR から廃止する
- Transformer が SWC AST の `TryStmt` を検出し、primitive な IR 文（`Stmt::Let`, `Stmt::If`, `Stmt::Break` 等）に直接展開する
- try body 内の `break`/`continue` が外側のループを正しくターゲットする
- Generator から `generate_try_body_stmt` 関数が消え、1:1 レンダリングに徹する
- 生成される Rust コードが `rustc` でコンパイルできる（error_handling テストのスキップ解消）

## スコープ

### 対象

- `Stmt::TryCatch` を IR から廃止
- `convert_try_stmt` を SWC AST レベルの展開関数に置き換え
- try body 内の `throw` → `break 'try_block Err(...)` を Transformer で変換
- try body 内の `break`/`continue` → フラグ変数 + `break 'try_block Ok(())` + ブロック後の条件分岐で制御フロー復元
- finally → `scopeguard::guard` を Transformer で Stmt 列として生成
- Generator の `TryCatch` 分岐と `generate_try_body_stmt` を削除

### 対象外

- try/catch の変換戦略の根本的な見直し（labeled block パターンは維持する）
- nested try/catch（内側の try/catch が外側の制御フローを阻害するケース）の対応
- error_handling テスト以外のスキップ解消

## 設計

### 技術的アプローチ

VecSpread 廃止と同じパターン: SWC AST レベルで検出し、`convert_stmt` 内で直接 primitive な IR 文に展開する。

#### 展開パターン

**try/catch（break/continue なし）:**

```typescript
try { risky(); } catch (e) { handle(e); }
```

→

```rust
let _try_result: Result<(), String> = 'try_block: {
    risky();      // throw は break 'try_block Err(...) に変換済み
    Ok(())
};
if let Err(e) = _try_result {
    handle(e);
}
```

IR 文列:
- `Stmt::Let { name: "_try_result", init: Expr::LabeledBlock { label: "try_block", body, trailing: Ok(()) } }`
- `Stmt::If { condition: Expr::LetPattern("Err(e)", "_try_result"), then_body: catch_body }`

ただし、IR に `Expr::LabeledBlock` や `Expr::LetPattern` がないため、代替案として **文字列ベースの IR ノード** を検討する。
実際には、既存の IR 構文で近似できる:
- labeled block は `Expr::Ident("'try_block: { ... Ok(()) }")` のようなハックではなく、IR に新しいバリアントを追加するのが正しい

→ **IR に `Expr::RawBlock` を追加**: 既に展開済みの Rust コードブロックをそのまま保持する式。Generator は中身をそのまま出力する。

```rust
pub enum Expr {
    // ...
    /// Pre-expanded Rust code block (used for try/catch labeled blocks)
    RawBlock(String),
}
```

ただし `RawBlock` は「IR が Rust 構文と 1:1 対応する」原則に反する。

→ **代替: `Stmt::Block` を追加**:

```rust
pub enum Stmt {
    // ...
    /// A labeled block: `'label: { body... }`
    LabeledBlock {
        label: String,
        body: Vec<Stmt>,
    },
}
```

これならば Generator は `'label: { body }` を 1:1 で出力するだけ。try/catch の展開は全て Transformer で行う。

#### try body 内の break/continue 処理

SWC AST の `TryStmt` の body を走査し:

1. `throw` → `Stmt::Return(Some(Err(...)))` に変換（既存ロジック）→ さらに `Stmt::Break { label: Some("try_block"), value: Some(Err(...)) }` に変換
2. `break` → `Stmt::Expr(Assign { _try_break = true })` + `Stmt::Break { label: Some("try_block"), value: Some(Ok(())) }` に変換
3. `continue` → 同様にフラグ + break

ブロック後:
```rust
if _try_break { break; }
if _try_continue { continue; }
```

#### finally 処理

`scopeguard::guard((), |_| { finally_body })` を `Stmt::Let` として生成。現在 Generator で行っている処理を Transformer に移動。

### 必要な IR 変更

1. **`Stmt::LabeledBlock`** の追加 — labeled block `'label: { ... }` を表現
2. **`Stmt::TryCatch`** の削除
3. `Stmt::Break` に value フィールドの追加を検討（`break 'label value;`）

### 影響範囲

- `src/ir.rs` — `Stmt::TryCatch` 削除、`Stmt::LabeledBlock` 追加、`Stmt::Break` 拡張
- `src/transformer/statements/mod.rs` — `convert_try_stmt` を展開ロジックに置き換え
- `src/transformer/functions/mod.rs` — `contains_throw` がネスト走査で `TryCatch` を参照していた場合の更新
- `src/generator/statements.rs` — `TryCatch` 分岐と `generate_try_body_stmt` の削除、`LabeledBlock` の追加
- `tests/compile_test.rs` — error_handling スキップ解消
- テストファイル全般

## 作業ステップ

### Part A: IR 変更

- [ ] ステップ1: IR に `Stmt::LabeledBlock { label, body }` を追加
- [ ] ステップ2: `Stmt::Break` に `value: Option<Expr>` フィールドを追加
- [ ] ステップ3（RED）: `LabeledBlock` が `'label: { body }` として生成されるテスト
- [ ] ステップ4（GREEN）: Generator に `LabeledBlock` の出力を実装
- [ ] ステップ5（RED）: `Break { label, value }` が `break 'label value;` として生成されるテスト
- [ ] ステップ6（GREEN）: Generator の `Break` を拡張

### Part B: Transformer 展開（try/catch without break/continue）

- [ ] ステップ7（RED）: `try { risky(); } catch(e) { handle(e); }` が primitive IR 文列に展開されるテスト
- [ ] ステップ8（GREEN）: `convert_stmt` に try/catch 展開ロジック実装
- [ ] ステップ9: finally の展開実装

### Part C: break/continue 対応

- [ ] ステップ10（RED）: `try { break; } catch(e) { ... }` が in-loop で正しく展開されるテスト
- [ ] ステップ11（GREEN）: try body 内の break/continue のフラグ化を実装
- [ ] ステップ12: continue 対応

### Part D: IR 廃止・統合

- [ ] ステップ13: `Stmt::TryCatch` を IR から削除
- [ ] ステップ14: Generator の `generate_try_body_stmt` を削除
- [ ] ステップ15: スナップショット更新
- [ ] ステップ16: error_handling コンパイルテストスキップ解消
- [ ] ステップ17: Quality check

## テスト計画

### Transformer レベル

- `try { risky(); } catch(e) { handle(e); }` → `[Let(_try_result, LabeledBlock), If(is_err)]` に展開
- `try { ... } finally { cleanup(); }` → `[Let(_finally_guard, scopeguard), ...]` に展開
- `try { break; } catch(e) { ... }` in loop → フラグ変数 + 制御フロー復元
- `try { continue; } catch(e) { ... }` in loop → 同上
- try body 内の `throw` → `Break { label: "try_block", value: Err(...) }` に変換

### E2E

- error_handling スナップショットが更新される
- error_handling コンパイルテストスキップが解消される
- 他のスナップショットに回帰がない

## 完了条件

- IR に `Stmt::TryCatch` が存在しない
- IR に `Stmt::LabeledBlock` が存在する
- Generator に `generate_try_body_stmt` が存在しない
- Generator の `TryCatch` 分岐が存在しない
- try body 内の break/continue が外側のループを正しくターゲットする
- error_handling コンパイルテストスキップが解消されている
- 全テスト pass、`cargo clippy` 0 警告、`cargo fmt` 通過
