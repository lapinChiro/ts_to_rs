# Hono ベンチマーク エラー定量分析レポート

- **基準コミット**: `ba8b6a4`（未コミットの I-211-b/c 変更を含む状態で計測）
- **計測日**: 2026-03-25
- **Hono バージョン**: `/tmp/hono-src` のクローン時点

## サマリ

| 指標 | 値 |
|------|-----|
| 総ファイル数 | 158 |
| クリーンファイル（エラー 0） | 93 (58.9%) |
| コンパイルクリーン（ファイル単位） | 92 (58.2%) |
| コンパイルクリーン（ディレクトリ単位） | 156 (98.7%) |
| エラーインスタンス | **114** |
| エラーが発生するファイル | 65 |
| エラーカテゴリ数 | 24 |

## カテゴリ別エラー集計

| # | カテゴリ | 件数 | 割合 | 関連 TODO |
|---|---------|------|------|-----------|
| 1 | OBJECT_LITERAL_NO_TYPE | 52 | 45.6% | I-112c, I-224 |
| 2 | INTERSECTION_TYPE | 9 | 7.9% | I-221, I-101 |
| 3 | INDEXED_ACCESS | 6 | 5.3% | I-35 |
| 4 | TYPE_ALIAS_COND_TYPE | 5 | 4.4% | I-219 |
| 5 | TYPE_ALIAS_MAPPED_TYPE | 5 | 4.4% | I-200 |
| 6 | FN_TYPE_PARAM | 4 | 3.5% | I-139 |
| 7 | QUALIFIED_TYPE | 3 | 2.6% | I-36 |
| 8 | ARROW_DEFAULT_PARAM | 3 | 2.6% | I-195 |
| 9 | TYPEOF_TYPE | 3 | 2.6% | I-194 |
| 10 | ASSIGN_TARGET | 3 | 2.6% | I-240, I-151 |
| 11 | TYPE_ALIAS_INFER | 3 | 2.6% | I-219 |
| 12 | INTERFACE_MEMBER | 2 | 1.8% | I-228 |
| 13 | MEMBER_PROPERTY | 2 | 1.8% | I-229 |
| 14-24 | その他（各 1 件） | 14 | 12.3% | 各種 |

## OBJECT_LITERAL_NO_TYPE の内訳分析（52 件）

最大カテゴリである OBJECT_LITERAL_NO_TYPE 52 件を、型が解決できない根本原因で細分類した。

> **⚠️ 訂正（2026-03-27）**: 当初の `function_arg` 分類は不正確だった。2026-03-27 の詳細調査（デバッグ出力による全 66 件の個別分析）により、`function_arg` と分類した 20 件の実際の根本原因は多様であることが判明した。既存の `set_call_arg_expected_types` は正常に動作しており、「関数引数位置の expected type 逆引き機構が不在」という前提が誤りだった。正確な分類は `backlog/I-266-constructor-and-call-arg-expected-types.md` を参照。

| 根本原因 | 件数 | 割合 | 解消に必要な開発 |
|----------|------|------|------------------|
| ~~**function_arg**~~ | ~~20~~ | ~~38.5%~~ | ~~関数引数位置のオブジェクトリテラルに対し、関数シグネチャのパラメータ型から expected type を逆引きする~~ |
| **return_new** | 14 | 26.9% | `return new Xxx({...})` や `return {...}` でクラスコンストラクタ/関数の戻り値型から型を推定する |
| **generic_param** | 14 | 26.9% | `E extends Env` のようなジェネリクス型パラメータのフィールドを `TypeRegistry::instantiate` で展開する |
| **optional_spread** | 4 | 7.7% | `options?: CORSOptions` のスプレッドで `Option<T>` を unwrap してフィールド展開する |

**2026-03-27 訂正後の正確な分類**（ディレクトリモード変換で全 66 件のオブジェクトリテラルをデバッグ出力で個別分析）:

| 根本原因 | 件数 | 説明 |
|----------|------|------|
| PROPS_NO_EXPECTED | 29 | プロパティあり・expected 未設定（`new` 式引数、型注釈なし変数等） |
| EMPTY_OBJ_NO_EXPECTED | 19 | `return {}`、`|| {}` 等の空オブジェクト |
| SPREAD_NO_EXPECTED | 7 | スプレッド含むが expected 未設定 |
| WRONG_EXPECTED(String) | 6 | `resolve_new_expr` が struct フィールドをコンストラクタパラメータと誤認 |
| WRONG_EXPECTED(Tuple) | 2 | 同上 |
| WRONG_EXPECTED(Bool) | 2 | 同上 |
| WRONG_EXPECTED(Any) | 1 | パラメータ型 `unknown` で構造体名特定不可 |

