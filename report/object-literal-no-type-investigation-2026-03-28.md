# OBJECT_LITERAL_NO_TYPE 50件 詳細調査レポート

**日付**: 2026-03-28
**対象**: Hono ベンチマーク `./scripts/hono-bench.sh` で検出される 50件の `"object literal requires a type annotation to determine struct name"` エラー

## 要約

50件の OBJECT_LITERAL_NO_TYPE エラーを個別にソースコードを確認して分類した。最大の発見は、**9件（18%）がディレクトリモード固有のバグ**であり、`client/types.ts` の `export interface Response {}` がビルトイン `Response` 型を上書きし、コンストラクタ情報が失われることが根本原因である。

I-266 のコンストラクタ引数 expected type 伝播は**正しく動作している**。単一ファイルモードでは `new Response(body, { status, headers })` は正しく変換される。ディレクトリモードで失敗するのは I-266 の不具合ではなく、TypeRegistry の merge 戦略の問題。

## 分類結果

| カテゴリ | 件数 | 説明 |
|----------|------|------|
| **X: ディレクトリモード固有** | **9** | `client/types.ts` の `export interface Response {}` がビルトイン `Response` を上書き |
| **G: return オブジェクトリテラル** | **9** | `return {}` や `return {field: value}` で戻り値型から expected type が逆引きされない |
| **H: フォールバック空オブジェクト** | **9** | `x || {}`, `x ?? {}`, `param = {}` のデフォルト値 |
| **D: コンストラクタ/関数/メソッド引数** | **9** | 引数位置のオブジェクトリテラルで expected type が伝播されないケース |
| **F: 型注釈なし変数** | **6** | `const x = {}`, `let x = {}`, `Record<K,V> = {}` |
| **B: ジェネリクスデフォルト値** | **3** | `<S extends Schema = {}>` のデフォルト値 |
| **L: メソッド呼び出し引数** | **3** | `.push({...})`, `.unshift({...})` のオブジェクト引数 |
| **A: super() 引数** | **1** | `super(msg, {cause})` — Error の2引数コンストラクタ未定義 |
| **K: フィールド代入** | **1** | `result.headers = {...}` |
| **合計** | **50** | |

## カテゴリ別詳細

### X: ディレクトリモード固有（9件） — **根本原因特定済み**

**根本原因**: `client/types.ts:232` に `export interface Response extends ClientResponse<unknown> {}` が定義されている。ディレクトリモードで全ファイルの TypeRegistry を merge する際、`TypeRegistry::merge()` (`src/registry/mod.rs:366`) が無条件に `insert` するため、ビルトインの `Response`（constructor 付き）がソース定義の `Response`（constructor なし、フィールドなし）で上書きされる。

**結果**: `resolve_new_expr` が `Response` を lookup すると `constructor: None` のため field-based fallback に入るが、フィールドも 0 件で param_types が空になり、`new Response(body, { status })` の第2引数に expected type が伝播されない。

**再現**: 2ファイルの最小ケースで再現確認済み:
```
// client/types.ts
export interface Response {}

// main.ts
function test(): Response {
    return new Response("hello", { status: 200 })  // → OBJECT_LITERAL_NO_TYPE
}
```

**対象ファイル**:
- `middleware/bearer-auth/index.ts:103`
- `middleware/body-limit/index.ts:57`
- `middleware/cors/index.ts:63`
- `middleware/csrf/index.ts:94`
- `middleware/etag/index.ts:79`
- `middleware/jwk/jwk.ts:170`
- `middleware/jwt/jwt.ts:160`
- `utils/buffer.ts:106`
- `helper/ssg/plugins.ts:55`

**修正方向**: `TypeRegistry::merge()` でビルトイン型（`external_types` フラグ付き）がソース定義型で上書きされないようにする。またはビルトイン型の constructor 情報をソース定義型に引き継ぐ。

### G: return オブジェクトリテラル（9件）

関数の戻り値型から expected type を逆引きする機構が未実装。I-267 の対象。

- `return {}` 空オブジェクト: `helper/cookie/index.ts:27,50`, `utils/body.ts:94,121`
- `return {field: value}` プロパティ付き: `helper/css/index.ts:61`, `helper/dev/index.ts:27,39`, `utils/concurrent.ts:12`
- `.map(x => ({field: x}))` コールバック内 return: `middleware/language/language.ts:74`

### H: フォールバック/デフォルト空オブジェクト（9件）

`x || {}`, `x ?? {}`, `param = {}` パターン。空オブジェクト `{}` に expected type がないため失敗。

- `options.verification || {}`: `middleware/jwk/jwk.ts:48`, `middleware/jwt/jwt.ts:53`, `utils/jwt/jwt.ts:56,191`
- `Object.entries(x || {})`: `adapter/aws-lambda/handler.ts:268,429,496`
- `proxyInit ?? {}`: `helper/proxy/index.ts:160`
- `constructor(opts: T = {})`: `hono-base.ts:98`

### D: コンストラクタ/関数/メソッド引数（9件）

I-266 が対象とした「コンストラクタ引数の expected type 伝播」では解消されないケース。各サブパターンの理由が異なる。

