# Hono トランスパイル結果 詳細分析レポート

**基準コミット**: `f210879`
**Hono バージョン**: `e1ae0eb` (4.12.9)
**ベンチマーク日時**: 2026-03-24

## サマリ

| 指標 | 値 |
|---|---|
| 総ファイル数 | 158 |
| クリーン（エラー 0） | 86 (54.4%) |
| エラーインスタンス | 132 |
| エラー種別数 | 31 |
| コンパイルクリーン（ファイル単位） | 85/86 (98.8%) |
| コンパイルクリーン（ディレクトリ単位） | 156/158 (98.7%) |

## 1. エラー分類（詳細）

### 1.1 エラー種別ごとの件数

| 件数 | エラー種別 | TODO ID |
|---:|---|---|
| 70 | `object literal requires a type annotation to determine struct name` | I-112c |
| 8 | `unsupported intersection member type` | I-221 |
| 5 | `unsupported type alias body: Discriminant(15)` (TsIndexedAccessType) | I-219 |
| 5 | `unsupported type alias body: Discriminant(16)` (TsMappedType) | I-200 |
| 4 | `unsupported function type parameter pattern` | I-139 |
| 3 | `unsupported qualified type name` | I-36 |
| 3 | `unsupported arrow default parameter` | I-195 |
| 3 | `unsupported indexed access key: only string literals` | I-35 |
| 3 | `unsupported indexed access base type` | I-35 |
| 3 | `unsupported type alias body: Discriminant(3)` (複合ユーティリティ型) | I-219 |
| 2 | `unsupported type: TsTypeQuery for unknown identifier` | I-194 |
| 2 | `unsupported interface member` | — |
| 2 | `unsupported call target member property` | — |
| 2 | `unsupported assignment target pattern` | I-151 |
| 1 | `unsupported expression: TaggedTpl` | I-138 |
| 1 | `unsupported expression: Class(ClassExpr)` | — |
| 1 | `unsupported type: TsRestType` | — |
| 1 | `unsupported unary operator: "delete"` | I-198 |
| 1 | `unsupported expression: Seq(SeqExpr)` | I-154 |
| 1 | `unsupported expression: TsSatisfies` | I-155 |
| 1 | `unsupported for...of binding pattern` | I-148 |
| 1 | `unsupported for loop: multiple declarators` | — |
| 1 | `unsupported update expression target` | — |
| 1 | `unsupported object literal property` | I-156 |
| 1 | `default parameter requires a type annotation` | I-195 |
| 1 | `parseInt expects 1 argument` | I-196 |
| 1 | `unsupported call target expression` | — |

### 1.2 ディレクトリごとの分布

| 件数 | ディレクトリ |
|---:|---|
| 30 | middleware/ |
| 26 | utils/ |
| 24 | adapter/ |
| 19 | helper/ |
| 14 | ルートレベル |
| 10 | client/ |
| 5 | router/ |
| 3 | validator/ |
| 1 | preset/ |

### 1.3 エラー集中ファイル（4件以上）

| 件数 | ファイル | 主なエラー |
|---:|---|---|
| 8 | adapter/aws-lambda/handler.ts | object literal ×4, intersection ×3, qualified type ×1 |
| 6 | client/types.ts | type alias ×4, fn type param ×1, intersection ×1 |
| 5 | adapter/lambda-edge/handler.ts | object literal ×4, intersection ×1 |
| 5 | helper/cookie/index.ts | object literal ×5 |
| 4 | context.ts | object literal ×3, indexed access ×1 |
| 4 | middleware/language/language.ts | object literal ×2, assignment target ×2 |
| 4 | types.ts | object literal ×1, type alias ×3 |
| 4 | utils/body.ts | object literal ×3, interface member ×1 |

## 2. エラートップ3 の根本原因分析

### 2.1 `object literal requires a type annotation` (70件, 53%)

**根本原因**: `src/transformer/expressions/data_literals.rs:143-160` の `convert_object_lit()` が `expected: Option<&RustType>` に `RustType::Named` を要求。TypeResolver からの期待型が伝搬されないパターンで発生。

**未伝搬のパターン（頻度順）:**

1. **関数内の return 文やスプレッド式のオブジェクト** (~30件): `{ path: '/', ...opt, secure: true }` のようにスプレッド構文でマージされるオブジェクト。期待型が伝搬されない
2. **コールシグネチャインターフェース経由の関数パラメータ** (~10件): `const getCookie: GetCookie = (c, key?) => { ... }` で `GetCookie` がコールシグネチャを持つが、パラメータ型の伝搬が途切れる
3. **匿名型リテラル** (~10件): `{ username: string; password: string }[]` のようなインライン型。struct 名が存在しない
4. **ジェネリック型パラメータ経由** (~10件): 複雑なジェネリクスで具体型が解決できない
5. **middleware ファクトリの戻り値** (~10件): `createMiddleware(async (c, next) => { ... })` のコールバック内のオブジェクト

**修正可能性**: 段階的に対応可能。パターン 1-2 で ~40 件の削減が見込める。

### 2.2 `unsupported intersection member type` (8件, 6%)

