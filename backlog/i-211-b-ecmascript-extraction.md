# I-211-b: ECMAScript 標準型の抽出 + JSON ファイル分割

## 背景・動機

I-211-a で器（複数シグネチャ対応 + Union の合成 enum 変換 + オーバーロード解決）が整備された。この PRD では中身（ECMAScript 標準型データ）を投入する。

現在の `builtin_types.json`（106 型）は `lib.dom.d.ts` の Web API 型のみを含む。`tools/extract-types/src/index.ts` の `libMap` は `dom`, `webworker`, `es2024` の 3 ファイルのみマッピングされており、ECMAScript 標準ライブラリ（`lib.es5.d.ts`, `lib.es2015.*.d.ts`）は対象外。

これにより `String`, `Array`, `Date`, `Error`, `RegExp`, `Map`, `Set` 等の型が TypeRegistry に存在せず、メソッドチェーン型追跡と instanceof の型解決が機能しない。

## ゴール

1. `String`, `Array`, `Date`, `Error`, `RegExp`, `Map`, `Set`, `WeakMap`, `WeakSet`, `Symbol`, `Promise`, `JSON`, `Math`, `Number`, `Object` の型定義（フィールド + メソッドシグネチャ）が TypeRegistry に登録される
2. 型定義ファイルが `src/builtin_types/web_api.json` と `src/builtin_types/ecmascript.json` に分割される
3. 抽出手順・対象 lib・TypeScript バージョンが文書化される
4. 既存テストが全て通り、ベンチマークでエラー数が減少（または同等）

## スコープ

### 対象

- `tools/extract-types/` の抽出スクリプトに ES5/ES2015 lib を追加
- ECMAScript 標準型のフィルタリングルート定義
- `src/builtin_types.json` → `src/builtin_types/web_api.json` + `ecmascript.json` への分割
- `src/external_types.rs` のローダーを複数ファイル対応に変更
- `tools/extract-types/README.md` と `src/builtin_types/README.md` の作成

### 対象外

- ES2016〜ES2024 の追加メソッド（TODO に記録）
- `Intl` 名前空間
- データモデル変更（I-211-a で完了済み）
- E2E テスト・ベンチマーク効果測定（I-211-c）

## 設計

### 技術的アプローチ

#### 1. 抽出スクリプトの拡張

**`tools/extract-types/src/index.ts`** の `libMap` を拡張:

```typescript
// Before（3 エントリ）
const libMap: Record<string, string> = {
  dom: "lib.dom.d.ts",
  webworker: "lib.webworker.d.ts",
  es2024: "lib.es2024.d.ts",
};

// After: ES5 + ES2015 サブモジュールを追加
const libMap: Record<string, string> = {
  dom: "lib.dom.d.ts",
  webworker: "lib.webworker.d.ts",
  es2024: "lib.es2024.d.ts",
  es5: "lib.es5.d.ts",
  "es2015.core": "lib.es2015.core.d.ts",
  "es2015.collection": "lib.es2015.collection.d.ts",
  "es2015.symbol": "lib.es2015.symbol.d.ts",
  "es2015.symbol.wellknown": "lib.es2015.symbol.wellknown.d.ts",
  "es2015.promise": "lib.es2015.promise.d.ts",
  "es2015.iterable": "lib.es2015.iterable.d.ts",
  "es2015.generator": "lib.es2015.generator.d.ts",
  "es2015.proxy": "lib.es2015.proxy.d.ts",
  "es2015.reflect": "lib.es2015.reflect.d.ts",
};
```

**`tools/extract-types/src/filter.ts`** に ECMAScript 標準型のルートリストを追加:

```typescript
export const ECMASCRIPT_TYPES: string[] = [
  // ES5 コア
  "String", "Number", "Boolean", "Object", "Function",
  "Array", "Date", "Error", "RegExp", "JSON", "Math",
  // ES5 エラー型
  "TypeError", "RangeError", "SyntaxError", "ReferenceError", "EvalError", "URIError",
  // ES2015 コレクション
  "Map", "Set", "WeakMap", "WeakSet",
  // ES2015 その他
  "Symbol", "Promise", "Proxy", "Reflect",
  // TypedArray
  "ArrayBuffer", "DataView",
  "Int8Array", "Uint8Array", "Uint8ClampedArray",
  "Int16Array", "Uint16Array", "Int32Array", "Uint32Array",
  "Float32Array", "Float64Array",
  // Iterator/Generator
  "Iterator", "Generator", "GeneratorFunction",
  "IterableIterator", "IteratorResult",
];
```

CLI に `--ecmascript` フラグを追加し、上記ルートリストでフィルタリング:

```
node dist/index.js --lib es5,es2015.core,es2015.collection,... --ecmascript
```

#### 2. JSON ファイル分割 + ローダー更新

**ファイル構成**:

