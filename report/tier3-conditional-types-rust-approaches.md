# Tier 3 条件型パターンの Rust アプローチ分析

**基準コミット**: `1f571a1`
**分析対象**: `/tmp/hono/src/types.ts`, `/tmp/hono/src/utils/types.ts`, `/tmp/hono/src/client/types.ts`, `/tmp/hono/src/context.ts`

---

## Group A: テンプレートリテラル型による文字列分解

### 対象型

| 型名 | 定義箇所 | 概要 |
|------|----------|------|
| `MergePath<A, B>` | types.ts:2320 | `/api/` + `/users` → `/api/users` のようにパス文字列を正規化して結合 |
| `ParamKey<Component>` | types.ts:2408 | `:id{pattern}?` のようなルートパラメータ記法からパラメータ名を抽出 |
| `ParamKeys<Path>` | types.ts:2416 | パス全体から全パラメータ名をユニオン型として抽出（再帰的に `/` で分割） |
| `ParamKeyToRecord<T>` | types.ts:2420 | パラメータ名 `"id"` → `{ id: string }`, `"id?"` → `{ id?: string }` に変換 |
| `ExtractParams<Path>` | types.ts:2263 | パス文字列からパラメータの Record 型を生成（例: `/users/:id/posts/:postId` → `{ id: string, postId: string }`） |
| `RemoveQuestion<T>` | types.ts:2439 | 末尾の `?` を除去 |
| `AddParam<I, P>` | types.ts:2311 | 入力型にパスパラメータ情報を合成 |
| `TrimStartSlash<T>` | client/types.ts:191 | 先頭の `/` を再帰的に除去 |
| `ParseHostName<T>` | client/types.ts:188 | `host:port` を `[host, port]` タプルに分解 |
| `ApplyParam<Path, P>` | client/types.ts:194 | パラメータ値をパスに埋め込み、リテラル型のURLを生成 |

### 目的（これらが解決する問題）

**ルート定義の文字列からパラメータの型を自動導出し、ハンドラ関数のシグネチャに反映する。** ユーザーが `app.get('/users/:id', handler)` と書いたとき、`handler` の引数 `c.req.param('id')` が `string` として型付けされ、`c.req.param('name')` はコンパイルエラーになる。

副次的な目的として、サブアプリのマウント時にベースパスとサブパスの結合を正規化する（重複スラッシュの除去等）。

### Rust Web フレームワークのアプローチ

**Axum の方式: 型抽出子（Extractor）パターン**

Axum はルート文字列からの型導出を行わない。代わりに、ハンドラ関数の引数の型からパラメータを抽出する:

```rust
// Axum: パス文字列 "/users/:id" と Path<(u32,)> は独立
async fn handler(Path(id): Path<u32>) -> String { ... }
app.route("/users/:id", get(handler));
```

- パス文字列とハンドラの引数型の整合性はコンパイル時に検証されない（パラメータ数の不一致は実行時エラー）
- パラメータの型は `Path<T>` の `T` で決まる（`String` でも `u32` でも可）
- `T` には `Deserialize` を実装した任意の型が使える

**Actix-web の方式: ほぼ同様**

```rust
async fn handler(path: web::Path<(u32,)>) -> String { ... }
```

**Poem の方式: `#[handler]` マクロ**

```rust
#[handler]
fn handler(Path(id): Path<u32>) -> String { ... }
```

proc macro でシグネチャを書き換えるが、パス文字列との整合性チェックは行わない。

### Rust での実現アプローチ

| アプローチ | 説明 | 評価 |
|-----------|------|------|
| **Extractor パターン（Axum 方式）** | ハンドラの引数型でパラメータ型を宣言。パス文字列との整合性は検証しない | 実績あり。Hono のような自動導出は諦めるが、実用上問題ない |
| **Proc macro によるパス解析** | `#[route("/users/:id")]` マクロがパス文字列をコンパイル時にパースし、パラメータ名・型を含む構造体を生成 | 技術的に可能。パス文字列は `&str` リテラルなので macro 内で完全にパース可能 |
| **`const fn` 文字列処理** | パスの結合・正規化は `const fn` で実行時ゼロコストで実現可能 | `MergePath` のようなパス結合には適するが、型生成には使えない |
| **実行時処理** | パラメータ抽出はリクエスト処理時に行う。型安全性は Extractor で担保 | 全 Rust Web フレームワークが採用している方式 |