**根本原因**: `try_convert_intersection_type` が intersection の各メンバーをフラットな struct に統合するが、メンバーが union 型（`A & (B | C)`）、型クエリ、ジェネリック型の場合に失敗。

**修正可能性**: union メンバーを持つ intersection はフラット化できないため、設計の検討が必要（enum 化 or optional フィールド化）。

### 2.3 `unsupported type alias body` (13件, 10%)

**内訳:**
- Discriminant(15) = `TsIndexedAccessType` (5件): `(typeof X)[number]`, `E['Variables']`
- Discriminant(16) = `TsMappedType` (5件): `{ [K in keyof T]: ... }`
- Discriminant(3) = 複合ユーティリティ型 (3件): `Exclude<...>[keyof T]` 等

**修正可能性**: TS の型レベルメタプログラミング。`(typeof X)[number]` は対応可能だが、mapped types は本質的に困難。

## 3. コンパイル状況

### 3.1 ファイル単位コンパイル: 85/86 (98.8%)

唯一の失敗: `index.rs` — re-export のみのファイルで余分な `}` が生成される構文エラー。

### 3.2 ディレクトリ単位コンパイル: 156/158 (98.7%)

2 エラーとも `types.rs` 内:
- `StringOrserde_json::Value` — union enum 名に `serde_json::Value` のパス区切り `::` が含まれ、不正な識別子になる
- 同ファイル内の別バリアントでも同様の問題

**根本原因**: union 型の enum 名生成（`String` + `serde_json::Value` → `StringOrserde_json_Value` とすべきところ `StringOrserde_json::Value` になる）。

## 4. クリーンファイルの品質評価

クリーンファイル 5 件のサンプル評価:

| ファイル | 構造 | 型 | セマンティクス | 総合 |
|---|---|---|---|---|
| `router/trie-router/router.ts` | ◎ struct + impl 正しい | △ f64 instead of usize | △ private field 欠落 | B |
| `compose.ts` | ◎ 関数構造正しい | △ Promise 未対応 | △ instanceof → todo!() | B |
| `utils/cookie.ts` | ◎ struct/enum 正しい | ○ CookieOptions 正確 | △ substring → slice | B+ |
| `utils/color.ts` | ○ 小規模で正確 | ○ | △ globalThis 未対応 | B |
| `index.ts` | △ re-export 余分な `}` | — | — | C（コンパイルエラー） |

**共通の品質課題:**
- `number` → `f64` 固定（`usize`/`i32` の使い分けなし）
- `typeof`/`instanceof`/`in` 演算子 → `todo!()`
- Web API 型がそのまま使用される
- `match` 等の Rust 予約語がメソッド名に使われる

## 5. 履歴推移

| 日付 | SHA | クリーン | エラー | コンパイル(file) |
|---|---|---|---|---|
| 03-18 | 0421bbd | 54 (34.2%) | 247 | — |
| 03-18 | 2f88b1b | 72 (45.6%) | 200 | — |
| 03-19 | 045b451 | 75 (47.5%) | 184 | — |
| 03-20 | 45fdc01 | 84 (53.2%) | 133 | — |
| 03-22 | d2dbf77 | 84 (53.2%) | 133 | 79 (50.0%) |
| **03-24** | **f210879** | **86 (54.4%)** | **132** | **85 (53.8%)** |

6 日間で: クリーン +32 (+59%), エラー -115 (-46.6%)。

## 6. TODO との対応表

| TODO ID | エラー種別 | 実測件数 | TODO 記載件数 | 一致 |
|---|---|---|---|---|
| I-112c | object literal no type | 70 | 70 | ✓ |
| I-221 | intersection member | 8 | 8 | ✓ |
| I-219 | conditional type (Disc 15) | 5 | 5 | ✓ |
| I-200 | mapped type (Disc 16) | 5 | 3 | **要更新** |
| I-139 | fn type param pattern | 4 | 3 | **要更新** |
| I-35 | indexed access | 6 | 5 | **要更新** |
| I-36 | qualified type | 3 | 3 | ✓ |
| I-195 | arrow default param | 3+1 | 3 | **要更新** |
| I-194 | typeof unknown id | 2 | 3 | **要更新** |
| I-138 | tagged template | 1 | 1 | ✓ |
| I-154 | seq expr | 1 | 1 | ✓ |
| I-155 | satisfies expr | 1 | 1 | ✓ |
| I-148 | for-of binding | 1 | 1 | ✓ |
| I-151 | assignment target | 2 | 2 | ✓ |
| I-156 | object literal key | 1 | 1 | ✓ |
| I-196 | parseInt args | 1 | 1 | ✓ |
| I-198 | delete operator | 1 | 1 | ✓ |

## 7. 結論と推奨

1. **最大のレバレッジ**: I-112c（object literal 70 件 = 全エラーの 53%）の段階的改善が最優先
2. **コンパイル品質は優秀**: 98.7% がコンパイル可能。残り 2 件は enum 名生成の軽微な修正で解消
3. **型エイリアスの制限**: indexed access / mapped / conditional type で 13 件。TS の型レベル計算の限界であり、段階的に対応
4. **生成コードの品質**: 構造的には正確だが、`f64` 固定・`todo!()` 多用・Web API 型残存が実用上の課題
