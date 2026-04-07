# T0.4: user 定義型参照の網羅マッピング

## 計測結果

probe で `excluded_user` (= `defined_elsewhere_names` exclusion でガードされた user 定義型) を集計した結果、**ユニーク 73 種類** の user 定義型が anonymous synthetic から参照されている。

(probe-raw.log: `grep "excluded_user" | sort -u | wc -l` = 73)

## 全 user 定義型一覧

```
ALBRequestContext, AfterGenerateHook, AfterResponseHook, Algorithm,
ApiGatewayRequestContext, ApiGatewayRequestContextV2, BeforeRequestHook,
Bindings, BodyDataValueComponent, BodyDataValueDot, BodyDataValueDotAll,
BodyDataValueObject, BuildSearchParamsFn, CacheType, CloudFrontRequest,
CloudFrontResult, Condition, ContentSecurityPolicyOptionHandler,
ContentfulStatusCode, Context, CssClassName, CssVariableAsyncType,
CssVariableBasicType, CustomHeader, Data, DetectorType, ExecutionContext,
ExtractValidatorOutput, FetchEventLike, H, HTTPExceptionFunction,
HTTPResponseError, HonoJsonWebKey, HonoRequest, HtmlEscapedString, Input,
InvalidJSONValue, IsAllowedOriginHandler, IsAllowedSecFetchSiteHandler,
JSONArray, JSONObject, JSONPrimitive, JSONValue, JWTPayload, KeyAlgorithm,
LatticeRequestContextV2, MergeSchemaPath, MessageFunction,
MiddlewareHandler, MountReplaceRequest, ParamIndexMap, ParamKey, ParamKeys,
PermissionsPolicyValue, ProxyRequestInit, RendererOptions, RequestHeader,
Response, ResponseOrInit, SSGParams, SecFetchSite, SecureHeadersCallback,
SignatureAlgorithm, StatusCode, ToEndpoints, TokenHeader, TypedResponse,
Variables, VerifyOptionsWithAlg, WSContext, WSEvents, WSMessageReceive
```

## 重要な観察

### 観察 1: `H` の正体 (調査済)

`/tmp/hono-src/src/types.ts:90`:
```ts
export type H<E extends Env = any, P extends string = any, I extends Input = BlankInput, R extends HandlerResponse<any> = any>
  = Handler<E, P, I, R> | MiddlewareHandler<E, P, I, R>
```

`H` は **user 定義 generic type alias**。型パラメータ leak ではなく PRD-δ スコープに正しく属する。ただし `H` は generic で、参照側 anonymous synthetic に焼き込む際に型引数を保持できているか PRD-δ 設計時に確認必要。

### 観察 2: `Response` の存在 — Hono が独自定義?

通常 `Response` は Web API builtin だが、73 件に含まれている = Hono が `Response` という同名の type/class を独自定義している。`defined_elsewhere_names` で正しく検出されている。

### 観察 3: 全件 anonymous synthetic からの参照

`H` を除く 72 件は、TypeResolver の anonymous union/intersection/object literal の field/variant 型として焼き込まれている。例:
- `MergeSchemaPath` → `MergeSchemaPathOrS` (referencer は anonymous union)
- `HTTPExceptionFunction` → `HTTPExceptionOrHTTPExceptionFunction`
- `ResponseOrInit` → user 定義 type alias、別の synthetic 内で参照
- `WSContext`, `WSEvents` → `TupleContextWSEventsTUOrTupleFnWSEventsTU` (T と U と一緒に)

つまり **PRD-δ (= I-382 本体) のスコープは「anonymous synthetic が user 定義型を field/variant として参照する場合の import 生成」**。

## ModuleGraph 解決可能性 (推定)

`ModuleGraph::module_path(file)` API は user 定義型の所属モジュールを Rust path に解決できる。

ただし注意点:
- 73 件のうち、TypeRegistry に `TypeDef::Struct` / `TypeDef::Enum` として登録されている型は、定義 file が一意に決まるはず
- type alias (`type Foo = ...`) は `TypeDef::TypeAlias` (現状の TypeRegistry に該当 variant あり?) として登録されているか確認必要
- 同名の型が複数 file に存在する場合の disambiguation 戦略 (Hono 内では稀だが要確認)

**追加調査タスク**: `TypeRegistry` の各 typedef に「定義された source file path」が記録されているか確認。記録されていない場合、定義 file 追跡を追加する必要 — これが PRD-δ の主要設計事項。

## 配置パターンと必要 import

I-382 本体の責務は anonymous synthetic の placement (inline / shared) に応じた import 生成:

### Pattern A: anonymous synthetic が **shared 配置** + user 型参照

- shared_types.rs に `use crate::<user-type-module>::<TypeName>;` を生成
- 例: `MergeSchemaPathOrS` が複数 file から参照される → shared 配置 → shared_types.rs に `use crate::types::MergeSchemaPath;`

### Pattern B: anonymous synthetic が **inline 配置** + user 型参照 (定義 file と同じ)

- 配置先 file が user 型の定義 file と同じ → import 不要 (同一モジュール)
- 例: `HTTPExceptionOrHTTPExceptionFunction` が `http-exception.ts` のみから参照される場合

### Pattern C: anonymous synthetic が **inline 配置** + user 型参照 (定義 file と異なる)

- 配置先 file に `use crate::<user-type-module>::<TypeName>;` を生成
- 既存 user import との衝突回避が必要

### Pattern D: 推移参照 (synthetic → synthetic → user)

- shared synthetic A が shared synthetic B を参照、B が user 型 X を参照
- shared_types.rs 内なので import 1 つで両方解決

## PRD-δ スコープ確定事項

- 対象: 上記 72〜73 件 (`H` の確認次第)
- 完了条件:
  - `generate_stub_structs` 関数が存在しない (grep ヒット 0)
  - `defined_elsewhere_names` 引数が存在しない
  - 全 user 定義型参照が import 経由で解決される
  - probe instrumentation 投入時に `excluded_user` ログ 0 件 (= excluded ロジック自体が不要)
  - regression 検出 panic が Pass 5c に存在し、dangling ref があれば即 panic
- 手段:
  - `OutputWriter::resolve_synthetic_placement` 拡張で synthetic → user type 参照を検出
  - `ModuleGraph::module_path` で各 user 型を解決
  - inline / shared 各 placement に対応する import 生成
  - Pass 5c の `generate_stub_structs` 削除
- 影響範囲:
  - `pipeline/external_struct_generator/mod.rs` (削除)
  - `pipeline/output_writer/placement.rs` (拡張)
  - `pipeline/mod.rs::collect_user_defined_type_names` (削除/再構成)
  - `tests/undefined_refs_tests.rs` (再構成)

## 残る検証 (PRD-δ Discovery 時)

- `H` が user 定義型なのか型パラメータ leak なのか (Hono ソース grep + TypeRegistry inspection)
- TypeRegistry に各 typedef の source file 情報が記録されているか
- 同名衝突時の disambiguation
- inline 配置時に user file の既存 import との重複検出
