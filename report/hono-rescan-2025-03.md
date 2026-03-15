# Hono コアファイル変換の再評価（第3回）

**基準コミット**: 95eec90（未コミットの変更あり: report/ 整理、TODO 更新）

## 概要

前回評価（基準コミット 1c0280e、変換率 57%）から 12 機能の追加実装を経て、Hono v4 コアファイル 7 個を再変換し、未対応項目を再計測した。

## ファイル別サマリー

| ファイル | TS行数 | 宣言数 | 未対応 | 前回 | 前々回 | 変化（前回比） |
|----------|--------|--------|--------|------|--------|-------------|
| compose.ts | 73 | 1 | 1 | 1 | 1 | ±0 |
| context.ts | 780 | 27 | 10 | 13 | 18 | **-3** |
| hono-base.ts | 539 | 8 | 5 | 5 | 5 | ±0 |
| http-exception.ts | 78 | 2 | 1 | 1 | 1 | ±0 |
| request.ts | 489 | 7 | 4 | 4 | 4 | ±0 |
| router.ts | 103 | 10 | 4 | 4 | 4 | ±0 |
| types.ts | 2488 | 53 | 12 | 18 | 22 | **-6** |
| **合計** | **4550** | **108** | **37** | **46** | **55** | **-9** |

**変換率**: 前々回 49% → 前回 57% → 今回 **66%**（71/108 成功）

目標の 65%+ を達成。

## エラー種別の集計

全 37 件の未対応項目を種別で集計:

| エラー種別 | 件数 | 前回 | 変化 | 説明 |
|-----------|------|------|------|------|
| unsupported type alias body | 8 | 11 | **-3** | mapped type, conditional type, template literal type 等 |
| unsupported type: TsLitType | 4 | 2 | +2 | 文字列/真偽値リテラル型（型注記位置） |
| unsupported type: TsConditionalType | 3 | 2 | +1 | conditional type（型注記位置） |
| unsupported call signature parameter pattern | 3 | 2 | +1 | call signature のパラメータ |
| unsupported member property (only identifiers) | 3 | 2 | +1 | computed property アクセス |
| unsupported indexed access key | 2 | 2 | ±0 | 非文字列キーの indexed access |
| unsupported parameter pattern | 2 | 2 | ±0 | 未対応パラメータパターン |
| unsupported type in union | 2 | 3 | **-1** | union 内の未対応型 |
| unsupported type: TsTypeLit | 2 | 2 | ±0 | 型リテラル（型注記位置） |
| unsupported intersection member type | 1 | 2 | **-1** | 名前付き型参照の intersection（TypeRegistry 未解決） |
| union with multiple non-null types | 1 | 1 | ±0 | nullable + 複数非null型の union |
| spread requires struct in TypeRegistry | 1 | 1 | ±0 | 未登録 struct での spread |
| unsupported qualified type name | 1 | 1 | ±0 | `A.B` 形式の型名 |
| unsupported binary operator: "in" | 1 | 1 | ±0 | `in` 演算子 |
| parameter has no type annotation | 1 | 1 | ±0 | 型注記なしパラメータ |
| unsupported type literal member | 1 | 1 | ±0 | プロパティ以外の型リテラルメンバー |

## 前回からの改善による解消

今セッションで実装した機能のうち、Hono 変換率に直接貢献したもの:

| 機能 | 解消件数 | 対象ファイル |
|------|---------|-------------|
| `object` keyword → `serde_json::Value` | **2件** | types.ts |
| 非 nullable union → enum 生成 | **6件** | context.ts(3), types.ts(3) |
| intersection + union 複合型 | **間接** | types.ts |
| キーワード型 type alias (`type X = string`) | **1件** | types.ts |
| **合計** | **9件** | |

## 根本原因の分析

### 1. unsupported type alias body（8件）— 最大ブロッカー

| パターン | 件数 | 例 |
|---------|------|-----|
| conditional type（type alias 本体） | 3 | `type X = T extends ... ? ... : ...` |
| mapped type | 3 | `type X = { [P in keyof T]: ... }` |
| template literal type | 1 | `` type X = `$${...}` `` |
| 不明（Discriminant 6, 17） | 1 | 要調査 |

### 2. 型注記位置の未対応型（9件）

| パターン | 件数 |
|---------|------|
| TsLitType（文字列/真偽値リテラル型） | 4 |
| TsConditionalType（conditional type） | 3 |
| TsTypeLit（インライン型リテラル） | 2 |

型注記位置で `convert_ts_type` が処理できない型。`extra_items` 伝搬の仕組みは整ったため、各型の変換ロジック追加で対応可能。

### 3. call signature パラメータ（3件）

interface の call signature `(req: Request): Response` のパラメータが未対応。

### 4. computed property（3件）

`obj[Symbol.for('...')]` のような動的プロパティキー。Rust に直接対応なし。

### 5. その他（14件）

- indexed access の非文字列キー（2件）
- パラメータパターン（2件）
- union 内の未対応型（2件）
- intersection（1件）: `Required<Omit<...>> & { ... }` — ジェネリック型の展開が前提
- union with multiple non-null types（1件）: nullable + 複数型の union
- その他各1件（spread, qualified name, `in` 演算子, 型注記なし, 型リテラルメンバー）

## 投資対効果の高い次のステップ

| 優先度 | 対応内容 | 解消見込み | 工数 |
|--------|---------|-----------|------|
| 1 | TsLitType（リテラル型）の型注記位置対応 | 4件 | 小 |
| 2 | TsConditionalType の型注記位置対応 | 3件 | 中 |
| 3 | mapped type の基本対応 | 3件 | 中 |
| 4 | call signature パラメータの対応 | 3件 | 小 |
| 5 | TsTypeLit の型注記位置対応 | 2件 | 中 |

上位 2 つを解消すれば 37 件中 7 件（19%）が解消され、変換率は **66% → 72%** に向上する見込み。

## 変換率の推移

| 時点 | 変換率 | 成功/合計 | 未対応 |
|------|--------|----------|--------|
| 初回（78d9f52） | 49% | 52/107 | 55 |
| 前回（1c0280e） | 57% | 62/108 | 46 |
| 今回（95eec90） | **66%** | **71/108** | **37** |

## 補足: router.ts の `match` 予約語問題

router.ts の変換で `match` が Rust の予約語と衝突し、rustfmt がエラーを返す。TS の `match` メソッド名を `r#match` にエスケープする対応が必要（TODO に記載なし、軽微）。
