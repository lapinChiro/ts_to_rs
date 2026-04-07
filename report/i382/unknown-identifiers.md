# T0.2: 不明識別子 4 件 調査結果

## 該当 dangling ref と特定結果

| dangling name | 真の正体 | referencer | PRD 帰属 |
|---|---|---|---|
| `OutputType` | generic 型パラメータ (validator) | `OutputTypeOrTypedResponse` | **PRD-α** (T0.1 と同根、型パラメータ leak) |
| `Status` | generic 型パラメータ `<Status extends number>` (`ExtractSchemaForStatusCode`) | `_TypeLit4` | **PRD-α** (T0.1 と同根) |
| `__type` | TypeScript の anonymous type marker (compiler internal) | `Headers...Or__type`, `RegExpMatchArray`, `String...Or__type` | **PRD-γ** (独立バグ) |
| `symbol` | TypeScript primitive `symbol` | `F64OrStringOrsymbol` | **PRD-δ** (独立バグ) |

## 詳細

### `OutputType` (→ PRD-α)

`validator/validator.ts` L16-22:

```ts
export type Validator<...OutputType, ...> = (
  ...
) => OutputType | TypedResponse | Promise<OutputType> | Promise<TypedResponse>
```

`<OutputType>` は型パラメータ。anonymous union `OutputTypeOrTypedResponse` の中に raw `OutputType` が leak。**T0.1 の単一根本原因に同じ**。

### `Status` (→ PRD-α)

`types.ts` L2450:

```ts
export type ExtractSchemaForStatusCode<T, Status extends number> = {
  [Path in keyof ExtractSchema<T>]: {
    [Method in keyof ExtractSchema<T>[Path]]: Extract<
      ExtractSchema<T>[Path][Method],
      { status: Status }
    >
  }
}
```

`{ status: Status }` の `Status` は型パラメータ。anonymous object literal type が `_TypeLit4` という synthetic 名で生成され、その field type に raw `Status` が leak。**T0.1 と同根**。

### `__type` (→ PRD-γ)

referencers: `HeadersOrVecTupleStringStringOr__type`, `RegExpMatchArray`, `StringOrURLSearchParamsOrVecVecStringOr__type`, `StringOr__type`

`__type` は TypeScript compiler 内部で anonymous type literal を symbol table 上で表現する識別子 (例: `interface Foo { (): void }` の call signature 用)。通常は AST 上で特別な node として現れ、文字列として出力されることはない。

**仮説**: TypeCollector または TypeConverter が、`type X = () => void` のような function type literal を変換する際に、内部の symbol 名 `__type` を `RustType::Named { name: "__type" }` として誤って収集している。これは独立したバグで、PRD-α とは別経路。

**調査必要事項** (PRD-γ Discovery 時):
- TypeCollector / TypeConverter で `__type` がどこで構築されるか grep
- TS の `function type` / `call signature` / `index signature` の変換 path を追跡
- 4 つの referencer の TS source を 1 つずつ特定

### `symbol` (→ PRD-δ)

`jsx/dom/render.ts` L368:

```ts
const cancelBuild: symbol = Symbol()
```

referencer: `F64OrStringOrsymbol` (おそらく `number | string | symbol` 型)

TS `symbol` primitive は Rust に直接対応物がない。現状は型名として `symbol` (小文字) が leak し、空 stub `pub struct symbol;` (lower case) が生成され Rust 命名規約違反でもある。

**修正方針案**:
- 案 i: `symbol` を `String` (unique tag 文字列) にマップ
- 案 ii: `symbol` を `usize` (unique id) にマップ
- 案 iii: 専用 `Symbol` 型を auto-generate
- 案 iv: PRD-β (lib.dom) と統合して「unsupported primitive」として明示エラー化

**推奨**: 案 iv (PRD-β に統合)。理由: `symbol` も lib builtin の一種で、現状の anonymous synthetic への leak path が DOM 型と同じため、同じ unsupported registry で扱える。

## サブ PRD 帰属の更新

- **PRD-α** に統合: `OutputType`, `Status` (元 (c) 4 件のうち 2 件)
- **PRD-β** に統合: `symbol` (案 iv 採用)
- **PRD-γ** 独立: `__type` のみ

PRD-γ は単独で 1 件のみとなるため、PRD 化するか、または T2.checkpoint 直前に局所修正で済ませるかを T0.5 統合時に最終判定する。
