# I-221: unsupported intersection member type 調査レポート

**日付**: 2026-03-28
**Base commit**: 4f1c76a
**対象**: Hono ベンチマークで検出される 9 件の `INTERSECTION_TYPE` エラー

## 要約

9 件すべてのエラーは `extract_intersection_members`（`src/pipeline/type_converter/intersections.rs:8-78`）のキャッチオール `_ =>` 分岐（line 72-74）に到達することで発生する。この関数は `TsTypeLit`、`TsTypeRef`、`TsKeywordType` の 3 種類の AST ノードのみを処理し、それ以外を一律エラーにしている。

未対応の AST ノード種別は以下の 3 種:

| AST ノード | 件数 | 例 |
|-----------|------|-----|
| TsMappedType | 5 | `{ [K in keyof T]: T[K] } & {}` |
| TsUnionType (TsUnionOrIntersectionType内) | 3 | `{ fields } & (A \| B \| C)` |
| TsConditionalType (TsParenthesized内) | 1 | `Var & (IsAny<T> extends true ? ... : ...)` |

## 個別エラー分析

### 1. adapter/aws-lambda/handler.ts:108 — TsUnionType

```typescript
export type APIGatewayProxyResult = {
  statusCode: number
  statusDescription?: string
  body: string
  cookies?: string[]
  isBase64Encoded: boolean
} & (WithHeaders | WithMultiValueHeaders)
```

- **失敗メンバー**: `(WithHeaders | WithMultiValueHeaders)` — TsUnionType
- **理想的な Rust 表現**: struct + enum フィールド、または各 variant にベースフィールドをマージした enum
- **現実的な変換**: ベースフィールドの struct + union 型を embedded フィールドとして保持

### 2. client/types.ts:67 — TsMappedType + TsConditionalType

```typescript
export type ClientRequest<Prefix, Path, S extends Schema> = {
  [M in keyof ExpandAllMethod<S>]: /* complex conditional */
} & {
  $url: ...
  $path: ...
} & (S['$get'] extends { outputFormat: 'ws' } ? { $ws: ... } : {})
```

- **失敗メンバー**: 第1メンバー（TsMappedType）および第3メンバー（TsConditionalType）
- **非常に複雑な型**: mapped type + conditional type + 通常の object type literal の 3 者交差
- **現実的な変換**: TsMappedType → HashMap フォールバック、conditional → true branch フォールバック。いずれも embedded フィールドとして struct に含める

### 3. context.ts:293 — TsConditionalType（クラス内）

```typescript
get var(): Readonly<
  ContextVariableMap & (IsAny<E['Variables']> extends true ? Record<string, any> : E['Variables'])
>
```

- **失敗メンバー**: `(IsAny<...> extends true ? ... : ...)` — TsParenthesizedType 内の TsConditionalType
- **位置**: クラスの getter 戻り値型のアノテーション内（`convert_intersection_in_annotation` 経由）
- **現実的な変換**: conditional type は `convert_conditional_type` で true branch（`Record<string, any>`）にフォールバック。embedded フィールドとして保持

### 4. helper/conninfo/types.ts:5 — TsUnionType

```typescript
export type NetAddrInfo = {
  transport?: 'tcp' | 'udp'
  port?: number
  address?: string
  addressType?: AddressType
} & ({ address: string; addressType: AddressType } | {})
```

- **失敗メンバー**: `({ address: string; addressType: AddressType } | {})` — TsUnionType
- **意味**: address/addressType が条件付き required。union の片方が空オブジェクト = 「あってもなくてもよい」
- **現実的な変換**: union 型を embedded フィールドとして保持

### 5. middleware/method-override/index.ts:11 — TsUnionType

```typescript
type MethodOverrideOptions = {
  app: Hono<any, any, any>
} & (
  | { form?: string; header?: never; query?: never }
  | { form?: never; header: string; query?: never }
  | { form?: never; header?: never; query: string }
)
```

- **失敗メンバー**: 3 variant の TsUnionType（discriminated union パターン）
- **現実的な変換**: union → enum として embedded フィールド

### 6. request.ts:30 — TsMappedType

```typescript
type RequiredRequestInit = Required<Omit<RequestInit, OptionalRequestInitProperties>> & {
  [Key in OptionalRequestInitProperties]?: RequestInit[Key]
}
```

- **失敗メンバー**: `{ [Key in OptionalRequestInitProperties]?: RequestInit[Key] }` — TsMappedType
- **第1メンバー**: `Required<Omit<...>>` は TsTypeRef → 正常に処理
- **現実的な変換**: mapped type を HashMap フォールバックで embedded フィールドとして保持

### 7. utils/body.ts:12 — TsMappedType + empty `{}`

```typescript
type SimplifyBodyData<T> = {
  [K in keyof T]: /* conditional value type */
} & {}
```

- **失敗メンバー**: `{ [K in keyof T]: ... }` — TsMappedType
- **`& {}`**: 空オブジェクトリテラルとの交差。TypeScript の型展開トリック。意味的にはno-op
- **現実的な変換**: `& {}` を除去し、単一メンバーを type alias として変換

### 8. utils/types.ts:89 — TsMappedType (identity) + empty `{}`

```typescript
export type Simplify<T> = { [KeyType in keyof T]: T[KeyType] } & {}
```

