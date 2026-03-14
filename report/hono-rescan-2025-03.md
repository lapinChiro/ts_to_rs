# Hono コアファイル変換の再評価

**基準コミット**: 1c0280e（未コミットの変更あり: 今セッションの全変更を含む）

## 概要

前回評価（基準コミット 78d9f52、`report/hono-core-conversion-quantitative.md`）から多数の機能追加・設計改善を経て、Hono v4 コアファイル 7 個を再変換し、未対応項目を再計測した。

## ファイル別サマリー

| ファイル | TS行数 | Top-level宣言数 | 未対応項目数 | 前回未対応 | 変化 |
|----------|--------|-----------------|-------------|-----------|------|
| compose.ts | 73 | 1 | 1 | 1 | ±0 |
| context.ts | 780 | 27 | 13 | 18 | **-5** |
| hono-base.ts | 539 | 8 | 5 | 5 | ±0 |
| http-exception.ts | 78 | 2 | 1 | 1 | ±0 |
| request.ts | 489 | 7 | 4 | 4 | ±0 |
| router.ts | 103 | 10 | 4 | 4 | ±0 |
| types.ts | 2488 | 53 | 18 | 22 | **-4** |
| **合計** | **4550** | **108** | **46** | **55** | **-9** |

**変換率**: 前回 49%（52/107 成功） → 今回 **57%**（62/108 成功）

## エラー種別の集計

全 46 件の未対応項目を種別で集計:

| エラー種別 | 件数 | 前回 | 変化 | 説明 |
|-----------|------|------|------|------|
| unsupported type alias body | 11 | 10 | +1 | 対応外の type alias 形式 |
| non-nullable union types | 7 | 4 | +3 | null/undefined を含まない複合 union（型注記位置） |
| unsupported type in union | 3 | 9 | **-6** | union 内の未対応型 |
| unsupported type: TsConditionalType | 2 | — | — | 条件型（型注記位置） |
| unsupported type: TsTypeLit | 2 | — | — | 型リテラル（型注記位置） |
| unsupported type: TsLitType | 2 | — | — | リテラル型（型注記位置） |
| intersection in annotation position | 2 | — | +2 | 型注記位置での intersection（新エラーメッセージ） |
| unsupported intersection member | 2 | — | +2 | 名前付き型参照の intersection |
| unsupported indexed access key | 2 | — | — | 非文字列キーの indexed access |
| unsupported parameter pattern | 2 | 2 | ±0 | 未対応パラメータパターン |
| unsupported call signature param | 2 | — | — | call signature のパラメータ |
| unsupported member property | 2 | 2 | ±0 | computed property アクセス |
| その他（各1件） | 6 | — | — | 下記参照 |

その他（各1件）:
- `unsupported statement` — ネスト関数宣言（compose.ts）
- `unsupported qualified type name` — `A.B` 形式の型名
- `unsupported binary operator: "in"` — `in` 演算子
- `spread requires struct in TypeRegistry` — 未登録 struct での spread
- `parameter has no type annotation` — 型注記なしパラメータ
- `union with multiple non-null types` — 複数非null型の union（型注記位置）
- `unsupported type literal member` — プロパティ以外の型リテラルメンバー

## 根本原因の分析

### 1. unsupported type alias body（11件）— 最大ブロッカー

具体的な内訳:

| パターン | 件数 | 例 |
|---------|------|-----|
| conditional type（type alias 本体） | 3 | `type ExtractSchema<T> = T extends ... ? ... : ...` |
| mapped type | 3 | `type MergeSchemaPath<...> = { [P in keyof ...]: ... }` |
| keyword type `object` | 2 | `type Bindings = object` |
| template literal type | 1 | `` type AddDollar<T> = `$${Lowercase<T>}` `` |
| 複合 conditional + intersection | 2 | `type IntersectNonAnyTypes<T> = ... ? ... & ...` |

**対応方針**:
- `object` keyword → `convert_ts_type` に `TsObjectKeyword` の処理追加。`Box<dyn Any>` や `HashMap<String, serde_json::Value>` 等への変換。工数小
- mapped type → PRD 化済み（`backlog/` にあるが優先度は低い）
- conditional type → type alias 本体での conditional は一部対応済みだが、`convert_ts_type` 経由のパスで未対応
- template literal type → Rust に対応なし。保留

### 2. non-nullable union types（7件）

`Response | Promise<Response>` のような、null/undefined を含まない複数型の union が型注記位置で出現。`convert_union_type` が `T | null` パターンのみ対応しているため。

**対応方針**: `convert_union_type` を拡張して非 nullable union を enum 的に処理する。ただし型注記位置で匿名 enum を生成する設計が必要。もしくは `Box<dyn Any>` にフォールバック。

### 3. unsupported type in union（3件）— 前回から 6 件減

union 型参照バリアント対応（今セッション）で大幅改善。残りは:
- タプル配列 `[string, string][]` が union メンバー
- オブジェクト型リテラルが union メンバー（discriminated union の判定に失敗）
- ネストしたタプル/配列構造

### 4. intersection 関連（4件）

- 型注記位置での intersection（2件）: `Response & TypedResponse<T>` — 名前付き型参照同士の intersection。type alias 位置では TsTypeLit 同士のみ対応
- 名前付き型参照の intersection（2件）: `Response & TypedResponse<...>`, `Required<Omit<...>> & { ... }` — TypeRegistry 拡張が前提

### 5. その他

- ネスト関数宣言（1件）: `compose.ts` 内のクロージャ内 `async function dispatch()`
- `in` 演算子（1件）: `hono-base.ts`
- パラメータパターン（2件）: constructor のデフォルト値、complex type bounds

## 前回からの改善点

今セッションで実装した機能のうち、Hono 変換率に直接貢献したもの:

| 機能 | 解消件数 |
|------|---------|
| union 内の型参照バリアント（`Success \| Failure`） | **6件** |
| intersection 型（TsTypeLit 同士） | 間接的（context.ts で intersection エラーが減少） |
| try/catch/finally | 間接的（compose.ts の変換範囲拡大） |
| abstract class | 直接影響なし（Hono は abstract class を使わない） |

## 投資対効果の高い次のステップ

件数の多い上位カテゴリの解消効果:

| 優先度 | 対応内容 | 解消見込み | 工数 |
|--------|---------|-----------|------|
| 1 | `object` keyword を `convert_ts_type` に追加 | 2件 | 小 |
| 2 | 非 nullable union の型注記位置での処理 | 7件 | 中 |
| 3 | intersection の型注記位置・名前付き型参照対応 | 4件 | 中〜大 |
| 4 | mapped type の基本対応 | 3件 | 中 |
| 5 | conditional type の type alias 本体対応拡張 | 3件 | 中 |

上位 2 つを解消すれば 46 件中 9 件（20%）が解消され、変換率は **57% → 65%** に向上する見込み。
