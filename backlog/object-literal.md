# オブジェクトリテラル変換

## 背景・動機

TS のオブジェクトリテラル（`{ x: 1, y: 2 }`）は頻出構文だが、現在の変換ツールでは未対応。型注記がある場合は、対応する Rust struct の初期化式に変換できる。

## ゴール

型注記付きのオブジェクトリテラルを Rust の構造体初期化式に変換できる。

### 変換例

**基本:**
```typescript
const p: Point = { x: 1, y: 2 };
```
→
```rust
let p: Point = Point { x: 1.0, y: 2.0 };
```

**関数引数:**
```typescript
function draw(p: Point) { ... }
draw({ x: 0, y: 0 });
```
→
```rust
fn draw(p: Point) { ... }
draw(Point { x: 0.0, y: 0.0 });
```

**ネストした構造体:**
```typescript
const r: Rect = { origin: { x: 0, y: 0 }, size: { w: 10, h: 20 } };
```
→
```rust
let r: Rect = Rect { origin: Origin { x: 0.0, y: 0.0 }, size: Size { w: 10.0, h: 20.0 } };
```

## スコープ

### 対象

- 型注記ありの変数宣言でのオブジェクトリテラル
- 型注記あり関数パラメータへのオブジェクトリテラル引数
- ネストしたオブジェクトリテラル（内側にも型情報がある場合）

### 対象外

- 型注記なしのオブジェクトリテラル（`const obj = { x: 1 }`）
- shorthand property（`{ x }` = `{ x: x }`）
- computed property（`{ [key]: value }`）
- spread（`{ ...obj, x: 1 }`）
- 分割代入（`const { x, y } = obj`）

## 設計

### 技術的アプローチ

1. **IR 拡張**: `Expr` に `StructInit` バリアントを追加（構造体名とフィールド名・値のペアのリスト）
2. **transformer 追加**: SWC の `ObjectLit` を解析し、型注記から構造体名を取得して `Expr::StructInit` に変換
3. **generator 更新**: `Expr::StructInit` を `StructName { field1: val1, field2: val2 }` 形式で出力

### 影響範囲

- `src/ir.rs` — `Expr` に `StructInit` バリアント追加
- `src/transformer/expressions.rs` — `ObjectLit` のハンドリング追加
- `src/transformer/statements.rs` — 変数宣言の型注記をオブジェクトリテラル変換に渡す仕組み
- `src/generator.rs` — `Expr::StructInit` の生成ロジック追加
- `tests/fixtures/` — オブジェクトリテラル用の fixture 追加

## 作業ステップ

- [ ] ステップ1: IR 拡張 — `Expr::StructInit { name: String, fields: Vec<(String, Expr)> }` を追加
- [ ] ステップ2: transformer — 型注記ありの `ObjectLit` を `Expr::StructInit` に変換（型注記から構造体名を取得）
- [ ] ステップ3: generator — `Expr::StructInit` を `Name { field: value, ... }` として出力
- [ ] ステップ4: 関数引数での型推論 — パラメータの型注記からオブジェクトリテラルの構造体名を解決
- [ ] ステップ5: スナップショットテスト — fixture ファイルで E2E 検証

## テスト計画

- 正常系: 型注記付き変数宣言、関数引数、ネストした構造体
- 正常系: 文字列・数値・ブール値フィールド
- 異常系: 型注記なしのオブジェクトリテラル（エラーまたはスキップ）
- 境界値: 1 フィールドの構造体、フィールドなしの構造体
- スナップショット: `tests/fixtures/object-literal.input.ts` で E2E 検証

## 完了条件

- 上記変換例が正しく変換される
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
- スナップショットテストが追加されている