```
src/builtin_types/
├── README.md           ← 各ファイルの抽出元と更新手順
├── web_api.json        ← 既存 builtin_types.json の内容（リネーム）
└── ecmascript.json     ← 新規抽出
```

**`src/external_types.rs`** のローダー変更:

```rust
// Before
const BUILTIN_TYPES_JSON: &str = include_str!("builtin_types.json");

pub fn load_builtin_types() -> Result<TypeRegistry> {
    load_types_json(BUILTIN_TYPES_JSON)
}

// After
const WEB_API_TYPES_JSON: &str = include_str!("builtin_types/web_api.json");
const ECMASCRIPT_TYPES_JSON: &str = include_str!("builtin_types/ecmascript.json");

pub fn load_builtin_types() -> Result<TypeRegistry> {
    let mut registry = TypeRegistry::new();
    load_types_into(&mut registry, ECMASCRIPT_TYPES_JSON)?;
    load_types_into(&mut registry, WEB_API_TYPES_JSON)?;
    Ok(registry)
}
```

`load_types_into` は既存の `load_types_json` を分解して作る。既存の `load_types_json` は JSON を parse → TypeRegistry を返す。新しい `load_types_into` は JSON を parse → 既存の TypeRegistry にマージ。

ECMAScript を先に読み込み、Web API を後に読み込む。理由:
1. `TypeRegistry::register` は `HashMap::insert`（上書き）。`Promise` 等の両方に定義がある型は、Web API 版（declaration merging で DOM 拡張を含む、より完全な定義）が最終的に登録される
2. 現在のローダーは型間の参照解決を行わないため、読み込み順序は `RustType::Named` 参照の解決には影響しない。将来的に参照解決を行う場合に備えて ECMAScript → Web API の順序とする

**`web_api.json` の再抽出は不要**: 現在の `web_api.json`（旧 `builtin_types.json`）は JSON レベルで全シグネチャを保持している。I-211-a で `first()` を除去したことで、既存 JSON から全シグネチャがロードされるようになる。

#### 3. ドキュメント化

**`tools/extract-types/README.md`**:

- ツールの目的と概要
- 前提条件（Node.js, npm）
- ビルド手順: `npm install && npm run build`
- 抽出コマンド例:
  - Web API: `node dist/index.js --lib dom,webworker,es2024 --server-web-api > ../../src/builtin_types/web_api.json`
  - ECMAScript: `node dist/index.js --lib es5,es2015.core,es2015.collection,es2015.symbol,es2015.symbol.wellknown,es2015.promise,es2015.iterable,es2015.generator,es2015.proxy,es2015.reflect --ecmascript > ../../src/builtin_types/ecmascript.json`
- テスト: `npm test`
- 使用している TypeScript バージョン（`package.json` から: `^5.9.0`）
- JSON フォーマットの概要

**`src/builtin_types/README.md`**:

- 各ファイルの抽出元 lib と最終更新日
- `web_api.json`: `lib.dom.d.ts` + `lib.webworker.d.ts` + `lib.es2024.d.ts` → `--server-web-api` フィルタ
- `ecmascript.json`: `lib.es5.d.ts` + `lib.es2015.*.d.ts` → `--ecmascript` フィルタ
- 再抽出手順（`tools/extract-types/` を参照）

### 設計整合性レビュー

- **高次の整合性**: 既存の型抽出・ローダーパイプラインを拡張する変更。新しいアーキテクチャの導入なし。`include_str!` + `load_types_into` のパターンは既存の `load_builtin_types` と一貫
- **DRY / 直交性**: Web API と ECMAScript のフィルタリングロジックが `filter.ts` 内で分離。ローダーは同一の `load_types_into` で両方を処理。型の種類に関わらず同じ `TypeDef` に変換される
- **結合度**: `external_types.rs` のインターフェースは `load_builtin_types() -> Result<TypeRegistry>` のまま変更なし。呼び出し側への影響ゼロ
- **割れ窓**: なし

### 影響範囲

| モジュール | 変更内容 |
|-----------|---------|
| `tools/extract-types/src/index.ts` | `libMap` 拡張 + `--ecmascript` フラグ |
| `tools/extract-types/src/filter.ts` | `ECMASCRIPT_TYPES` ルートリスト追加 + ECMAScript フィルタ関数 |
| `src/external_types.rs` | `include_str!` 2 分割 + `load_types_into` 関数追加 |
| `src/builtin_types.json` | 削除（`src/builtin_types/web_api.json` にリネーム） |
| `src/builtin_types/` | 新規ディレクトリ + `web_api.json` + `ecmascript.json` + `README.md` |
| `tools/extract-types/README.md` | 新規作成 |

## タスク一覧

### T1: 抽出スクリプトの ES5/ES2015 対応

