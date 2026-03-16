# オブジェクト分割代入の拡張（I-14）

## 背景・動機

現在のオブジェクト分割代入は shorthand（`{ x }`）と rename（`{ x: newX }`）のみ対応している。デフォルト値（`{ x = 0 }`）、ネスト（`{ a: { b } }`）、rest（`{ a, ...rest }`）は未対応で、これらを含む TS コードは変換エラーになる。

これら 3 パターンは実際の TS コードで頻出し、組み合わせて使われる（例: `{ a, b = 0, ...rest }`）。変数宣言と関数パラメータの両方で未対応であり、変換率に直接影響する。

## ゴール

1. 以下の分割代入パターンが変数宣言・関数パラメータの両方で変換できる:
   - デフォルト値: `{ x = 0 }` → `let x = obj.x` に `unwrap_or` 相当のフォールバック
   - ネスト: `{ a: { b } }` → `let b = obj.a.b;`
   - rest: `{ a, ...rest }` → 残フィールドを個別展開（型情報あり）、またはコメント付きフォールバック（型情報なし）
2. 上記パターンの組み合わせ（`{ a, b = 0, c: { d }, ...rest }`）が変換できる
3. 既存の分割代入テスト（shorthand, rename）が全て通る

## スコープ

### 対象

- 変数宣言での 3 パターン（`try_convert_object_destructuring`）
- 関数パラメータでの 3 パターン（`convert_object_destructuring_param`）
- パターンの組み合わせ

### 対象外

- 配列分割代入の拡張（既に rest, hole 対応済み）
- 分割代入のコンピューテッドキー（`{ [expr]: x }`）→ I-46 と同時対応が自然
- 分割代入の型注記（`{ x }: { x: number }` の型注記から推論）→ 既存の仕組みで動作

## 設計

### 技術的アプローチ

#### 1. デフォルト値 `{ x = 0 }`

SWC AST では `ObjectPatProp::Assign` の `value` フィールドにデフォルト値が入る。現在は `value` を無視している。

**変数宣言:**
```typescript
const { x = 0, y = "default" } = obj;
```
→
```rust
let x = obj.x; // TypeRegistry でフィールドが Option<T> なら unwrap_or
let y = obj.y;
```

フィールド型が `Option<T>` の場合:
```rust
let x = obj.x.unwrap_or(0.0);
let y = obj.y.unwrap_or_else(|| "default".to_string());
```

フィールド型が不明（TypeRegistry 未登録）の場合:
```rust
let x = obj.x; // TODO: default value 0 not applied (type unknown)
```

**関数パラメータ:** 同様のロジック。パラメータ型注記から TypeRegistry 経由でフィールド型を取得。

#### 2. ネスト `{ a: { b } }`

SWC AST では `ObjectPatProp::KeyValue` の `value` が `Pat::Object` になる。現在は `extract_pat_ident_name` で `Pat::Ident` のみ許容しているためエラーになる。

**変換戦略:** 再帰的に展開する。

```typescript
const { a: { b, c } } = obj;
```
→
```rust
let b = obj.a.b;
let c = obj.a.c;
```

深さ N のネストも同様に、フィールドアクセスのチェーンで展開する。

#### 3. rest `{ a, ...rest }`

SWC AST では `ObjectPatProp::Rest` で表現される。

**型情報ありの場合:** TypeRegistry から元の型のフィールド一覧を取得し、明示的に列挙されたフィールドを除いた残りを個別展開する。

```typescript
const { a, ...rest } = point; // point: { a: number, b: number, c: number }
```
→
```rust
let a = point.a;
let b = point.b; // rest fields
let c = point.c; // rest fields
```

ただし、これだと `rest` という名前の変数が消える。TS の `rest` はオブジェクトとして使われるため、Rust では新しい struct インスタンスとして構築する:

```rust
let a = point.a;
let rest = PointRest { b: point.b, c: point.c }; // 合成 struct
```

→ 合成 struct の生成は複雑。初版は個別フィールド展開とする:

```rust
let a = point.a;
// rest: { b, c } — expanded as individual fields
let b = point.b;
let c = point.c;
```

**型情報なしの場合:** rest の展開先が不明。コメント付きで記録:

```rust
let a = obj.a;
// TODO: rest pattern `...rest` not expanded (type information unavailable)
```

### 影響範囲

- `src/transformer/statements/mod.rs` — `try_convert_object_destructuring` の拡張
- `src/transformer/functions/mod.rs` — `convert_object_destructuring_param` の拡張
- `src/transformer/mod.rs` — `extract_pat_ident_name` の拡張（ネスト対応）、またはバイパス
- `tests/` — 新規テスト追加

## 作業ステップ

- [ ] ステップ 1: デフォルト値（変数宣言）
  - `ObjectPatProp::Assign` の `value` フィールドを処理
  - フィールド型が `Option<T>` の場合に `unwrap_or` / `unwrap_or_else` を生成
  - フィールド型が不明の場合はデフォルト値なしで展開（コメント付き）
  - テスト: `{ x = 0 }`, `{ x = "default" }`, `{ x = true }`

- [ ] ステップ 2: デフォルト値（関数パラメータ）
  - `convert_object_destructuring_param` で同様のロジック
  - テスト: `function foo({ x = 0 }: Opts)`

- [ ] ステップ 3: ネスト（変数宣言）
  - `ObjectPatProp::KeyValue` の `value` が `Pat::Object` の場合、再帰的に展開
  - ソース式をフィールドアクセスチェーンで構築
  - テスト: `{ a: { b } }`, `{ a: { b: { c } } }`（2 段ネスト）

- [ ] ステップ 4: ネスト（関数パラメータ）
  - `convert_object_destructuring_param` で同様のロジック
  - テスト: `function foo({ a: { b } }: Outer)`

- [ ] ステップ 5: rest パターン（変数宣言）
  - TypeRegistry から型のフィールド一覧を取得
  - 明示フィールドを除いた残りを個別展開
  - 型情報なしの場合はコメント付きフォールバック
  - テスト: `{ a, ...rest }` with TypeRegistry 登録あり/なし

- [ ] ステップ 6: rest パターン（関数パラメータ）
  - テスト: `function foo({ a, ...rest }: Point)`

- [ ] ステップ 7: パターン組み合わせ
  - テスト: `{ a, b = 0, c: { d }, ...rest }` の複合パターン

## テスト計画

- **単体テスト（変数宣言）**: デフォルト値（各型）、ネスト（1段/2段）、rest（型あり/なし）、組み合わせ
- **単体テスト（関数パラメータ）**: 同上
- **回帰テスト**: 既存の shorthand / rename テストが全て通る
- **境界値**: 空の分割代入 `{}`、rest のみ `{ ...rest }`、デフォルト値が複雑な式の場合のエラー
- **統合テスト（スナップショット）**: 分割代入を含む変換のスナップショット

## 完了条件

1. `cargo test` 全テスト通過
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
3. `cargo fmt --all --check` 通過
4. デフォルト値・ネスト・rest の 3 パターンが変数宣言と関数パラメータの両方で変換可能
5. パターンの組み合わせが変換可能
6. 型情報なしの rest パターンがコメント付きフォールバックで処理される（パニックしない）
7. 既存の分割代入テストに退行がない
