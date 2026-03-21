# Hono 変換失敗の徹底調査: 本質的ブロッカーの特定

**基準コミット**: `bcfc4a5`（未コミットの変更あり: PRD `benchmark-directory-compile-check` の修正を含む）
**調査日**: 2026-03-21
**ベンチマーク結果**: 158ファイル中 84クリーン (53.2%)、ファイル単位コンパイルクリーン 83 (52.5%)、ディレクトリ単位コンパイルクリーン 145 (91.8%)、133エラーインスタンス

## 要約

Hono の変換失敗は **2層** に分かれる:

1. **変換エラー（133件）**: 変換器が未対応構文を報告し、該当箇所の Rust コードが生成されない
2. **コンパイルエラー（12ファイル）**: Rust コードは生成されるが、構文的に不正で `cargo check` が通らない

ディレクトリコンパイルチェック導入により、以前の調査で 174 件あったクロスモジュール参照起因の偽陽性は解消された。真のコンパイルエラーは **12ファイル・16件のみ**で、**全てが構文エラー**（型エラーはゼロ）。

### ブロッカー一覧（優先順位順）

| # | ブロッカー | 変換エラー | コンパイルエラー | 対応 TODO |
|---|-----------|-----------|---------------|----------|
| 1 | 型注釈なしオブジェクトリテラルの struct 名推定 | 74件 (55.6%) | - | I-112c |
| 2 | 高度な型システム機能（conditional type 等） | 12件 (9.0%) | - | I-219, I-220 |
| 3 | intersection の未対応メンバー型 | 8件 (6.0%) | - | I-221 |
| 4 | 相対 import パスの不正変換 | - | 3ファイル | 新規 |
| 5 | 文字列エスケープの不正変換 | - | 3ファイル | 新規 |
| 6 | 個別の未対応構文（各 1-5 件） | 39件 (29.3%) | 6ファイル | 各種 |

## 1. トップダウン分析

### 1.1 変換エラー（133インスタンス）

| カテゴリ | 件数 | 割合 | 影響ファイル数 | 変換器コードパス |
|---------|------|------|-------------|---------------|
| OBJECT_LITERAL_NO_TYPE | 74 | 55.6% | 47 | `src/transformer/expressions/data_literals.rs:192` `convert_object_lit` |
| TYPE_ALIAS_UNSUPPORTED | 12 | 9.0% | 5 | `src/transformer/types/mod.rs:1094` type alias fallback |
| INTERSECTION_TYPE | 8 | 6.0% | 7 | `src/transformer/types/mod.rs:1831,1970` `try_convert_intersection_type` |
| INDEXED_ACCESS | 5 | 3.8% | 3 | `src/transformer/types/mod.rs:2029-2046` `convert_indexed_access_type` |
| OTHER | 5 | 3.8% | 5 | 複数箇所 |
| QUALIFIED_TYPE | 3 | 2.3% | 2 | `src/transformer/types/mod.rs:209,1690,1799` `convert_type_ref` |
| ARROW_DEFAULT_PARAM | 3 | 2.3% | 3 | `src/transformer/expressions/functions.rs:87` `convert_function_param_pat` |
| TYPEOF_TYPE | 3 | 2.3% | 3 | `src/transformer/types/mod.rs:193` `convert_ts_type` |
| MEMBER_PROPERTY | 3 | 2.3% | 3 | `src/transformer/expressions/calls.rs:71` `convert_call_expr` |
| FN_TYPE_PARAM | 3 | 2.3% | 3 | `src/transformer/types/mod.rs:1135,2003` `convert_fn_type` |
| ASSIGN_TARGET | 3 | 2.3% | 2 | `src/transformer/expressions/assignments.rs:30,32` `convert_assign_expr` |
| その他（各 1-2 件） | 11 | 8.3% | 11 | 各種 |

**重要な観察**: OBJECT_LITERAL_NO_TYPE が全エラーの 55.6% を占める。これのみのエラーを持つファイルが **36ファイル**あり、I-112c の解消で 84 → 120 にクリーンファイルが増加する見込み（53.2% → 75.9%）。

### 1.2 コンパイルエラー — ディレクトリ単位（12ファイル・16件）

全てが**構文エラー**。型エラー（E0308等）はゼロ。

