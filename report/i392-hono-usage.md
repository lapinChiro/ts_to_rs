# I-392 Hono 内 Callable Interface 使用調査 (P0.0 Step 8)

## 定義されている Callable Interface (4 件)

### 1. GetCookie (3 overloads, divergent return)

```typescript
// src/helper/cookie/index.ts:10-14
interface GetCookie {
  (c: Context, key: string): string | undefined
  (c: Context): Cookie
  (c: Context, key: string, prefixOptions?: CookiePrefixOptions): string | undefined
}
```

- **Return divergence**: overload 2 は `Cookie` を返す (他は `string | undefined`)
- **Optional params**: overload 3 に `prefixOptions?`
- **Usage**: `export const getCookie: GetCookie = ...` (19 call sites)

### 2. GetSignedCookie (3 overloads, divergent return, async)

```typescript
// src/helper/cookie/index.ts:16-26
interface GetSignedCookie {
  (c: Context, secret: string | BufferSource, key: string): Promise<string | undefined | false>
  (c: Context, secret: string | BufferSource): Promise<SignedCookie>
  (c: Context, secret: string | BufferSource, key: string, prefixOptions?: CookiePrefixOptions): Promise<string | undefined | false>
}
```

- **Async**: 全 overload が `Promise<T>` を返す
- **Return divergence**: overload 2 は `Promise<SignedCookie>` (他は `Promise<string | undefined | false>`)
- **Union types**: `string | BufferSource`, `string | undefined | false`
- **Usage**: `export const getSignedCookie: GetSignedCookie = async (...)` (8 call sites)

### 3. SetHeaders (3 overloads, same return, string literal type)

```typescript
// src/context.ts:258-262
interface SetHeaders {
  (name: 'Content-Type', value?: BaseMime, options?: SetHeadersOptions): void
  (name: ResponseHeader, value?: string, options?: SetHeadersOptions): void
  (name: string, value?: string, options?: SetHeadersOptions): void
}
```

- **Same return**: 全 overload が `void`
- **String literal type**: overload 1 は `'Content-Type'` literal
- **Optional params**: `value?`, `options?`
- **Usage**: `header: SetHeaders = (name, value, options): void => {...}` (1 usage site, class member)

### 4. SetMetric (2 overloads, same return)

```typescript
// src/middleware/timing/timing.ts:126-129
interface SetMetric {
  (c: Context, name: string, value: number, description?: string, precision?: number): void
  (c: Context, name: string, description?: string): void
}
```

- **Same return**: 全 overload が `void`
- **Arity divergence**: overload 1 は 5 params, overload 2 は 3 params
- **Usage**: `export const setMetric: SetMetric = (...)` (8 call sites)

## 分類サマリ

| Interface | Overloads | Return type | Async | Call sites |
|---|---|---|---|---|
| GetCookie | 3 | divergent (Cookie vs string\|undefined) | No | 19 |
| GetSignedCookie | 3 | divergent (SignedCookie vs string\|undefined\|false) | Yes | 8 |
| SetHeaders | 3 | same (void) | No | 1 |
| SetMetric | 2 | same (void) | No | 8 |

**合計 call sites**: 36

## trait 化の影響

- **Single overload**: Hono 内に GetValue 等の single overload callable interface は **存在しない**
  (GetValue は本プロジェクトの test fixture 由来)
- **Multi overload**: 4 件全てが multi overload (2-3 overloads)
- **Async**: GetSignedCookie のみ async
- **Return divergence**: GetCookie と GetSignedCookie の 2 件
- **Same return**: SetHeaders と SetMetric の 2 件 (void)

## Design 影響

1. **void-only multi-overload** (SetHeaders, SetMetric) は P9.4 の Stage 2 fix 対象
2. **Promise<T> unwrap** (GetSignedCookie) は P4.2 で対応
3. **String literal type** (SetHeaders の `'Content-Type'`) は Rust では `&str` に変換。
   overload resolution で literal 特殊化は不要 (widest が `String` に吸収)
