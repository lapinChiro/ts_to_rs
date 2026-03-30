# C-3 調査: OBJECT_LITERAL_NO_TYPE 全32件の根本原因分析

**Base commit**: ea68769

## 概要

OBJECT_LITERAL_NO_TYPE エラー全32件の個々のオブジェクトリテラルを特定し、TypeResolver コードパスを具体的にトレースして根本原因を分類した。

## エラー発生条件

`convert_object_lit` (`src/transformer/expressions/data_literals.rs`) は `expected` が `Some(RustType::Named { ... })` でない場合にエラーを返す。つまり TypeResolver が `expected_types` マップにオブジェクトリテラルのスパンに対応する `Named` 型を設定できなかった場合に発生する。

## 全32件の分類

### A: 関数/コンストラクタ引数 — 16件

| # | ファイル | ObjLit位置 | 呼び出し先 | 同一ファイル? | 失敗原因 |
|---|---------|-----------|-----------|------------|---------|
| 1 | `adapter/bun/serve-static.ts` | L25 | `baseServeStatic({...})` | No | registry 未登録 |
| 2 | `adapter/deno/serve-static.ts` | L35 | `baseServeStatic({...})` | No | registry 未登録 |
| 3 | `adapter/cloudflare-pages/handler.ts` | L38 | `app.fetch(req, {...})` | No | registry 未登録 |
| 4 | `adapter/cloudflare-pages/handler.ts` | L61 | `new Context(req, {...})` | No | registry 未登録 |
| 5 | `adapter/lambda-edge/handler.ts` | L126 | `app.fetch(req, {...})` | No | registry 未登録 |
| 6 | `adapter/lambda-edge/handler.ts` | L169 | `new Request(url, {...})` | No（外部型） | Request の constructor params 未活用 |
| 7 | `adapter/service-worker/handler.ts` | L30 | `app.fetch(req, {...})` | No | registry 未登録 |
| 8 | `client/client.ts` | L16 | `new Proxy(fn, {...})` | No（標準API） | Proxy 未登録 |
| 9 | `client/client.ts` | L214 | `new ClientRequestImpl(url, m, {...})` | **Yes** | constructor 3rd param が TypeLit |
| 10 | `hono-base.ts` | L415 | `new Context(req, {...})` | No | registry 未登録 |
| 11 | `helper/cookie/index.ts` | L85 | `serialize(name, val, {...})` | No | registry 未登録 |
| 12 | `helper/streaming/sse.ts` | L58 | `stream.writeSSE({...})` | **Yes** | writeSSE の param 型は `SSEMessage`。同ファイル内 |
| 13 | `helper/css/index.ts` | L195 | `createCssContext({id})` | **Yes** | param 型は inline `{id: Readonly<string>}` |
| 14 | `http-exception.ts` | L68 | `new Response(body, {...})` | No（外部型） | Response constructor params 未活用 |
| 15 | `middleware/jsx-renderer/index.ts` | L52 | `jsx(fn, {...})` | No | registry 未登録 |
| 16 | `helper/ssg/utils.ts` | L66 | `acc.push({path})` | — | `.push()` は Vec メソッド |