| 根本原因 | ファイル数 | エラー件数 | 代表ファイル |
|---------|----------|----------|------------|
| 相対 import パスの不正変換（`..` / `::` 重複） | 3 | 3 | `adapter/bun/conninfo.rs`, `middleware/ip_restriction/index.rs`, `helper/streaming/text.rs` |
| 文字列エスケープの不正変換（`\?`, `\$`, `\"` 等） | 3 | 4 | `utils/accept.rs`, `client/utils.rs`, `helper/css/common.rs` |
| `fn`/`mod`/`match` 等の予約語衝突 | 2 | 4 | `middleware/logger/index.rs`, `adapter/netlify/index.rs` |
| ハイフン入り識別子（`Content-Type`） | 1 | 1 | `context.rs` |
| `$` 付きパラメータ名 | 1 | 1 | `helper/css/common.rs` |
| 負のスライスインデックス（`-1`） | 1 | 1 | `helper/ssg/utils.rs` |
| 文字列リテラル enum バリアント（`*`） | 1 | 1 | `middleware/secure_headers/secure_headers.rs` |
| 関数呼び出し結果の未返却 | 1 | 1 | `helper/accepts/accepts.rs` |

### 1.3 コンパイルエラー — ファイル単位（1ファイル・1件）

ファイル単位では `index.rs`（re-export only ファイル）の構文エラー1件のみ。PRD 1 の use パス修正により、以前の 36 ファイルから 1 ファイルに劇的に改善。

### 1.4 ファイル単位 vs ディレクトリ単位の差分分析

| 指標 | ファイル単位 | ディレクトリ単位 | 差分 |
|------|-----------|--------------|------|
| コンパイルクリーン | 83/158 (52.5%) | 145/158 (91.8%) | +62 |
| エラーファイル数 | 1 | 12 | +11 |

差分の理由: ファイル単位は変換エラー 0 件のファイルのみを対象（84ファイル）。ディレクトリ単位は全 158 ファイルの生成 Rust を1つのクレートとしてチェックするため、変換エラーがあるファイルの「生成された部分」もコンパイル対象に含まれる。ディレクトリ単位で落ちている 12 ファイルのうち、変換エラーが別途あるファイル（context.rs 等）も含まれる。

## 2. ボトムアップ分析

### 2.1 ブロッカー1: 型注釈なしオブジェクトリテラル（74件）

**本質**: TypeScript ではオブジェクトリテラル `{ key: value }` の型は構造的に推論されるが、Rust では明示的な struct 名が必要。`convert_object_lit`（`src/transformer/expressions/data_literals.rs:192`）は `expected` 型が `Some(RustType::Named { name, .. })` でない場合にエラーを返す。

**Hono での典型パターン**:

パターン A — 関数戻り値のオブジェクトリテラル:
```typescript
// helper/cookie/index.ts:30
const getCookie: GetCookie = (c, key?, prefix?) => {
  return { ...cookie }  // ← 戻り値型 Cookie から推定可能
}
```

パターン B — 変数の型注釈付きオブジェクトリテラル:
```typescript
// adapter/aws-lambda/handler.ts:268
const result: APIGatewayProxyResult = {
  body: body,
  statusCode: res.status,
}
```
型注釈 `APIGatewayProxyResult` が存在するにもかかわらずエラー。変換器が変数宣言の型注釈を `expected` として `convert_object_lit` に渡していない可能性がある。

パターン C — 引数位置のオブジェクトリテラル:
```typescript
// middleware/etag/index.ts:15
return someFunc({ weak: true })  // ← 引数の型から推定可能
```

**変換器の該当コードパス**: `src/transformer/expressions/data_literals.rs:188-195`。`expected` が `None` または `Named` 以外の場合にエラーを返す。呼び出し元（`convert_expr` 等）から `expected` 型が適切に伝搬されていない。

### 2.2 ブロッカー2: 高度な型システム機能（12件）

**本質**: TypeScript の型レベル計算（conditional type, infer, mapped type）が `convert_ts_type` で変換できずにフォールバックで報告される。`src/transformer/types/mod.rs:1094` の type alias 変換で、未対応の型ボディに対して `Discriminant(N)` 形式のエラーを出力。

**Hono での典型パターン**:

Conditional type（Discriminant 15）— 5件:
```typescript
// client/types.ts:24
type ExpandAllMethod<S> = MethodNameAll extends keyof S
  ? { [M in StandardMethods]: S[MethodNameAll] } & Omit<S, MethodNameAll>
  : S
```
**目的**: 型安全なルーティングのための型レベル分岐。特定のメソッド名キーが存在する場合にメソッドオーバーロードを展開する。

Infer type（Discriminant 16）— 4件:
```typescript
// types.ts:85
type ParamKeys<Path> = Path extends `${infer Component}/${infer Rest}`
  ? ParamKeys<Component> | ParamKeys<Rest>
  : /* ... */
```
**目的**: URL パスからパラメータ名を型レベルで抽出する。`/users/:id/posts/:postId` → `"id" | "postId"`。

