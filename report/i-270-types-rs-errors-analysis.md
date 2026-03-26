# I-270 types.rs コンパイルエラー根本原因分析

**基準コミット**: `187d801`（未コミットの I-270 実装あり）
**対象**: Hono ベンチマーク ディレクトリコンパイルの `types.rs` — 36 エラー

## エラー分類

### カテゴリ A: 不正なフィールド名（6 件）

| 行 | フィールド名 | 問題 |
|---|---|---|
| 265 | `Content-Type` | ハイフン |
| 330 | `x-request-id` | ハイフン |
| 426 | `Content-Type` | ハイフン |
| 452 | `content-type` | ハイフン |
| 447 | `_` | 予約識別子 |
| 464 | `foo[]` | ブラケット |

**根本原因**: `src/generator/mod.rs:74` の `escape_ident` が Rust キーワードのみ対応し、ハイフン・ブラケット・アンダースコアのみの識別子を処理しない。TypeScript のオブジェクトリテラルキー（`{ "Content-Type": string }`）がそのまま Rust フィールド名として出力される。

**修正箇所**: `src/generator/mod.rs:74` — `escape_ident` を `sanitize_field_name` に置き換え。ハイフンをアンダースコアに、ブラケットを除去、`_` のみのフィールドを `_unnamed` に変換。

### カテゴリ B: JSON に存在するが struct 未生成の外部型（5 件）

| 型名 | JSON 所在 | エラー行 | 未生成の理由 |
|---|---|---|---|
| `Uint8Array` | web_api + ecmascript | 111, 183, 614 | `RUST_BUILTIN_TYPES` 除外リストに含まれている |
| `KeyAlgorithm` | web_api | 513 | `CryptoKey` struct のフィールドから参照。CryptoKey 自体が外部 struct 生成で追加されるが、生成後の再走査が行われない（推移的依存の検出不足） |
| `RsaOtherPrimesInfo` | web_api | 568 | `JsonWebKey` struct のフィールドから参照。同上（推移的依存） |

**根本原因 1**: `src/pipeline/external_struct_generator.rs` の `RUST_BUILTIN_TYPES` に `Uint8Array` が含まれている（Rust の型ではなく Web API の型）。

**根本原因 2**: 外部 struct 生成が 1 回のみで固定点計算を行わない。生成された struct のフィールドが参照する型（`CryptoKey.algorithm: KeyAlgorithm`）は 2 巡目の走査で初めて検出されるが、2 巡目が実行されない。

**修正箇所**:
- `RUST_BUILTIN_TYPES` から `Uint8Array` を削除
- `collect_undefined_type_references` + `generate_external_struct` を固定点に達するまでループ

### カテゴリ C: JSON に存在せず struct 生成不能の型（16 件）

#### C-1: Hono 固有型（他ファイルで定義、types.rs から参照）— 9 件

| 型名 | 定義元ファイル | エラー行 |
|---|---|---|
| `HTTPResponseError` | `types.ts:113` | 37 |
| `JSXNode` | `jsx/base.ts` | 69 |
| `Context` | `context.ts` | 141 |
| `HtmlEscapedString` | `utils/html.ts` | 165, 245, 246 |
| `MessageFunction` | `middleware/bearer-auth/index.ts` | 196 |
| `FC` | `jsx/base.ts` | 255 |

**根本原因**: OutputWriter が synthetic items を `types.rs` に配置する際、参照先の型がどのモジュールで定義されているかを追跡しない。`types.rs` に `use crate::context::Context;` 等のインポートが生成されない。

**修正箇所**: `src/pipeline/output_writer.rs` — `types.rs` に配置する際、`use crate::module::TypeName;` を自動生成。

#### C-2: 型パラメータの漏出 — 5 件

| 型名 | 漏出元 | エラー行 |
|---|---|---|
| `T` | `RecursiveRecord<K, T>` の型パラメータ | 227 |
| `K` | `RecursiveRecord<K, T>` の型パラメータ | 228 |
| `E` | `Env` の型パラメータ | 20 |
| `H` | Hono のジェネリック型パラメータ | 153 |
| `TArrayBuffer` | `ArrayBufferView<TArrayBuffer>` の型パラメータ | 500 |

