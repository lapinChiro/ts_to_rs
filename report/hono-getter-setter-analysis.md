# Hono getter/setter 使用パターン調査

調査日: 2026-03-13
対象: `/tmp/hono/src/` 配下の全 `.ts` ファイル（`.test.ts` 除外、186 ファイル）

## 定量サマリ

| 分類 | 数 |
|------|----|
| getter のみ（setter なし） | 14 |
| setter のみ（getter なし） | 0 |
| getter + setter ペア | 1 |
| **合計 getter** | **16**（うち 1 つはシンボルキー） |
| **合計 setter** | **1** |

## 詳細一覧

### 1. getter + setter ペア（1 件）

| ファイル | 行 | 名前 | 概要 |
|----------|----|------|------|
| `context.ts` | L403, L414 | `res` | getter: `Response` を返す（なければ空レスポンスを生成）。setter: 既存ヘッダーをマージして `this.#res` を更新し `finalized = true` にする。setter の引数型は `Response \| undefined` |

### 2. getter のみ（14 件 + シンボルキー 1 件）

#### `src/request.ts`（HonoRequest クラス）

| 行 | 名前 | 戻り値型 | 本体概要 |
|----|------|----------|----------|
| L353 | `url` | `string` | `this.raw.url` をそのまま返す |
| L369 | `method` | `string` | `this.raw.method` をそのまま返す |
| L373 | `[GET_MATCH_RESULT]` | `Result<[unknown, RouterRoute]>` | シンボルキーの getter。`this.#matchResult` を返す |
| L404 | `matchedRoutes` | `RouterRoute[]` | `#matchResult` を map して route を抽出（deprecated） |
| L424 | `routePath` | `string` | `#matchResult` から現在のルートの path を返す（deprecated） |

#### `src/context.ts`（Context クラス）

| 行 | 名前 | 戻り値型 | 本体概要 |
|----|------|----------|----------|
| L366 | `req` | `HonoRequest<P, I['out']>` | `#req` を遅延初期化して返す（lazy initialization） |
| L377 | `event` | `FetchEventLike` | `#executionCtx` に `respondWith` があれば返す、なければ throw |
| L391 | `executionCtx` | `ExecutionContext` | `#executionCtx` があれば返す、なければ throw |
| L593 | `var` | `Readonly<ContextVariableMap & ...>` | `#var` の Map を `Object.fromEntries` で変換して返す（read-only） |

#### `src/helper/websocket/index.ts`（WSContext クラス）

| 行 | 名前 | 戻り値型 | 本体概要 |
|----|------|----------|----------|
| L83 | `readyState` | `WSReadyState` | `this.#init.readyState` を委譲 |

#### `src/router/smart-router/router.ts`（SmartRouter クラス）

| 行 | 名前 | 戻り値型 | 本体概要 |
|----|------|----------|----------|
| L63 | `activeRouter` | `Router<T>` | ルーターが未決定なら throw、決定済みなら `#routers[0]` を返す |

#### `src/jsx/base.ts`（JSXNode クラス）

| 行 | 名前 | 戻り値型 | 本体概要 |
|----|------|----------|----------|
| L143 | `type` | `string \| Function` | `this.tag` を返す（React 互換） |
| L149 | `ref` | `any` | `this.props.ref \|\| null` を返す（React 互換） |

#### `src/adapter/deno/websocket.ts`（オブジェクトリテラル内）

| 行 | 名前 | 戻り値型 | 本体概要 |
|----|------|----------|----------|
| L14 | `protocol` | 型注記なし | `socket.protocol` を委譲 |
| L18 | `readyState` | 型注記なし | `socket.readyState as WSReadyState` を委譲 |

#### `src/adapter/cloudflare-workers/websocket.ts`（オブジェクトリテラル内）

| 行 | 名前 | 戻り値型 | 本体概要 |
|----|------|----------|----------|
| L23 | `protocol` | 型注記なし | `server.protocol` を委譲 |
| L27 | `readyState` | 型注記なし | `server.readyState as WSReadyState` を委譲 |

### 3. setter のみ

該当なし（0 件）。

## getter の戻り値型パターン

| パターン | 件数 | 例 |
|----------|------|----|
| プリミティブ型（`string`） | 4 | `url`, `method`, `routePath`, `type` |
| 独自型 / ジェネリクス型 | 6 | `HonoRequest<P, ...>`, `Router<T>`, `RouterRoute[]`, `Result<...>`, `WSReadyState` |
| インターフェース型 | 2 | `FetchEventLike`, `ExecutionContext` |
| `Readonly<...>` ラッパー | 1 | `var` |
| `any` | 1 | `ref` |
| 型注記なし（オブジェクトリテラル内） | 4 | adapter 内の `protocol`, `readyState` |

**傾向**: クラス定義内の getter は全て戻り値型が注記されている。オブジェクトリテラル内の getter（adapter の WebSocket 初期化パラメータ）は型注記がない。

## setter の引数型パターン

| 名前 | 引数型 | 備考 |
|------|--------|------|
| `res` | `Response \| undefined` | union 型。undefined を許容してリセット可能にしている |

setter は 1 件のみ。

## 使用パターンの分類

getter の用途は以下の 4 パターンに分類できる:

1. **プロパティ委譲**（8 件）: 内部オブジェクトのプロパティをそのまま返す（`url`, `method`, `readyState`, `protocol` 等）
2. **遅延初期化**（2 件）: 初回アクセス時にインスタンスを生成する（`req`, `res` の getter）
3. **変換・加工**（3 件）: 内部データを加工して返す（`matchedRoutes`, `routePath`, `var`）
4. **ガード付きアクセス**（2 件）: 条件を満たさなければ例外を投げる（`event`, `executionCtx`）
5. **互換性エイリアス**（2 件）: 別名でプロパティを公開する（`type` → `tag`, `ref` → `props.ref`）

## 定量まとめ

- 全 186 ファイル中、getter/setter を使用しているのは **6 ファイル**
- getter は主にクラス定義で使用される（14/16 件）
- setter はプロジェクト全体で **1 件のみ**（`Context.res`）
- setter 単体の使用は **0 件**
- オブジェクトリテラル内の getter は WebSocket adapter の初期化パラメータに限定される（4 件）
