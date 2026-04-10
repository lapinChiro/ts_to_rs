# Phase D Step 0: Probe 再計測レポート (2026-04-10)

Phase C (I-387) 完了後、Phase D 着手前の dangling refs 実測結果。

---

## 計測方法

`generate_stub_structs` (shared_types.rs 用 stub) と `resolve_external_types_globally`
(外部型 stub) に `[I-382-PROBE]` eprintln! を注入し、Hono 158 fixture に対して
`./target/release/ts_to_rs --report-unsupported` を実行。

Raw ログ: [`probe-phase-d.log`](./probe-phase-d.log)

---

## 結果サマリ

### Phase A (2026-04-07) → Phase D (2026-04-10) 比較

| Category | Phase A | Phase D | Delta | 備考 |
|---|---|---|---|---|
| dangling (shared_types stubs) | 34 | **24** | -10 | Cluster 1a の 10/11 件解消 |
| excluded_user (defined_elsewhere) | 73 | **72** | -1 | |
| external_dangling (外部型 stubs) | N/A | **79** | (新計測) | `resolve_external_types_globally` 経由 |

### Dangling 24 件の内訳

| Cluster | 件数 | 識別子 | Phase D 対応 |
|---|---|---|---|
| **1a (type param leak)** | **1** | `P` | **要調査**: Phase C で 11→1 に減少したが P が残存 |
| 1b (DOM/Web API) | 20 | HTMLCanvasElement, HTMLImageElement, HTMLVideoElement, SVGImageElement, ImageBitmap, ImageBitmapRenderingContext, VideoFrame, AudioData, BufferSource, CanvasGradient, CanvasPattern, MediaSourceHandle, RTCDataChannel, ServiceWorker, WebGL2RenderingContext, WebGLRenderingContext, Window, HeadersInit, RequestInfo, TemplateStringsArray | PRD-β |
| 1c (compiler marker) | 1 | `__type` | PRD-γ |
| 1c (primitive) | 1 | `symbol` | PRD-β に統合 |
| User-defined (Struct) | 1 | `HTTPException` | PRD-δ (I-382 本体) |
| **合計** | **24** | | |

### Excluded User 72 件

Phase A の 73 件から 1 件減少。全件リスト:

ALBRequestContext, AfterGenerateHook, AfterResponseHook, Algorithm,
ApiGatewayRequestContext, ApiGatewayRequestContextV2, BeforeRequestHook, Bindings,
BodyDataValueComponent, BodyDataValueDot, BodyDataValueDotAll, BodyDataValueObject,
BuildSearchParamsFn, CacheType, CloudFrontRequest, CloudFrontResult, Condition,
ContentSecurityPolicyOptionHandler, ContentfulStatusCode, Context, CssClassName,
CssVariableAsyncType, CssVariableBasicType, CustomHeader, Data, DetectorType,
ExecutionContext, ExtractValidatorOutput, FetchEventLike, H, HTTPExceptionFunction,
HTTPResponseError, HonoJsonWebKey, HonoRequest, HtmlEscapedString, Input,
InvalidJSONValue, IsAllowedOriginHandler, IsAllowedSecFetchSiteHandler, JSONArray,
JSONObject, JSONPrimitive, JSONValue, JWTPayload, KeyAlgorithm, LatticeRequestContextV2,
MergeSchemaPath, MessageFunction, MiddlewareHandler, MountReplaceRequest, ParamIndexMap,
ParamKey, ParamKeys, PermissionsPolicyValue, ProxyRequestInit, RendererOptions,
RequestHeader, Response, ResponseOrInit, SSGParams, SecFetchSite, SecureHeadersCallback,
SignatureAlgorithm, StatusCode, ToEndpoints, TokenHeader, TypedResponse, Variables,
VerifyOptionsWithAlg, WSContext, WSEvents, WSMessageReceive

### External Dangling 79 件 (新計測)

`resolve_external_types_globally` が処理する外部型。`is_external=true` で
`TypeRegistry` に登録済みだが、synthetic/user items から参照された時点で初めて
struct 生成される。Phase D の直接スコープ外だが参考値として記録。

iter=0: AbortSignal, AddEventListenerOptions, AesCbcParams, AesCtrParams,
AesDerivedKeyParams, AesGcmParams, AesKeyAlgorithm, AesKeyGenParams, Array,
ArrayBuffer, ArrayBufferView, ArrayLike, Blob, CloseEvent, ConcatArray, CryptoKey,
CryptoKeyPair, DOMPointInit, DataView, Date, EcKeyGenParams, EcKeyImportParams,
EcdhKeyDeriveParams, EcdsaParams, Error, Event, EventListenerObject,
EventListenerOptions, File, FormData, Headers, HkdfParams, HmacImportParams,
HmacKeyGenParams, IteratorReturnResult, IteratorYieldResult, JSON, JsonWebKey,
Locale, Map, MessageEvent, MessageEventInit, MessagePort, Number, Object,
OffscreenCanvas, OffscreenCanvasRenderingContext2D, Path2D, Pbkdf2Params, Promise,
PromiseLike, ReadableByteStreamController, ReadableStream, ReadableStreamBYOBReader,
ReadableStreamDefaultController, ReadableStreamDefaultReader,
ReadableStreamReadDoneResult, ReadableStreamReadValueResult, RegExp, RegExpMatchArray,
Request, RequestInit, RsaHashedImportParams, RsaHashedKeyGenParams, RsaOaepParams,
RsaOtherPrimesInfo, RsaPssParams, SharedArrayBuffer, String, TextDecoder, TextEncoder,
TransformStream, URL, URLSearchParams, Uint8Array, WebSocket, WritableStream (77 件)

iter=1 (推移依存): EventTarget, ReadableStreamBYOBRequest (2 件)

---

## 重要な発見

### `P` の残存 (Cluster 1a regression) — ✅ 解消済 (2026-04-10)

Phase C (I-387) で Cluster 1a 11 件中 10 件は解消されたが、`P` が 1 件残存していた。

- **Root cause**: `registry/collection.rs::collect_type_alias_fields` に
  `push_type_param_scope` が欠落。TypeCollector (Pass 2) の変換経路が
  TypeConverter (Pass 4) とは独立しており、scope 管理が同期されていなかった。
  `ValidationTargets<T, P>` の `param: Record<P, ...>` で `P` が `Named` として
  registry に格納 → `unique_field_types()` → synthetic union に伝播 → dangling ref
- **修正**: `collect_type_alias_fields` に scope push/restore を追加
- **検証**: dangling 24→**23** (`P` 解消)、test 2259 pass、Hono regression 0
- **副次発見**: TypeCollector/TypeConverter wrapper 層の乖離 3 件 → I-388 (Phase D 後)

### excluded_user 73 → 72 の差分

1 件減少の原因は未特定。Phase C の Named 構造化により一部の参照パターンが変化
した可能性。PRD-δ の spec に影響するため、差分の特定が望ましい。

---

## Phase D 計画への含意 (D-2 完了後に更新)

1. ~~`P` 残存の調査~~ ✅ D-0.5 で解消
2. ~~PRD-γ scope: `__type` 1 件~~ ✅ D-1 (I-389) で解消
3. ~~PRD-β scope: 21 件~~ ✅ D-2 (I-391) で解消
4. **PRD-δ scope**: excluded_user **71** 件 + HTTPException 1 件の対応が必要 (残存)
5. **現在の dangling**: **1 件** (D-0: 24 → D-0.5: 23 → D-1: 22 → D-2: **1**)