**結論**: Hono の「パス文字列 → パラメータ型の自動導出」は、Rust では **proc macro で近いものが実現可能** だが、主要フレームワークは全て **Extractor パターンで別の方向から同じ問題を解決** している。Extractor パターンの方が Rust のエコシステムに馴染む（Deserialize トレイトとの統合、型の柔軟性）。

---

## Group B: 再帰的条件型

### 対象型

| 型名 | 定義箇所 | 概要 |
|------|----------|------|
| `SimplifyDeepArray<T>` | utils/types.ts:94 | 配列要素を再帰的に Simplify する（IDE のホバー表示改善） |
| `IntersectNonAnyTypes<T>` | types.ts:2473 | 型の配列 `[E1, E2, E3]` を `E1 & E2 & E3` に変換。ただし `any` 型は `{}` に置換 |
| `PathToChain<Prefix, Path, E>` | client/types.ts:294 | URL パス `/users/posts` をネストしたオブジェクト型 `{ users: { posts: ClientRequest } }` に変換 |
| `InterfaceToType<T>` | utils/types.ts:98 | interface を再帰的に type に変換（構造的等価性の問題回避） |

### 目的

**SimplifyDeepArray / InterfaceToType**: TypeScript の型表示の問題を回避するためのユーティリティ。IDE が `Foo & Bar & Baz` のような交差型を展開せずそのまま表示する問題に対処する。**Rust ではこの問題自体が存在しない**（Rust の型推論結果は具体型として解決される）。

**IntersectNonAnyTypes**: ミドルウェアチェーンで各ミドルウェアが宣言する環境型（`E1`, `E2`, ...）を 1 つの統合された環境型にマージする。`any` は「何も宣言していない」を意味し、マージ時に無視する。

**PathToChain**: hono/client の型安全な RPC クライアントのため。`client.users.posts.$get()` のようなチェーン呼び出しを可能にする。

### Rust Web フレームワークのアプローチ

**Axum の方式: タプル型のネスト + trait impl**

Axum のミドルウェア（Layer/Service）は Tower のサービスモデルに基づく。型の合成は trait の associated type で行う:

```rust
// Tower の Service トレイト
trait Service<Request> {
    type Response;
    type Error;
    fn call(&self, req: Request) -> Self::Future;
}
```

ミドルウェアのチェーンは `Service<Service<Service<...>>>` のネストで表現される。各レイヤーの型は具体的に解決される。

Axum の State 共有（Hono の `Env` に相当）はジェネリック `S` パラメータで単一の型として表現する。複数のミドルウェアが異なる State を要求する場合、`FromRef` トレイトで部分的な State 取得を型安全に実現する:

```rust
#[derive(Clone, FromRef)]
struct AppState {
    db: DatabasePool,
    cache: CacheClient,
}
```

**RPC クライアント型（PathToChain 相当）**: Rust の Web フレームワークにはこのパターンの直接的な対応物がない。型安全な API クライアント生成は通常 OpenAPI スキーマからの **コード生成** で実現する（例: `progenitor`, `openapi-generator`）。

### Rust での実現アプローチ

| パターン | 目的 | Rust アプローチ |
|----------|------|----------------|
| SimplifyDeepArray / InterfaceToType | IDE 表示改善 | **不要**。Rust には対応する問題がない |
| IntersectNonAnyTypes | 複数の環境型のマージ | **`FromRef` トレイト**（Axum 方式）。または proc macro で State 構造体を合成。再帰的 trait bound `T: Trait1 + Trait2 + ...` でも表現可能 |
| PathToChain | パスからネストしたクライアント型を生成 | **Proc macro によるコード生成**。サーバー側のルート定義からクライアント構造体を生成する。または OpenAPI → コード生成 |

**結論**: Group B の再帰型のうち半分（SimplifyDeepArray, InterfaceToType）は TypeScript 固有の問題への対処であり Rust では不要。IntersectNonAnyTypes は Axum の `FromRef` パターンが洗練された代替。PathToChain はコード生成が自然な解法。

---

## Group C: 高階型操作ユーティリティ

### 対象型

| 型名 | 定義箇所 | 概要 |
|------|----------|------|
| `Equal<X, Y>` | utils/types.ts:8 | 2つの型が完全に等しいか判定 |
| `NotEqual<X, Y>` | utils/types.ts:10 | Equal の否定 |
| `UnionToIntersection<U>` | utils/types.ts:12 | ユニオン型 `A \| B \| C` を交差型 `A & B & C` に変換 |