Mapped type（Discriminant 3）— 3件:
```typescript
// utils/types.ts:2
type RemoveBlankRecord<T> = T extends Record<infer K, unknown>
  ? K extends string ? T : never
  : never
```
**目的**: 空の Record 型をフィルタリングする型ユーティリティ。

**変換可能性の分析**: これらは Hono の型安全なルーティングシステムの中核。Rust では trait の associated type や proc macro で一部表現可能だが、全てのパターンを自動変換するには型レベル計算のセマンティクス理解が必要。ただし Hono の場合、これらの型は全て `types.ts`、`client/types.ts`、`utils/types.ts` 等の型定義ファイルに集中しており、ランタイムコードを生成しないため、変換をスキップしても実行可能な Rust コードへの影響は限定的。

### 2.3 ブロッカー3: intersection の未対応メンバー型（8件）

**本質**: `try_convert_intersection_type`（`src/transformer/types/mod.rs:1831`）が `TsTypeLit` / `TsTypeRef` / `TsKeywordType` 以外のメンバー型を処理できない。

**Hono での典型パターン**:
```typescript
// adapter/aws-lambda/handler.ts:108
export type APIGatewayProxyResult = {
  statusCode: number
  body: string
} & (WithHeaders | WithMultiValueHeaders)
```
intersection のメンバーが union 型（`TsUnionType`）の場合に未対応。

**変換器の該当コードパス**: `src/transformer/types/mod.rs:1775-1833` の match 式。

### 2.4 ブロッカー4: 相対 import パスの不正変換（3ファイル）

**本質**: PRD 1 で `../` の import パス解決を実装したが、一部のパターンで不正なパスが残っている。

パターン A — `../..` が `..::` に:
```typescript
// adapter/bun/conninfo.ts:1
import type { Context } from '../..'
```
生成: `use crate::adapter::..::Context;`（不正）

パターン B — `./` が空モジュールに:
```typescript
// helper/streaming/text.ts:4
import { stream } from './'
```
生成: `use crate::helper::streaming::::stream;`（`::` 重複）

**変換器の該当コードパス**: `src/transformer/mod.rs:366` `convert_relative_path_to_crate_path`。`../..`（ドットドット2段）と `./`（カレントディレクトリの index）のエッジケースが未処理。

### 2.5 ブロッカー5: 文字列エスケープの不正変換（3ファイル）

**本質**: TypeScript の正規表現リテラルやテンプレートリテラルが Rust の文字列に変換される際、エスケープシーケンスが不正になる。

パターン A — 正規表現内のエスケープ:
```typescript
// client/utils.ts:20
const reg = new RegExp('/:' + k + '(?:{[^/]+})?\\??')
```
生成: `"(?:{[^/]+})?\??"` — `\?` は Rust の文字列リテラルで無効なエスケープ。`r"..."` raw string を使うべき。

パターン B — バックスラッシュの不正エスケープ:
```typescript
// utils/accept.ts:51
if (value.includes('\\')) {
```
生成: `if value.contains(&"\") {` — `"\"` は文字列を閉じていない。`"\\"` が正しい。

**変換器の該当コードパス**: `src/generator/expressions.rs` の `Expr::StringLit` 生成、および `src/transformer/expressions/literals.rs` の文字列リテラル変換。

### 2.6 ブロッカー6: 個別の未対応構文（39件・6ファイル）

以下は件数が少なく、個別に異なる未対応構文:

| パターン | 変換エラー件数 | コンパイルエラー件数 | 代表例 |
|---------|-------------|-------------------|-------|
| indexed access type | 5 | - | `context.ts`: `Record<'Content-Type', BaseMime>` |
| qualified type name | 3 | - | `context.ts`: `NodeJS.WritableStream` |
| arrow default param | 3 | - | `adapter/aws-lambda/handler.ts`: オブジェクトデストラクチャリング付きデフォルト |
| typeof 未登録識別子 | 3 | - | `adapter/cloudflare-pages/handler.ts`: `typeof fetch` |
| call target member property | 3 | - | `request.ts`: 計算プロパティアクセスのメソッド呼び出し |
| fn type parameter pattern | 3 | - | `hono-base.ts`: 関数型のオブジェクト/配列デストラクチャリング |
| assignment target | 3 | - | `utils/accept.ts`: 配列デストラクチャリング代入 |
| 予約語衝突 | - | 2 | `middleware/logger/index.rs`: パラメータ名 `fn` |
| ハイフン入り識別子 | - | 1 | `context.rs`: struct フィールド `Content-Type` |
| 負のスライスインデックス | - | 1 | `helper/ssg/utils.rs`: `arr.slice(0, -1)` |
| 文字列リテラル enum | - | 1 | `secure_headers.rs`: `'*'` → enum バリアント |
| 関数呼び出し未返却 | - | 1 | `helper/accepts/accepts.rs` |

