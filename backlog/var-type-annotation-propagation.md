# 変数型注釈付き Arrow 関数の戻り値型伝播

## 背景・動機

Hono 全151ファイルの変換で、28ファイルが `"object literal requires a type annotation to determine struct name"` エラーで失敗しており、最大のボトルネックになっている。

調査の結果（`report/object-literal-type-inference-design.md`）、28件のうち **16件は型情報が AST に存在するのに無視されているバグ** であることが判明した。

```typescript
// 型注釈 `: GetConnInfo` はAST上に存在するが、変換器が無視
export const getConnInfo: GetConnInfo = (c) => ({
    remote: { address: c.req.header('cf-connecting-ip') }
})
```

`convert_var_decl_arrow_fns` (mod.rs:553) が変数名だけを抽出し、`type_ann` フィールドを読み取っていない。同じパターンは `convert_var_decl` (statements/mod.rs:132) で正しく実装されており、実装漏れである。

## ゴール

1. `const f: FnType = (args) => { return { ... } }` 形式で、`FnType` の戻り値型がオブジェクトリテラルに伝播される
2. 型エイリアス（`GetConnInfo` → `(c: Context) => ConnInfo`）が TypeRegistry 経由で解決される
3. Hono の該当16ファイルでオブジェクトリテラルのエラーが解消される

## スコープ

### 対象

- `convert_var_decl_arrow_fns` で変数の型注釈を読み取る処理の追加
- 型注釈が `RustType::Fn { return_type }` の場合、その `return_type` を Arrow body に伝播
- 型注釈が `RustType::Named { name }` の場合、TypeRegistry で `TypeDef::Function { return_type }` を検索して戻り値型を取得
- ユニットテスト・スナップショットテスト・E2E テスト追加

### 対象外

- 未登録コンストラクタ/関数の引数型（I-112b）— 外部型解決（I-24）が前提
- 型注釈なし変数のオブジェクトリテラル（I-112c）— 匿名構造体の設計が必要
- Arrow 関数のパラメータ型の伝播（変数型注釈からパラメータ型を推論）— 別テーマ

## 設計

### 技術的アプローチ

`convert_var_decl_arrow_fns` (mod.rs:553) に以下の処理を追加:

1. `declarator.name` の `type_ann` から型注釈を取得（`convert_var_decl` と同じパターン）
2. 型注釈を `RustType` に変換
3. 戻り値型を抽出:
   - `RustType::Fn { return_type, .. }` → `return_type` を直接使用
   - `RustType::Named { name, .. }` → `reg.get(name)` で `TypeDef::Function { return_type, .. }` を検索
4. Arrow 関数の変換時に、抽出した戻り値型を body の `convert_stmt` 群に `return_type` として渡す

現在の `convert_var_decl_arrow_fns` は Arrow を `convert_arrow_expr` で変換した後に `Expr::Closure` から情報を取り出して `Item::Fn` を組み立てている。戻り値型は Arrow の明示的な戻り値型注釈（`(): number => ...`）からのみ取得しており、変数の型注釈からは取得していない。

修正は「変数型注釈からの戻り値型」を「Arrow 明示型注釈からの戻り値型」のフォールバックとして追加する形になる。

### 型エイリアスの解決チェーン

```
const getConnInfo: GetConnInfo = (c) => ({ ... })
```

1. `type_ann` = `TsTypeRef { type_name: "GetConnInfo" }`
2. `convert_ts_type` → `RustType::Named { name: "GetConnInfo" }`
3. `reg.get("GetConnInfo")` → `TypeDef::Function { return_type: Some(RustType::Named { name: "ConnInfo" }) }`
4. `return_type` = `RustType::Named { name: "ConnInfo" }` → これが Arrow body に伝播

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/mod.rs` | `convert_var_decl_arrow_fns` に型注釈読み取り・戻り値型伝播追加 |
| `src/transformer/mod.rs` (tests) | ユニットテスト追加 |
| `tests/fixtures/` | 新規フィクスチャ追加 |
| `tests/integration_test.rs` | スナップショットテスト追加 |
| `tests/e2e/scripts/` | E2E テスト追加 |

## 作業ステップ

- [ ] ステップ1: RED — `const f: (x: number) => Point = (x) => { return { x, y: 0 } }` の変換テスト作成 → 失敗確認
- [ ] ステップ2: GREEN — `convert_var_decl_arrow_fns` に型注釈読み取り追加
  - 変数の `type_ann` から `RustType` を取得
  - `RustType::Fn { return_type }` から戻り値型を抽出
  - Arrow body の変換に `return_type` を渡す
- [ ] ステップ3: RED/GREEN — TypeRegistry 経由の型エイリアス解決
  - `const f: MyFnType = ...` で `MyFnType` が `TypeDef::Function` として登録されているケース
  - `reg.get(name)` で `TypeDef::Function { return_type }` を取得
- [ ] ステップ4: スナップショットテスト・E2Eテスト追加
- [ ] ステップ5: Hono 再変換で該当16ファイルのエラー解消確認

## テスト計画

### ユニットテスト

- `const f: (x: number) => Point = (x) => { return { x, y: 0 } }` → 戻り値のオブジェクトリテラルが `Point` 構造体に変換される
- `type Handler = (c: Context) => Response; const h: Handler = (c) => { return { status: 200 } }` → TypeRegistry 経由で戻り値型解決
- Arrow の明示的戻り値型と変数型注釈の両方がある場合 → Arrow の明示型が優先

### スナップショットテスト

- `var-type-arrow.input.ts` — interface 定義 + 型注釈付き Arrow 関数 + return オブジェクトリテラル

### E2Eテスト

- 型注釈付き Arrow 関数で構造体を返し、フィールドにアクセスして値を検証

## 完了条件

1. `cargo test` 全テスト通過（0 エラー・0 警告）
2. `cargo clippy --all-targets --all-features -- -D warnings` 通過
3. `cargo fmt --all --check` 通過
4. Hono の該当16ファイルで I-112a に起因するエラーが解消されている
5. Hono 全151ファイルの再変換結果がレポートに反映されている