#### D-CALL-SPREAD: 関数呼び出しでスプレッドオブジェクト（4件）
- `baseServeStatic({...options, getContent, join})`: `adapter/bun/serve-static.ts:8`, `adapter/deno/serve-static.ts:8`
- `serializeSigned(name, value, secret, {path, ...opt})`: `helper/cookie/index.ts:104`
- `component({...props, Layout}, c)`: `middleware/jsx-renderer/index.ts:33`

**原因**: 呼び出し先の関数シグネチャのパラメータ型がジェネリクスや動的型のため、expected type として Named struct を特定できない。

#### D-FETCH: app.fetch() メソッド呼び出し（2件）
- `app.fetch(req, {...env...})`: `adapter/lambda-edge/handler.ts:116`, `adapter/service-worker/handler.ts:18`

**原因**: `Hono.fetch()` の第2引数は `E['Bindings'] | {}` 型。union/indexed access で Named struct に解決されない。

#### D-NEW-CONTEXT: ソース定義クラスの constructor 引数（1件）
- `new Context(req, {...options...})`: `adapter/cloudflare-pages/handler.ts:32`

**原因**: `Context` の constructor 第2引数は `ContextOptions<E>` で、ジェネリクス + indexed access (`E['Bindings']`) を含む。型解決で Named struct に到達できない。

#### D-NEW-PROXY: ビルトイン未登録型（1件）
- `new Proxy(() => {}, {get, apply})`: `client/client.ts:15`

**原因**: `Proxy` がビルトイン型定義に存在しない（I-264 で記録済み）。

#### D-CALL: 関数呼び出しの空オブジェクト引数（1件）
- `resolveCallback(data, phase, false, {})`: `helper/streaming/sse.ts:13`

**原因**: `resolveCallback` の第4引数の型がジェネリクスで Named struct に解決されない。

### F: 型注釈なし変数（6件）

- `const merged = {...target} as ObjectType<T>`: `client/utils.ts:66`
- `const remoteData = {addr, type, isIPv4}`: `middleware/ip-restriction/index.ts:128`
- `const staticMap = {} as StaticMap<T>`: `router/reg-exp-router/prepared-router.ts:9`
- `const context = {}`: `utils/html.ts:129`
- `let value = {}`: `validator/validator.ts:46`
- `Record<string, () => T> = {...}`: `helper/adapter/index.ts:10` — `Record` は Named struct ではない

### L: メソッド呼び出し引数（3件）

`.push()` / `.unshift()` の引数オブジェクトリテラル。配列の要素型から expected type を逆引きする機構が必要。

- `acc.push({path})`: `helper/ssg/utils.ts:61`
- `users.unshift({username, password})`: `middleware/basic-auth/index.ts:80`
- `curNode.#methods.push({method, handler})`: `router/trie-router/node.ts:25`

### A: super() 引数（1件）

- `super(options?.message, {cause: options?.cause})`: `http-exception.ts:46`

**原因**: ビルトインの `Error` コンストラクタに1引数シグネチャ `(message: string)` しか定義されておらず、ES2022 の2引数形式 `(message: string, options?: ErrorOptions)` が未登録。

### B: ジェネリクスデフォルト値（3件）

- `<S extends Schema = {}>` 系: `adapter/aws-lambda/handler.ts:239`, `adapter/cloudflare-pages/handler.ts:49`, `request.ts:36`

### K: フィールド代入（1件）

- `result.headers = {...}`: `adapter/aws-lambda/handler.ts:578`

## I-266 の効果評価

I-266 は `resolve_new_expr` でコンストラクタシグネチャを参照し、引数の expected type を伝播する仕組みを実装した。これは**正しく動作している**:

- 単一ファイルモードでは `new Response(body, {init})` は正しく `ResponseInit` として変換される
- `new HTTPException(status, {res})` も正しく変換される
- `new ReadableStream({pull, cancel})` も正しく変換される

**I-266 で解消されなかった理由**は、個々のエラーによって異なる:

| 原因 | 件数 |
|------|------|
| `client/types.ts` がビルトイン `Response` を上書き（ディレクトリモード merge バグ） | 9 |
| 戻り値型からの逆引き未実装（I-267 の対象） | 9 |
| フォールバック空オブジェクト `|| {}` / `?? {}` | 9 |
| 呼び出し先のパラメータ型がジェネリクス/union で Named に解決されない | 7 |
| 型注釈なし変数 | 6 |
| ビルトイン型の不備（Error 2引数, Proxy 未登録） | 2 |
| メソッド呼び出し引数（配列要素型の逆引き未実装） | 3 |
| ジェネリクスデフォルト値 `= {}` | 3 |
| フィールド代入 | 1 |
| **I-266 の不具合** | **0** |

## 推奨アクション（優先度順）

1. **TypeRegistry merge 戦略の修正**（9件解消見込み）: ビルトイン型の constructor 情報がソース定義型で上書きされないようにする。`src/registry/mod.rs:366` の `merge()` を修正
2. **I-267 return 型逆引き**（9件解消見込み）: 既存の PRD `backlog/I-267-return-object-literal-expected-type.md` を実装
3. **Error 2引数コンストラクタの追加**（1件解消 + 波及効果）: `src/builtin_types/ecmascript.json` の `Error` に `(message: string, options?: ErrorOptions)` を追加
4. **フォールバック空オブジェクトの型推論**（最大9件）: `x || {}` で `x` の型から `{}` の expected type を推論
