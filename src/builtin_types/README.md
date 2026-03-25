# builtin_types

ts_to_rs バイナリに埋め込まれるビルトイン型定義の JSON ファイル。

## ファイル一覧

| ファイル | 抽出元 | フィルタ | 最終更新 |
|---------|--------|---------|---------|
| `web_api.json` | lib.dom.d.ts + lib.webworker.d.ts + lib.es2024.d.ts | `--server-web-api` | 2026-03-25 |
| `ecmascript.json` | lib.es5.d.ts + lib.es2015.*.d.ts | `--ecmascript` | 2026-03-25 |

## 再抽出手順

TypeScript の新バージョンが出た場合や、型定義を更新する場合:

```bash
cd tools/extract-types
npm install
npm run build

# Web API 型
node dist/index.js \
  --lib dom,webworker,es2024 \
  --server-web-api \
  > ../../src/builtin_types/web_api.json

# ECMAScript 標準型
node dist/index.js \
  --lib es5,es2015.core,es2015.collection,es2015.symbol,es2015.symbol.wellknown,es2015.promise,es2015.iterable,es2015.generator,es2015.proxy,es2015.reflect \
  --ecmascript \
  > ../../src/builtin_types/ecmascript.json
```

詳細は [tools/extract-types/README.md](../../tools/extract-types/README.md) を参照。

## 読み込み順序

`src/external_types.rs` の `load_builtin_types()` で ECMAScript → Web API の順に読み込む。`TypeRegistry::register` は `HashMap::insert`（上書き）のため、両方に定義がある型（`Promise` 等）は Web API 版が優先される。
