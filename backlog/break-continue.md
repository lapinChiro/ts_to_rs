# break / continue 変換

## 背景・動機

TS の `break` / `continue` はループ制御の基本構文だが、現在の変換ツールでは未対応。Rust にも同名の構文があり、ラベル付き break/continue は Rust のライフタイムラベル（`'label`）に対応させる。

## ゴール

TS の `break` / `continue`（ラベル付き含む）を Rust の対応する構文に変換できる。

### 変換例

**基本:**
```typescript
while (true) {
    if (x > 10) {
        break;
    }
    continue;
}
```
→
```rust
while true {
    if x > 10.0 {
        break;
    }
    continue;
}
```

**ラベル付き:**
```typescript
outer: for (const item of items) {
    for (const sub of item.children) {
        if (sub.done) {
            break outer;
        }
    }
}
```
→
```rust
'outer: for item in items {
    for sub in item.children {
        if sub.done {
            break 'outer;
        }
    }
}
```

## スコープ

### 対象

- `break`（ラベルなし）
- `continue`（ラベルなし）
- `break label` → `break 'label`
- `continue label` → `continue 'label`
- ラベル付きループ（`label: for` / `label: while`）→ `'label: for` / `'label: while`

### 対象外

- 特になし

## 設計

### 技術的アプローチ

1. **IR 拡張**: `Stmt` に `Break` と `Continue` バリアントを追加（オプショナルなラベルを持つ）。ループ文（`While`、`ForOf`、`ForRange`）にオプショナルなラベルを追加
2. **transformer 追加**: SWC の `BreakStmt` / `ContinueStmt` を解析し、ラベルがあれば `'label` 形式に変換。`LabeledStmt` を解析し、内側のループにラベルを付与
3. **generator 更新**: `Stmt::Break` / `Stmt::Continue` のラベル付き出力、ループのラベル付き出力

### 影響範囲

- `src/ir.rs` — `Stmt::Break`、`Stmt::Continue` 追加。ループ文にラベルフィールド追加
- `src/transformer/statements.rs` — `BreakStmt`、`ContinueStmt`、`LabeledStmt` のハンドリング追加
- `src/generator.rs` — `Stmt::Break`、`Stmt::Continue` の生成、ループのラベル生成
- `tests/fixtures/` — break/continue 用の fixture 追加

## 作業ステップ

- [ ] ステップ1: IR 拡張 — `Stmt::Break { label: Option<String> }`、`Stmt::Continue { label: Option<String> }` を追加。ループ文にラベルフィールド追加
- [ ] ステップ2: transformer — `BreakStmt` / `ContinueStmt` を変換。`LabeledStmt` でラベルをループに付与
- [ ] ステップ3: generator — `break ['label]`、`continue ['label]`、`'label: for/while` の出力
- [ ] ステップ4: スナップショットテスト — fixture ファイルで E2E 検証

## テスト計画

- 正常系: ラベルなし break、ラベルなし continue、ラベル付き break、ラベル付き continue
- 正常系: while ループ内、for...of ループ内、for range ループ内での break/continue
- 正常系: ネストしたループでのラベル付き break/continue
- 境界値: ラベル名が Rust の予約語と衝突するケース（必要に応じてプレフィックス付与）
- スナップショット: `tests/fixtures/break-continue.input.ts` で E2E 検証

## 完了条件

- 上記変換例が正しく変換される
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
- スナップショットテストが追加されている