**根本原因**: 外部型の union 定義やフィールド型が型パラメータを含む場合、それが具象型として出力される。例: `interface ArrayBufferView<TArrayBuffer>` の `buffer: TArrayBuffer` が `pub buffer: TArrayBuffer` になるが、struct に `<TArrayBuffer>` 型パラメータがない。

**これは 2 つの異なる問題の複合**:
1. **抽出スクリプト**: `tools/extract-types/src/extractor.ts` が interface の型パラメータを抽出しない（`ExternalInterfaceDef` に `type_params` フィールドがない）
2. **TS ソース側の型パラメータ**: `E extends Env` や `H` はユーザーコード（Hono）のジェネリクスであり、具象化されないまま union バリアントに入る

**修正箇所**:
- `tools/extract-types/src/types.ts` — `ExternalInterfaceDef` に `type_params?: string[]` フィールド追加
- `tools/extract-types/src/extractor.ts` — `extractInterface` で `node.typeParameters` を抽出
- `src/external_types.rs` — `type_params` をパース → `TypeDef::Struct.type_params` に反映
- `src/builtin_types/ecmascript.json` + `web_api.json` — 再抽出して type_params を含める
- Hono 側の型パラメータ漏出は C-1 の解決とは別の問題で、union 登録時に型パラメータを検出して enum のジェネリクスに含める必要がある

#### C-3: JSON・Hono 両方に存在しない型 — 2 件

| 型名 | 由来 | エラー行 |
|---|---|---|
| `RecursiveRecord` | Hono のユーティリティ型（`utils/types.ts` で定義、テストファイルのみ） | 228 |
| `TemplateStringsArray` | ES 標準の tagged template 型（抽出スクリプトが `es5.d.ts` から抽出できていない） | 233 |

#### C-4: 他ファイルの合成型 — 1 件

| 型名 | 由来 | エラー行 |
|---|---|---|
| `__type` | 別ファイルのインライン型リテラルの合成名 | 54 |

**根本原因**: C-1 と同じく、OutputWriter が types.rs にインポートを生成しない。

### カテゴリ D: HashMap インポート不足（1 件）

| 型名 | エラー行 |
|---|---|
| `HashMap` | 159 |

**根本原因**: `HashMap<String, String>` は `RustType::Named { name: "HashMap", ... }` として表現される。types.rs に `use std::collections::HashMap;` が出力されない。

**修正箇所**: `src/pipeline/output_writer.rs` — types.rs に `use std::collections::HashMap;` を追加。

### カテゴリ E: ジェネリクス不一致（5 件）

| 型名 | 使用箇所 | struct 定義 | エラー行 |
|---|---|---|---|
| `ArrayBufferView<ArrayBuffer>` | enum variant | `struct ArrayBufferView { }` (0 params) | 4, 11 |
| `ReadableStream<serde_json::Value>` | enum variant | `struct ReadableStream { }` (0 params) | 14 |
| `Promise<Vec<HtmlEscapedString>>` | enum variant | `struct Promise { }` (0 params) | 246 |
| `ReadableStream<Uint8Array<ArrayBuffer>>` | struct field | `struct ReadableStream { }` (0 params) | 614 |

**根本原因**: C-2 と同根。JSON に type_params が含まれないため、外部 struct 生成時に type_params が空。しかし union 登録時の `convert_external_type` は型引数を保持するため、`ReadableStream<serde_json::Value>` のような型引数付き参照が生成される。

**修正箇所**: C-2 の修正（抽出スクリプトで type_params を抽出）で解決。

### カテゴリ F: 再帰型（1 件）

| 型名 | フィールド | エラー行 |
|---|---|---|
| `Function` | `caller: Function` | 543 |