**同一ファイルで理論上修正可能**: 3件 (#9, #12, #13)
**外部型 constructor**: 2件 (#6, #14) — Request/Response は外部型 JSON で登録済みのはず
**imported/標準API**: 11件 — 単一ファイルモードでは registry に情報なし

### B: 型注釈なしローカル変数 — 7件

| # | ファイル | ObjLit位置 | コード | 備考 |
|---|---------|-----------|--------|------|
| 1 | `adapter/aws-lambda/handler.ts` | L170 | `const httpResponseMetadata = {statusCode, headers, cookies}` | 注釈なし |
| 2 | `helper/proxy/index.ts` | L166 | `{raw: proxyInit}` (三項演算子) | destructuring 代入 |
| 3 | `utils/html.ts` | L135 | `const context = {}` | 空オブジェクト |
| 4 | `utils/concurrent.ts` | L39 | `const marker = {}` | 空オブジェクト（Set マーカー） |
| 5 | `validator/validator.ts` | L90 | `let value = {}` | 空オブジェクト |
| 6 | `middleware/basic-auth/index.ts` | L134 | `const headers = {'WWW-Authenticate': ...}` | 注釈なし |
| 7 | `middleware/ip-restriction/index.ts` | L157 | `const remoteData = {addr, type, isIPv4: ...}` | 注釈なし |

→ I-301 のスコープ。匿名構造体生成 or 後方型推論が必要。

### C: クラスフィールドデフォルト値 — 2件

| # | ファイル | ObjLit位置 | コード | 根本原因 |
|---|---------|-----------|--------|---------|
| 1 | `request.ts` | L69 | `bodyCache: BodyCache = {}` | `BodyCache` は `type BodyCache = Partial<Body>`。TypeAlias の RHS が `TsTypeRef` の場合、`collect_type_alias_fields` が `None` を返す (`collection.rs:456` が `TsTypeLit`/`TsIntersectionType` のみ対応) |
| 2 | `context.ts` | L315 | `env: E['Bindings'] = {}` | `E['Bindings']` → `Named("E::Bindings")` に変換。`resolve_type_params_in_type` は `"E::Bindings"` をキー検索するが、制約マップには `"E"` しかないため未解決 |

### D: デフォルトパラメータ — 2件

| # | ファイル | ObjLit位置 | コード | 根本原因 |
|---|---------|-----------|--------|---------|
| 1 | `adapter/service-worker/handler.ts` | L20 | `opts: HandleOptions = {fetch: ...}` | `HandleOptions` のフィールド `fetch?: typeof fetch` で `typeof fetch` が解決不能 → 空 Struct 登録 |
| 2 | `hono-base.ts` | L126 | `constructor(options: HonoOptions<E> = {})` | `HonoOptions<E>` のジェネリクス。型パラメータ `E` の制約 `Env` から具体化が必要 |

### E: return 文 — 5件

| # | ファイル | ObjLit位置 | コード | 根本原因 |
|---|---------|-----------|--------|---------|
| 1 | `helper/cookie/index.ts` | L43 | `return {}` (getCookie) | `getCookie` は `GetCookie` 型変数に代入。`GetCookie` は callable interface → `TypeDef::Struct` として登録。`resolve_fn_type_info` は `TypeDef::Function` のみ対応 → `current_fn_return_type` 未設定 |
| 2 | `helper/cookie/index.ts` | L71 | `return {}` (getSignedCookie) | 同上パターン |
| 3 | `helper/dev/index.ts` | L30 | `return {path, method, ...}` | `.map()` callback 内。`propagate_expected` が `Expr::Call` を処理しない → callback body に戻り値型が伝播しない |
| 4 | `utils/concurrent.ts` | L23 | `return {run: ...}` | `createPool` の戻り値型 `Pool`。`Pool` が TypeRegistry にあれば動作するが、inline 型の場合失敗 |
| 5 | `utils/body.ts` | L110 | `return {}` | `parseBody` の戻り値型 → overloaded callable |

### F: 代入右辺 — 2件

| # | ファイル | ObjLit位置 | コード | 根本原因 |
|---|---------|-----------|--------|---------|
| 1 | `adapter/aws-lambda/handler.ts` | L488 | `result.multiValueHeaders = {'set-cookie': cookies}` | `result` の型が解決不能、または `multiValueHeaders` フィールドが registry に未登録 |
| 2 | `router/trie-router/node.ts` | L38 | `m[method] = {handler, ...}` | `resolve_member_type` が computed property + HashMap に未対応。`m: Record<string, HandlerSet<T>>` → `HashMap<String, HandlerSet<T>>` だが、`m[key]` の value 型 `HandlerSet<T>` を返せない |

## 根本原因のまとめ（修正可能性別）

### C-3 スコープ内で修正可能

| 原因 | 影響件数 | 修正内容 |
|------|---------|---------|
| **resolve_member_type: PrivateName 未対応** | ~1件 | `MemberProp::PrivateName` のブランチ追加 |
| **resolve_member_type: HashMap computed access 未対応** | 1件 | `Named("HashMap")` + Computed → `type_args[1]` 返却 |
| **resolve_new_expr: type_args 無視** | 数件 | `new_expr.type_args` を返り値・param 解決に反映 |
| **set_call_arg_expected_types: 明示的型引数未活用** | 数件 | `call.type_args` を param 型解決に利用 |
| **型引数推論（I-286c S3）** | ~3件（同ファイル） | 実引数型から型パラメータを unification で推論 |

### 別イシューとして対応すべき

| 原因 | 影響件数 | 対応イシュー |
|------|---------|------------|
| 型注釈なしオブジェクトリテラル | 7件 | I-301 |
| callable interface の return 型解決 | 2件 | **新規**: resolve_fn_type_info の TypeDef::Struct 対応 |
| .map() callback への型伝播 | 1件 | **新規**: propagate_expected の Call 式対応 |
| TypeAlias (Partial<T> 等) の registry 登録 | 1件 | **新規**: collect_type_alias_fields の TsTypeRef 対応 |
| `E['Bindings']` indexed access の型パラメータ解決 | 1件 | **新規**: resolve_type_params_in_type の複合名対応 |
| imported 関数の registry 未登録 | 11件 | マルチファイルモード or directory モード改善 |
| 外部型 constructor params の活用 | 2件 | **新規**: Request/Response constructor expected type |

## C-3 の期待効果

ベンチマーク数値は**多くの場合改善しない**。理由:
- 32件中16件が fn-arg だが、そのうち11件は imported 関数で単一ファイルモードでは解決不可
- 同一ファイル fn-arg 3件 + HashMap computed 1件 + PrivateName 1件 = **最大5件の改善**
- ただし同一関数内に複数の ObjLit エラーが共存する場合、最初の1つが解決しても関数単位のエラー報告は変わらない可能性あり

**重要**: C-3 の価値は数値改善だけでなく、**型引数推論の基盤整備**にある。この基盤は今後の全ての型解決改善の前提となる。