最大の根本原因は **`resolve_new_expr` がコンストラクタパラメータではなく struct フィールドを引数の expected type として使用するバグ**（WRONG_EXPECTED 計 10 件 + PROPS_NO_EXPECTED の多数）。

### ~~function_arg（20 件）の例~~ — 訂正: 分類が不正確

```typescript
// middleware/basic-auth/index.ts:80
const basicAuth = (options: BasicAuthOptions) => {
    return async function basicAuth(c, next) {
        const requestUser = { username: "", password: "" };  // ← ここ
        // ...
    }
}
```

~~オブジェクトリテラルが関数のローカル変数として宣言され、型注釈がない。関数引数のパラメータ型や、その後の代入先から型を推定する必要がある。~~

**訂正**: 上記の例を含む 20 件は、実際には `new Response(body, { status })` のコンストラクタ引数、型注釈なし変数宣言、コンディショナル型パラメータ等の混合であり、単一の「関数引数 expected type」施策では解消できない。詳細は `backlog/I-266-constructor-and-call-arg-expected-types.md` を参照。

### return_new（14 件）の例

```typescript
// adapter/aws-lambda/handler.ts:268
return new Response(body, { status, headers });  // ← {status, headers} の型が不明
```

コンストラクタや関数呼び出しの引数位置にあるオブジェクトリテラル。呼び出し先の型情報（ResponseInit 等）から型を逆引きする必要がある。

### generic_param（14 件）の例

```typescript
// adapter/bun/serve-static.ts:8
export const serveStatic = <E extends Env = Env>(options: ServeStaticOptions<E>) => {
    return async function serveStatic(c, next) {
        const path = getFilePath({ filename: options.path ?? ... });  // ← ここ
    }
}
```

`E extends Env` の制約からフィールド情報を取得し、ジェネリクスパラメータをスプレッドソースとして展開する必要がある。

### optional_spread（4 件）の例

```typescript
// middleware/cors/index.ts:63
const cors = (options?: CORSOptions) => {
    const defaults = { origin: "*", ...options };  // ← options? のスプレッド
}
```

`options?: CORSOptions` は `Option<CORSOptions>` に変換されるため、スプレッド時に unwrap が必要。

## 開発施策の定量的効果予測

以下、各施策の実装により解消が見込まれるエラー数を予測する。

### 施策 A: ~~関数引数位置の expected type 逆引き~~ → コンストラクタ引数 expected type 修正

> **⚠️ 訂正（2026-03-27）**: 通常の関数呼び出しの expected type 伝播（`set_call_arg_expected_types`）は既に正常動作していた。真の問題は `resolve_new_expr` がコンストラクタパラメータではなく struct フィールドを使用するバグ。詳細は `backlog/I-266-constructor-and-call-arg-expected-types.md` を参照。

| 項目 | 値 |
|------|-----|
| 対象カテゴリ | OBJECT_LITERAL_NO_TYPE（WRONG_EXPECTED + PROPS_NO_EXPECTED の `new` 式部分） |
| 解消見込み | **10-20 件**（WRONG_EXPECTED 10 件確実 + PROPS_NO_EXPECTED の `new` 式部分） |
| 残エラー予測 | 114 → 94-104 |
| 技術概要 | (1) TypeDef::Struct にコンストラクタシグネチャを追加 (2) `collect_class_info` で Constructor を収集 (3) ビルトイン型にコンストラクタ情報追加 (4) `resolve_new_expr` をコンストラクタパラメータ優先に変更 |
| 依存 | なし |
| 関連 TODO | I-266 |

### 施策 B: ジェネリクスパラメータのフィールド展開

| 項目 | 値 |
|------|-----|
| 対象カテゴリ | OBJECT_LITERAL_NO_TYPE (generic_param) |
| 解消見込み | **14 件** |
| 残エラー予測 | 94 → 80 |
| 技術概要 | `E extends Env` のようなジェネリクス型パラメータをスプレッドソースとして展開。`TypeRegistry::instantiate` で制約型のフィールド情報を取得し、匿名構造体のフィールドとして展開する |
| 依存 | TypeRegistry, instantiate 基盤（構築済み） |
| 関連 TODO | I-112c 追加改善（ジェネリクスフィールド展開） |

### 施策 C: return_new の型逆引き