## 3. ブロッカー間の因果関係

```
ブロッカー1 (オブジェクトリテラル 74件)
  ← 独立した問題。最大のレバレッジ
  ← 解消で 36 ファイルがクリーンに（53.2% → 75.9%）

ブロッカー2 (高度な型 12件)
  ← 型定義ファイルに限定。ランタイムコードへの影響は間接的
  ← 目的分析に基づく変換戦略の調査が必要

ブロッカー3 (intersection 8件)
  ← ブロッカー2 と一部重複（型定義ファイル内）
  ← union 型メンバーの intersection 対応で大半が解消見込み

ブロッカー4 (import パス 3ファイル)
  ← PRD 1 のエッジケース残り。修正は容易
  ← 解消でディレクトリコンパイルクリーン率がさらに向上

ブロッカー5 (文字列エスケープ 3ファイル)
  ← 正規表現の raw string 化で大半が解消見込み
  ← ブロッカー4 と合わせて修正すればコンパイルエラーが半減

ブロッカー6 (個別構文 39件)
  ← 相互に独立。各項目は TODO に既存
```

## 4. 優先順位付けと推奨アクション

### 最優先: ブロッカー1（I-112c: オブジェクトリテラル型推定）

- **直接的価値**: 変換エラー 74 件（55.6%）の解消。36 ファイルがクリーンに
- **相乗効果**: 最大。クリーンファイル増 → コンパイルチェック対象増 → 他の問題の早期発見
- **伝播防止**: 高。今後の変換機能追加が全てこの制約の下で動作する

### 高優先: ブロッカー4+5（import パス + 文字列エスケープ修正）

- **直接的価値**: ディレクトリコンパイルエラー 12 ファイル中 6 ファイルの解消
- **相乗効果**: コンパイルクリーン率 91.8% → 95%+ に
- **伝播防止**: import パスの不正は他のファイルにも影響する汎用的な問題
- **技術的実現性**: 高。エッジケースの修正のみ

### 中優先: ブロッカー3（I-221: intersection 未対応メンバー）

- **直接的価値**: 変換エラー 8 件の解消
- **相乗効果**: union 型を含む intersection は Hono の型定義で頻出パターン
- **技術的実現性**: 中。`try_convert_intersection_type` に union 型ハンドラを追加

### 要調査: ブロッカー2（I-219, I-220: conditional type / infer）

- **直接的価値**: 変換エラー 12 件の解消
- **目的分析**: これらの型は Hono の型安全ルーティングの基盤。Rust での同等機能の実現方法（trait associated type, proc macro, const generics 等）を調査し、変換戦略を設計する必要がある。TypeScript の型レベル計算が「何を達成しようとしているか」を個別に分析し、Rust で同じ目的を達成する代替手段を特定すべき（`.claude/rules/conversion-feasibility.md`）
- **補足**: 全 12 件が型定義ファイルに集中しており、ランタイムコード生成への直接影響は限定的

### 低優先: ブロッカー6（個別構文）

- 各 1-5 件の個別問題。TODO に既存の項目が多い
- I-112c 解消後のベンチマーク再実行で優先順位を再評価

### 既存 TODO との対応関係

| ブロッカー | 既存 TODO | 追加アクション |
|-----------|----------|-------------|
| 1: オブジェクトリテラル | I-112c | なし（最優先で既に記載） |
| 2: 高度な型 | I-219, I-220 | 目的分析に基づく変換戦略の調査 |
| 3: intersection | I-221 | union 型メンバー対応の追記 |
| 4: import パス | (なし) | 新規起票: `../..` と `./` のエッジケース |
| 5: 文字列エスケープ | (なし) | 新規起票: 正規表現の raw string 化、バックスラッシュエスケープ |
| 6: 個別構文 | I-35, I-36, I-195 等 | なし |

## 5. 数値目標の試算

現在: 84/158 クリーン (53.2%)、83 ファイルコンパイルクリーン (52.5%)、145 ディレクトリコンパイルクリーン (91.8%)

### ブロッカー1 解消後（I-112c）
- 予測クリーン: 120/158 (75.9%) — **+36 ファイル**
- 予測エラーインスタンス: 59 件（133 - 74）
- フェーズ移行基準「エラーインスタンス < 50 件」にあと 9 件

### ブロッカー1 + 4 + 5 解消後
- 予測ディレクトリコンパイルクリーン: 151/158 (95.6%) — **+6 ファイル**

### 全ブロッカー解消後（理想値）
- 予測クリーン: 138+/158 (87%+)
- 予測エラーインスタンス: 20 件未満