**根本原因**: `Function` の JSON 定義に `caller: Function` フィールドがある。Rust では直接的な自己参照は infinite size エラーになるため `Box` が必要。

**修正箇所**: `src/pipeline/external_struct_generator.rs` — struct 生成時に自己参照フィールドを検出し `Box<T>` でラップ。

### カテゴリ G: BufferSource 未定義（1 件）

| 型名 | エラー行 |
|---|---|
| `BufferSource` | 123 |

**根本原因**: `BufferSource` は TypeScript の `lib.dom.d.ts` で `type BufferSource = ArrayBufferView | ArrayBuffer` として定義される型エイリアス。抽出スクリプトは型エイリアスを `ExternalAliasDef` として出力するが、現在の `external_types.rs` は alias を `None` として無視する（I-263）。

**修正箇所**: I-263（型エイリアスの TypeRegistry 登録）で解決。本 PRD では、`BufferSource` は union 型であるため `SyntheticTypeRegistry::register_union` で合成 enum として生成されるべき。ただし JSON に `BufferSource` 自体が含まれていない可能性がある。

---

## エラー件数サマリ

| カテゴリ | 件数 | 修正の複雑度 | 修正箇所 |
|---|---|---|---|
| A: 不正フィールド名 | 6 | 低 | generator |
| B: 除外リスト + 推移的依存 | 5 | 低 | external_struct_generator |
| C-1: Hono 型のインポート不在 | 9 | 中 | output_writer |
| C-2: 型パラメータ漏出 | 5 | 高 | extractor + JSON + external_types |
| C-3: JSON に存在しない型 | 2 | 中 | 抽出スクリプト改修 |
| C-4: 合成型インポート不在 | 1 | 中 | output_writer (C-1 と同じ修正) |
| D: HashMap インポート | 1 | 低 | output_writer |
| E: ジェネリクス不一致 | 5 | 高 | C-2 と同根 |
| F: 再帰型 | 1 | 低 | external_struct_generator |
| G: BufferSource | 1 | 中 | I-263 or 抽出改修 |
| **合計** | **36** | | |

## 理想的な修正設計

### 修正 1: フィールド名サニタイズ（カテゴリ A — 6 件解消）

**場所**: `src/generator/mod.rs:74`

```rust
// 現在: escape_ident(&field.name)
// 修正: sanitize_field_name(&field.name)
```

`sanitize_field_name` は以下のルールを適用:
1. ハイフン → アンダースコア（`Content-Type` → `content_type`）
2. ブラケット除去（`foo[]` → `foo`）
3. `_` のみ → `_field`（Rust の `_` は破棄パターン）
4. 先頭が数字 → `_` プレフィクス
5. 最後に `escape_ident`（キーワードエスケープ）

**設計判断**: generator の責務（IR → 有効な Rust ソース）に含まれる。IR レベルでは元の TS フィールド名を保持し、generator が出力時にサニタイズ。

### 修正 2: Uint8Array 除外リスト修正 + 推移的依存解決（カテゴリ B — 5 件解消）

**場所**: `src/pipeline/external_struct_generator.rs`

1. `RUST_BUILTIN_TYPES` から `Uint8Array` を削除
2. 外部 struct 生成をループ化:
   ```
   loop {
       let undefined = collect_undefined_type_references(&all_items, &registry);
       if undefined.is_empty() { break; }
       for name in sorted(undefined) {
           all_items.push(generate_external_struct(name, registry));
       }
   }
   ```

**設計判断**: 固定点計算は正しいアプローチ。無限ループ防止のため最大イテレーション回数（例: 10）を設定。

### 修正 3: types.rs へのインポート自動生成（カテゴリ C-1, C-4, D — 11 件解消）

**場所**: `src/pipeline/output_writer.rs`

types.rs に配置する synthetic items から参照される型名を走査し、以下のインポートを生成:

1. **Rust 標準型**: `HashMap` → `use std::collections::HashMap;`
2. **他モジュール定義型**: `Context` → `use crate::context::Context;`
3. **他ファイルの合成型**: `__type` → 参照先ファイルの特定が必要