| 項目 | 値 |
|------|-----|
| 対象カテゴリ | OBJECT_LITERAL_NO_TYPE (return_new) |
| 解消見込み | **~10 件**（14 件中、コンストラクタ引数は施策 A と重複） |
| 残エラー予測 | 80 → 70 |
| 技術概要 | `return new Response(body, init)` の `init` 位置でコンストラクタのパラメータ型（`ResponseInit`）を TypeRegistry から逆引きする。施策 A の拡張 |
| 依存 | 施策 A |
| 関連 TODO | I-112c の残タスク |

### 施策 D: Optional 型スプレッドの unwrap

| 項目 | 値 |
|------|-----|
| 対象カテゴリ | OBJECT_LITERAL_NO_TYPE (optional_spread) |
| 解消見込み | **4 件** |
| 残エラー予測 | 70 → 66 |
| 技術概要 | `options?: CORSOptions` のスプレッドで `Option<CORSOptions>` → `CORSOptions` のフィールド展開 |
| 依存 | 施策 B と同じ基盤 |
| 関連 TODO | I-112c 追加改善（Optional unwrap） |

### 施策 E: TypeResolver の `this` 型解決

| 項目 | 値 |
|------|-----|
| 対象カテゴリ | OBJECT_LITERAL_NO_TYPE の一部（this.field 経由） |
| 解消見込み | **3-5 件**（直接的。間接的にはさらに多い可能性） |
| 残エラー予測 | 66 → 61-63 |
| 技術概要 | クラスメソッド内の `this` 式を `RustType::Named { name: class_name }` として解決。`this.field` / `this.method()` の型情報が利用可能になる |
| 依存 | なし（独立して実施可能） |
| 関連 TODO | I-224 |

### 施策 F: intersection 型の改善

| 項目 | 値 |
|------|-----|
| 対象カテゴリ | INTERSECTION_TYPE |
| 解消見込み | **9 件** |
| 残エラー予測 | 61-63 → 52-54 |
| 技術概要 | `try_convert_intersection_type` で処理できない型パターンへの対応。ジェネリクス intersection（I-101）を含む |
| 依存 | ジェネリクス基盤（構築済み） |
| 関連 TODO | I-221, I-101 |

### 施策 G: 型エイリアス（conditional/mapped/infer）

| 項目 | 値 |
|------|-----|
| 対象カテゴリ | TYPE_ALIAS_COND_TYPE, TYPE_ALIAS_MAPPED_TYPE, TYPE_ALIAS_INFER |
| 解消見込み | **13 件** |
| 残エラー予測 | 52-54 → 39-41 |
| 技術概要 | conditional type (`T extends U ? X : Y`), mapped type (`{ [K in keyof T]: V }`), infer type の変換対応 |
| 依存 | なし |
| 関連 TODO | I-219, I-200 |

### 施策 H: 小規模修正のバッチ（各 1-4 件）

| 対象 | 件数 | 技術概要 |
|------|------|----------|
| INDEXED_ACCESS | 6 | indexed access key の非文字列キー対応 |
| FN_TYPE_PARAM | 4 | 関数型パラメータパターン |
| QUALIFIED_TYPE | 3 | `A.B` 形式の qualified type name |
| ARROW_DEFAULT_PARAM | 3 | arrow 関数のデフォルトパラメータ |
| TYPEOF_TYPE | 3 | `typeof fetch` 等の TsTypeQuery |
| ASSIGN_TARGET | 3 | 代入ターゲットパターン |
| その他（各 1-2 件） | 8 | tagged template, class expr, etc. |
| **小計** | **30** | — |

## 優先順序と累積効果

施策を効果/工数比と依存関係で並べた推奨実行順序:

| 順序 | 施策 | 解消件数 | 累積残エラー | 工数感 |
|------|------|----------|-------------|--------|
| 1 | **E: `this` 型解決** | 3-5 | 109-111 | 小 |
| 2 | **A: コンストラクタ引数 expected type 修正** | 10-20 | 89-101 | 中 |
| 3 | **B: ジェネリクスフィールド展開** | ~14 | 75-77 | 中 |
| 4 | **D: Optional スプレッド unwrap** | 4 | 71-73 | 小 |
| 5 | **C: return_new 型逆引き** | ~10 | 61-63 | 中（A の拡張） |
| 6 | **F: intersection 改善** | 9 | 52-54 | 中 |
| 7 | **G: 型エイリアス** | 13 | 39-41 | 大 |
| 8 | **H: 小規模修正バッチ** | ~30 | 9-11 | 中（個々は小） |

### マイルストーン予測

