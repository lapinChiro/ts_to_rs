# extract-types

TypeScript Compiler API を使用して、TypeScript の型定義ファイル（lib.dom.d.ts 等）から完全に解決された型情報を JSON 形式で抽出するツール。

## 前提条件

- Node.js >= 20
- npm

## ビルド

```bash
cd tools/extract-types
npm install
npm run build
```

## 抽出コマンド

### Web API 型（lib.dom.d.ts ベース）

```bash
node dist/index.js \
  --lib dom,webworker,es2024 \
  --server-web-api \
  > ../../src/builtin_types/web_api.json
```

### ECMAScript 標準型（lib.es5.d.ts + lib.es2015.*.d.ts ベース）

```bash
node dist/index.js \
  --lib es5,es2015.core,es2015.collection,es2015.symbol,es2015.symbol.wellknown,es2015.promise,es2015.iterable,es2015.generator,es2015.proxy,es2015.reflect \
  --ecmascript \
  > ../../src/builtin_types/ecmascript.json
```

### プロジェクトの型抽出（tsconfig.json ベース）

```bash
node dist/index.js --tsconfig ./path/to/tsconfig.json
```

## テスト

```bash
npm test
```

## TypeScript バージョン

`package.json` で `typescript: ^5.9.0` を使用。抽出元の lib ファイルはこのバージョンに同梱されたものが使われる。

## JSON フォーマット

出力は以下の構造:

```json
{
  "version": 1,
  "types": {
    "TypeName": {
      "kind": "interface",
      "fields": [{ "name": "fieldName", "type": { "kind": "string" } }],
      "methods": {
        "methodName": {
          "signatures": [{
            "params": [{ "name": "p", "type": { "kind": "string" } }],
            "return_type": { "kind": "number" }
          }]
        }
      },
      "constructors": [{ "params": [...], "return_type": { ... } }]
    }
  }
}
```

型定義の `kind` は `"interface"`, `"function"`, `"alias"` の 3 種類。型表現の `kind` は `"string"`, `"number"`, `"boolean"`, `"void"`, `"any"`, `"unknown"`, `"never"`, `"null"`, `"undefined"`, `"named"`, `"array"`, `"tuple"`, `"union"`, `"function"` の 14 種類。

## CLI フラグ

| フラグ | 説明 |
|--------|------|
| `--lib <names>` | カンマ区切りの lib 名（dom, webworker, es2024, es5, es2015.core 等） |
| `--tsconfig <path>` | tsconfig.json のパス |
| `--files <paths...>` | 個別の .d.ts ファイル |
| `--server-web-api` | Server Web API 型のみにフィルタリング |
| `--ecmascript` | ECMAScript 標準型のみにフィルタリング |