- **失敗メンバー**: `{ [KeyType in keyof T]: T[KeyType] }` — TsMappedType
- **identity mapped type**: `keyof T` のキーを `T[K]` の値にマップ = T そのもの
- **`& {}`**: no-op
- **理想的な変換**: `type Simplify<T> = T`（identity simplification は I-200 スコープ）
- **現実的な変換**: `& {}` 除去後、mapped type を type alias として変換

### 9. validator/utils.ts:23 — TsMappedType (identity) + empty `{}`

```typescript
type SimplifyDeep<T> = { [K in keyof T]: T[K] } & {}
```

- 項目 8 と同一パターン

## 根本原因

`extract_intersection_members`（`src/pipeline/type_converter/intersections.rs:8-78`）が処理する AST ノード種別が限定的:

```rust
match ty.as_ref() {
    TsType::TsTypeLit(lit) => { /* フィールド/メソッド抽出 */ }
    TsType::TsTypeRef(type_ref) => { /* レジストリ参照 or 埋め込み */ }
    TsType::TsKeywordType(_) => continue,
    _ => {
        return Err(anyhow!("unsupported intersection member type")); // ← ここ
    }
}
```

一方、`convert_ts_type`（`src/pipeline/type_converter/mod.rs:94-230`）は以下を含む幅広い AST ノードを処理可能:
- TsUnionType → `convert_union_type`（合成 enum 生成）
- TsMappedType → `HashMap<String, V>` フォールバック
- TsConditionalType → `convert_conditional_type`（true branch フォールバック）
- TsParenthesizedType → 再帰的にアンラップ

**既存の TsTypeRef フォールバック**（line 60-66）は、レジストリ未登録の型参照を `convert_type_ref` で変換し `_i` フィールドとして埋め込む。同じパターンを他の AST ノード種別に適用可能。

## 呼び出し元の影響

`extract_intersection_members` は 2 箇所から呼ばれる:

1. **`try_convert_intersection_type`**（line 85-172）— 型エイリアス位置
   - `convert_type_alias` → `try_convert_intersection_type` → `extract_intersection_members`
   - エラー時: `Result<Option<Item>>` の `Err` が伝播 → 型エイリアス変換全体が失敗
   - 該当: 項目 1, 2, 4, 5, 6, 7, 8, 9（8件）

2. **`convert_intersection_in_annotation`**（line 218-256）— アノテーション位置
   - `convert_ts_type` → `convert_intersection_in_annotation` → `extract_intersection_members`
   - エラー時: 型変換が失敗し、上位のフィールド/パラメータ変換に伝播
   - 該当: 項目 3（1件、context.ts のクラス内ゲッター）

## 修正設計

### Phase A: 汎用フォールバック（9件解消）

`extract_intersection_members` のキャッチオール `_` を修正し、`convert_ts_type` での変換を試みる:

```rust
_ => {
    let rust_type = convert_ts_type(ty, synthetic, reg)
        .map_err(|_| anyhow!("unsupported intersection member type"))?;
    fields.push(StructField {
        vis: None,
        name: format!("_{i}"),
        ty: rust_type,
    });
}
```

これにより:
- TsUnionType → enum 型として `_i` フィールドに埋め込み
- TsMappedType → HashMap として `_i` フィールドに埋め込み
- TsConditionalType → 解決型として `_i` フィールドに埋め込み
- TsParenthesizedType → convert_ts_type がアンラップして処理

**全 9 件がエラーなく変換される。**

### Phase B: `& {}` 除去（品質改善）

型エイリアス位置（`try_convert_intersection_type`）およびアノテーション位置（`convert_intersection_in_annotation`）で、空の TsTypeLit メンバーを事前にフィルタリング:

1. 空 TsTypeLit（メンバー 0 件）を除去
2. 残り 1 件の場合:
   - 型エイリアス位置: 単一メンバーを `convert_ts_type` で変換し TypeAlias として返す
   - アノテーション位置: 単一メンバーを `convert_ts_type` で直接返す
3. 残り 2+ 件: 通常の intersection 処理（Phase A 含む）

効果:
- `Simplify<T> = { [K in keyof T]: T[K] } & {}` → `type Simplify<T> = HashMap<String, Value>` (struct ではなく type alias に)
- `SimplifyBodyData<T> = mapped & {}` → `type SimplifyBodyData<T> = HashMap<String, Value>` (同上)

### Phase C（対象外: I-200 スコープ）: identity mapped type simplification

`{ [K in keyof T]: T[K] }` → `T` の検出・簡約。Simplify/SimplifyDeep が `type Simplify<T> = T` になる。Phase B の後に I-200 で実装する。

## 波及効果の検証

Phase A の修正は `extract_intersection_members` のキャッチオール分岐のみ。既存の TsTypeLit/TsTypeRef/TsKeywordType の処理に影響なし。Phase B はフィルタリングの追加であり、空でないメンバーの処理順序は変わらない。

`convert_intersection_in_annotation`（アノテーション位置）も同じ `extract_intersection_members` を使用するため、Phase A/B の恩恵を自動的に受ける。

## 参照

- `src/pipeline/type_converter/intersections.rs:8-78` — `extract_intersection_members`
- `src/pipeline/type_converter/intersections.rs:85-172` — `try_convert_intersection_type`
- `src/pipeline/type_converter/intersections.rs:218-256` — `convert_intersection_in_annotation`
- `src/pipeline/type_converter/mod.rs:94-230` — `convert_ts_type`（汎用型変換）
- `src/pipeline/type_converter/type_aliases.rs:140-327` — `convert_type_alias`（型エイリアス変換フロー）