### 目的

**Equal / NotEqual**: テストユーティリティ。型レベルのアサーション `Expect<Equal<Result, Expected>>` に使用。プロダクションの型ロジックではなく、型テストのインフラ。

**UnionToIntersection**: 複数のハンドラ/ミドルウェアが返す型（ユニオン）を1つの統合された型（交差）にまとめる。例えば `ExtractSchema<T>` で複数のルート定義からスキーマ全体を生成する際に使用（types.ts:2447）。

### Rust での実現アプローチ

| パターン | Rust アプローチ |
|----------|----------------|
| Equal / NotEqual | `static_assertions` クレートの `assert_type_eq!` マクロ。または `std::any::TypeId` を使ったコンパイル時チェック。proc macro で `compile_fail` テストも可能 |
| UnionToIntersection | **Rust では問題の構造が異なる**。Rust にユニオン型はない（enum はタグ付きユニオン）。Hono が UnionToIntersection で解決している「複数の型を1つにマージ」は、Rust では **trait bound の合成**（`T: Trait1 + Trait2`）や **構造体の合成**（フィールドの追加）で行う |

**結論**: Equal/NotEqual はテストインフラであり `static_assertions` で代替。UnionToIntersection は Rust の型システムでは問題の立て方自体が異なるため、直接の対応物は不要。

---

## Group D: 複雑なネスト条件型（3階層以上）

### 対象型

| 型名 | 定義箇所 | 概要 |
|------|----------|------|
| `ToSchema<M, P, I, RorO>` | types.ts:2210 | HTTP メソッド・パス・入力・レスポンスからルートスキーマ型を構築。`any` の場合のフォールバック付き |
| `ToSchemaOutput<RorO, I>` | types.ts:2193 | レスポンス型から output/outputFormat/status を抽出。TypedResponse の場合とそうでない場合で分岐 |
| `MergeTypedResponse<T>` | types.ts:2359 | `Promise<void>`, `Promise<TypedResponse>`, `TypedResponse`, 生の `Response` を正規化。3階層の条件分岐 |
| `MergeMiddlewareResponse<T>` | types.ts:2372 | ミドルウェア関数の返り値型から TypedResponse だけを抽出。void を除外し、Response/TypedResponse のみ通す |
| `AddSchemaIfHasResponse<Merged, S, M, P, I, BasePath>` | types.ts:2247 | レスポンスが `Promise<void>`（ミドルウェア）ならスキーマ追加しない、そうでなければ追加 |
| `JSONParsed<T>` | utils/types.ts:53 | 任意の型を `JSON.stringify → JSON.parse` 後の型に変換。6階層以上のネスト条件型 |

### 目的

これらは **Hono のルーティングパイプラインの型追跡システム** の中核を担う。具体的に解決する問題は:

1. **ルート定義の蓄積**: `app.get(path, handler)` を呼ぶたびに、返り値の `HonoBase` 型に新しいルート情報が型レベルで追加される（`S & ToSchema<...>`）
2. **ミドルウェアとハンドラの区別**: ミドルウェア（`Promise<void>` を返す）はスキーマに追加しない。ハンドラだけがエンドポイント定義として記録される
3. **レスポンス型の正規化**: `Promise<Response>`, `TypedResponse<T>`, 生の JSON 値など、様々な返り値型を統一フォーマットに変換
4. **JSON シリアライゼーションの型安全性**: `c.json(data)` のレスポンス型が、`data` を `JSON.stringify` した結果の型と一致することを保証

### Rust Web フレームワークのアプローチ

**Axum の方式: IntoResponse トレイト + 型消去**

Axum はレスポンス型の追跡を行わない。全てのハンドラは `IntoResponse` を実装した型を返す:

```rust
trait IntoResponse {
    fn into_response(self) -> Response;
}
```

`IntoResponse` は `String`, `Json<T>`, `(StatusCode, String)`, `Result<T, E>` 等に実装されている。型情報は `into_response()` の時点で消去される。

Hono の ToSchema が行うような「ルート定義全体のスキーマを型レベルで保持する」ことは Axum は行わない。API ドキュメント生成には `utoipa` クレート（proc macro ベース）を使う:

```rust
#[utoipa::path(get, path = "/users/{id}", responses(
    (status = 200, body = User),
    (status = 404, body = Error),
))]
async fn get_user(Path(id): Path<u32>) -> Json<User> { ... }
```

**ミドルウェアの型**: Tower の `Service` トレイトで、各レイヤーの `Response` associated type は具象型。ミドルウェアのネストは型レベルで追跡されるが、Hono のような「全ルートのスキーマを統合した型」にはならない。

### Rust での実現アプローチ

| Hono の機能 | 目的 | Rust アプローチ |
|-------------|------|----------------|
| ルートスキーマの型レベル蓄積 | 型安全な RPC クライアント生成 | **Proc macro によるコード生成**。`#[route]` マクロがルート情報を収集し、スキーマ型・クライアント型を生成する。または **ビルドスクリプト** でルート定義を走査してコード生成 |
| ミドルウェアとハンドラの区別 | スキーマにミドルウェアを含めない | **トレイトの分離**。`Handler` トレイトと `Middleware` トレイトを別に定義し、型レベルで区別。Axum は `Handler` と `Layer` で区別している |
| レスポンス型の正規化 | 統一的なレスポンス型 | **`IntoResponse` トレイト**（Axum 方式）。各具象型が自分のシリアライゼーション方法を実装。enum で返り値型を統合することも可能 |
| JSON シリアライゼーション型安全性 | `c.json()` の型整合性 | **`Serialize` トレイト境界**。`Json<T: Serialize>` で、シリアライズ可能な型のみ受け付ける。`JSONParsed` のような往復変換後の型追跡は不要（Rust の `serde` は型安全にシリアライズ/デシリアライズする） |

**結論**: Group D は Hono の最も複雑な型システムだが、その複雑さの大部分は **TypeScript の構造的型付けとユニオン型の制約を乗り越えるため** に存在する。Rust では:

- **レスポンス型の正規化** → `IntoResponse` トレイト（既に確立されたパターン）
- **JSON 型安全性** → `Serialize` / `Deserialize` トレイト（`serde` が完全に解決済み）
- **ルートスキーマの型レベル蓄積** → Rust の型システムでは直接的に実現困難だが、proc macro + コード生成で同等の機能を実現可能。ただし、**主要フレームワークはこの機能を提供していない**（需要が限定的）

---

## 総合まとめ

### パターン別の変換戦略

| Group | Hono が解決する問題 | Rust での自然な解法 | 「不可能」か？ |
|-------|--------------------|--------------------|--------------|
| A: テンプレートリテラル型 | パス文字列 → パラメータ型の自動導出 | Extractor パターン（問題の回避）/ proc macro（直接実現） | **不要**: Extractor が別の方法で解決 |
| B: 再帰的条件型 | IDE 表示改善、環境型マージ、クライアント型生成 | 半分は不要。残りは `FromRef` / proc macro | **不要〜proc macro で可能** |
| C: 高階型操作 | 型テスト、ユニオン→交差変換 | `static_assertions` / trait bound 合成 | **不要**: 問題構造が異なる |
| D: 複雑ネスト条件型 | ルートスキーマ蓄積、型正規化、JSON 安全性 | `IntoResponse` + `serde` + proc macro | **不要〜proc macro で可能** |

### 核心的な洞察

Hono の Tier 3 型の複雑さは、以下の TypeScript 固有の制約から生じている:

1. **ユニオン型の暗黙的な発生**: TypeScript では関数オーバーロードや型推論でユニオン型が自然発生し、それを交差型に変換する必要がある。Rust ではこの問題が起きない（enum は明示的、trait object は明示的）

2. **構造的型付けの曖昧さ**: `any` や `unknown` の存在により、「この型は何も宣言していない」と「この型は全てを許容する」の区別が必要。Rust の公称型付けでは不要

3. **型レベルメタプログラミングの代替手段がない**: TypeScript にはマクロがないため、条件型で全ての型計算を行う必要がある。Rust では proc macro がこの役割を担う

4. **`serde` の不在**: TypeScript にはシリアライゼーションフレームワークの型安全な抽象がないため、`JSONParsed` のような巨大な型が必要。Rust では `Serialize` / `Deserialize` トレイトが解決済み

**変換方針としての推奨**: Tier 3 の型を Rust の型システムに直訳しようとするのではなく、各型が「解決する問題」を特定し、Rust のエコシステムで確立されたパターン（Extractor, IntoResponse, serde, proc macro）で同じ問題を解決する。