- **作業内容**:
  - `tools/extract-types/src/index.ts` の `libMap` に ES5 + ES2015 サブモジュールを追加
  - `tools/extract-types/src/filter.ts` に `ECMASCRIPT_TYPES` ルートリストと `filterEcmascriptTypes` 関数を追加
  - `tools/extract-types/src/index.ts` に `--ecmascript` CLI フラグを追加し、`ECMASCRIPT_TYPES` によるフィルタリングを適用
  - `tools/extract-types/src/extractor.test.ts` に ES5 lib からの `String`/`Array` 抽出テストを追加
- **完了条件**:
  - `cd tools/extract-types && npm run build && node dist/index.js --lib es5,es2015.core,es2015.collection,es2015.symbol,es2015.symbol.wellknown,es2015.promise,es2015.iterable --ecmascript` で JSON が出力される
  - 出力 JSON に `String`（`trim`, `split`, `toLowerCase` 等のメソッド付き）, `Array`（`map`, `filter`, `find` 等のメソッド付き）, `Date`, `Error`, `Map`, `Set` が含まれる
  - `String.split` のシグネチャに `return_type: { kind: "array", element: { kind: "string" } }` が含まれる
  - `npm test` が通る
- **依存**: なし

### T2: JSON ファイル分割 + ローダー更新

- **作業内容**:
  - `src/builtin_types/` ディレクトリを作成
  - `src/builtin_types.json` を `src/builtin_types/web_api.json` に移動
  - T1 の抽出コマンドで `src/builtin_types/ecmascript.json` を生成
  - `src/external_types.rs` を変更:
    - `BUILTIN_TYPES_JSON` → `WEB_API_TYPES_JSON` + `ECMASCRIPT_TYPES_JSON`
    - `load_builtin_types` を `load_types_into` ベースに変更
    - `load_types_json`（`--tsconfig` モードで使用）は既存インターフェースを維持
  - `include_str!` のパスを `"builtin_types/web_api.json"` と `"builtin_types/ecmascript.json"` に変更
- **完了条件**:
  - `load_builtin_types()` が `Response`（Web API）と `Date`（ECMAScript）の両方を含む TypeRegistry を返す
  - `registry.get("String")` が `Some(TypeDef::Struct { methods, .. })` を返し、`methods` に `trim`, `split`, `toLowerCase` が含まれる
  - `registry.get("Array")` が `Some` を返し、`methods` に `map`, `filter`, `find` が含まれる
  - `cargo check` が通る
  - 既存テストが全て通る
- **依存**: T1（抽出済み JSON）

### T3: ドキュメント化

- **作業内容**:
  - `tools/extract-types/README.md` を作成（ツール概要、ビルド手順、抽出コマンド例、TypeScript バージョン、JSON フォーマット概要）
  - `src/builtin_types/README.md` を作成（各ファイルの抽出元、最終更新日、再抽出手順）
- **完了条件**:
  - README の手順のみで第三者が型定義の再抽出・更新ができること
- **依存**: T1, T2

## テスト計画

### 抽出スクリプトテスト（vitest）

- ES5 lib からの `String` 型抽出: フィールドなし、`trim`/`split`/`toLowerCase` 等のメソッド、複数シグネチャ保持
- ES5 lib からの `Array` 型抽出: `map`/`filter`/`find` メソッド、ジェネリック型パラメータ
- ES2015 lib からの `Map`/`Set` 型抽出: コンストラクタ、`get`/`set`/`has` メソッド
- フィルタリング: `ECMASCRIPT_TYPES` に含まれない型（DOM 型等）が出力に含まれないこと

### ローダーテスト（Rust）

- `load_builtin_types()` が Web API 型と ECMAScript 型の両方を返す
- 型の重複がある場合の挙動: `TypeRegistry::register` は `HashMap::insert` で上書き。ECMAScript → Web API の順で読み込むため、両方に定義がある型（`Promise` 等）は Web API 版が優先される。Web API 版は DOM 固有の拡張（`PromiseLike` 等との declaration merging）を含むため、より完全な定義になる

### 回帰テスト

- 既存の全テストが通ること（Web API 型の読み込みが壊れていないこと）
- ベンチマーク: エラー数が減少（新しい型が TypeRegistry に追加されるため、未解決型によるエラーが減るはず）

## 完了条件

1. `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
2. `cargo fmt --all --check` が通る
3. `cargo test` が全テスト通過
4. `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` が通る
5. `registry.get("String")`, `registry.get("Array")`, `registry.get("Date")`, `registry.get("Error")`, `registry.get("Map")`, `registry.get("Set")` が全て `Some` を返す
6. `src/builtin_types/web_api.json` と `src/builtin_types/ecmascript.json` に分割されている
7. `src/builtin_types.json` が削除されている
8. `tools/extract-types/README.md` と `src/builtin_types/README.md` が存在し、再抽出手順が記載されている
9. `cd tools/extract-types && npm test` が通る