| 段階 | 残エラー | クリーン率 | 必要施策 |
|------|----------|-----------|----------|
| 現在 | 114 | 58.9% | — |
| 施策 E+A 完了後 | ~89 | ~65% | `this` 型解決 + 関数引数 expected type |
| 施策 B+D 完了後 | ~71 | ~70% | ジェネリクス展開 + Optional unwrap |
| 施策 C+F 完了後 | ~52 | ~75% | return_new + intersection |
| 施策 G 完了後 | ~39 | ~80% | 型エイリアス |
| 施策 H 完了後 | ~9 | ~95% | 小規模修正バッチ |

## カテゴリ別詳細ロケーション

### OBJECT_LITERAL_NO_TYPE (52 instances)

#### function_arg (20 件)
- `client/client.ts:15` — createProxy callback
- `client/utils.ts:66` — deepMerge spread
- `helper/cookie/index.ts:27,50,104` — getCookie/getSignedCookie/generateSignedCookie
- `middleware/basic-auth/index.ts:80` — basicAuth options
- `middleware/bearer-auth/index.ts:103` — bearerAuth options
- `middleware/body-limit/index.ts:57` — bodyLimit options
- `middleware/ip-restriction/index.ts:128` — ipRestriction options
- `middleware/jsx-renderer/index.ts:33` — createRenderer
- `middleware/jwk/jwk.ts:48,170` — jwk options/unauthorizedResponse
- `middleware/jwt/jwt.ts:53,160` — jwt options/unauthorizedResponse
- `middleware/language/language.ts:74` — parseAcceptLanguage
- `middleware/method-override/index.ts:60` — methodOverride options
- `utils/body.ts:121` — parseFormData
- `utils/html.ts:129` — resolveCallbackSync
- `utils/jwt/jwt.ts:56,191` — sign/verifyWithJwks

#### return_new (14 件)
- `adapter/aws-lambda/handler.ts:268,429,496,578` — EventProcessor subclasses
- `adapter/lambda-edge/handler.ts:152` — createRequest
- `helper/css/index.ts:61` — createCssContext
- `helper/ssg/plugins.ts:55` — redirectPlugin
- `helper/streaming/sse.ts:13` — SSEStreamingApi
- `helper/websocket/index.ts:95` — createWSMessageEvent
- `http-exception.ts:46` — HTTPException
- `router/reg-exp-router/prepared-router.ts:9` — PreparedRegExpRouter
- `router/trie-router/node.ts:25` — Node
- `utils/buffer.ts:106` — bufferToFormData
- `utils/stream.ts:6` — StreamingApi

#### generic_param (14 件)
- `adapter/bun/serve-static.ts:8`, `adapter/deno/serve-static.ts:8` — serveStatic `<E extends Env>`
- `adapter/cloudflare-pages/handler.ts:32,49` — handle/handleMiddleware
- `adapter/lambda-edge/handler.ts:116` — handle
- `adapter/service-worker/handler.ts:18` — handle
- `helper/adapter/index.ts:10` — env function
- `helper/dev/index.ts:27,39` — inspectRoutes/showRoutes
- `helper/ssg/utils.ts:61` — filterStaticGenerateRoutes
- `hono.ts:16`, `preset/quick.ts:13` — Hono class
- `request.ts:36` — HonoRequest class
- `validator/validator.ts:46` — validator function

#### optional_spread (4 件)
- `helper/proxy/index.ts:160` — proxy proxyInit
- `middleware/cors/index.ts:63` — cors CORSOptions
- `middleware/csrf/index.ts:94` — csrf CSRFOptions
- `middleware/etag/index.ts:79` — etag ETagOptions

### INTERSECTION_TYPE (9 instances)
- `adapter/aws-lambda/handler.ts:108`
- `client/types.ts:67`
- `context.ts:293`
- `helper/conninfo/types.ts:5`
- `middleware/method-override/index.ts:11`
- `request.ts:30`
- `utils/body.ts:12`
- `utils/types.ts:89`
- `validator/utils.ts:23`

### INDEXED_ACCESS (6 instances)
- `context.ts:95,105`
- `middleware/compress/index.ts:12`
- `types.ts:1457`
- `utils/html.ts:11,142`

### TYPE_ALIAS (13 instances)
- conditional (5): `client/types.ts:28,338`, `middleware/csrf/index.ts:13`, `utils/jwt/jws.ts:13`, `utils/mime.ts:35`
- mapped (5): `client/types.ts:352,371`, `types.ts:2273,2451`, `utils/types.ts:37`
- infer (3): `middleware/language/language.ts:200`, `utils/types.ts:100`, `validator/utils.ts:25`