**方式**: OutputWriter が `resolve_synthetic_placement` の後に、shared_module の内容を走査し、`file_outputs` の各ファイルで定義されている型名をインデックス化。types.rs 内で未定義だが他ファイルで定義されている型に対して `use crate::module::TypeName;` を生成。

**Rust 標準型の判定**: `HashMap`, `HashSet`, `BTreeMap` 等のリストを保持し、使用されていれば `use std::collections::*;` を追加。

### 修正 4: 抽出スクリプトの type_params 対応（カテゴリ C-2, E — 10 件解消）

**場所**:
- `tools/extract-types/src/types.ts` — スキーマに `type_params?: string[]` 追加
- `tools/extract-types/src/extractor.ts` — `extractInterface` で `node.typeParameters` 抽出
- `src/external_types.rs` — `type_params` をパース → `TypeDef::Struct.type_params` に設定
- JSON 再抽出

**JSON スキーマ変更**:
```typescript
export interface ExternalInterfaceDef {
  kind: "interface";
  type_params?: string[];  // NEW: ["T", "TArrayBuffer", "R"]
  fields: ExternalField[];
  methods: Record<string, ExternalMethod>;
  constructors: ExternalSignature[];
}
```

**Rust 側の変更**: `ExternalInterfaceDef` に `type_params: Vec<String>` を追加。`convert_external_typedef` で `TypeParam { name, constraint: None }` に変換。

### 修正 5: 再帰型検出（カテゴリ F — 1 件解消）

**場所**: `src/pipeline/external_struct_generator.rs`

`generate_external_struct` で、フィールドの型が struct 自身を参照している場合に `Box` でラップ:

```rust
let ty = if references_self(&ty, name) {
    RustType::Named { name: "Box".to_string(), type_args: vec![ty] }
} else {
    ty
};
```

### 修正 6: BufferSource + TemplateStringsArray + RecursiveRecord（カテゴリ C-3, G — 3 件解消）

- `BufferSource`: Web API の型エイリアス。抽出スクリプトに `BufferSource` が含まれていない場合は追加。I-263（alias 登録）がなくても、union 型として SyntheticTypeRegistry 経由で合成 enum が生成される可能性がある。
- `TemplateStringsArray`: ES 標準型。抽出スクリプトの lib 対象に追加。
- `RecursiveRecord`: Hono 独自型。C-1 のインポートで解決（他ファイルで定義）。

### 修正 7: Hono 固有の型パラメータ漏出（カテゴリ C-2 の一部）

`E::Bindings`、`H`、`T`、`K` のような型パラメータが union variant に漏出する問題は、union 登録時の型変換に起因する。外部型の union メンバーが型パラメータを含む場合、それを enum のジェネリクスとして伝播させるか、`serde_json::Value` にフォールバックさせる必要がある。

この問題は本 PRD のスコープでは部分的にしか解決できない（型パラメータの検出は可能だが、正しいジェネリクス伝播は複雑）。修正 4 の type_params 抽出で外部型側は解決されるが、Hono コード由来の型パラメータは別の対処が必要。

---

## 修正優先順序

| 順序 | 修正 | 解消件数 | 依存 |
|---|---|---|---|
| 1 | 修正 1: フィールド名サニタイズ | 6 | なし |
| 2 | 修正 2: 除外リスト + 推移的依存 | 5 | なし |
| 3 | 修正 5: 再帰型検出 | 1 | なし |
| 4 | 修正 3: types.rs インポート生成 | 11 | なし |
| 5 | 修正 4: type_params 抽出 | 10 | 抽出スクリプト改修 + JSON 再生成 |
| 6 | 修正 6: 不足型の追加 | 3 | 修正 4 + 修正 3 |
| **合計** | | **36** | |

修正 1〜3 は独立して実施可能。修正 4 は抽出スクリプト（TypeScript）の改修が必要で工数が最大。修正 3 と 4 が最も多くのエラーを解消する。
