# Hono コアファイル変換の定量評価

**基準コミット**: 78d9f52 → interface 混在対応後に再計測（未コミット変更あり）

## 概要

Hono v4 のコアソースファイル 8 個（`index.ts` は re-export のみで 1 行出力のため除外）を `ts_to_rs` で変換し、成功/失敗を定量的に計測した。

## ファイル別サマリー

| ファイル | TS行数 | Top-level宣言数 | 未対応項目数 | 変換率 |
|----------|--------|-----------------|-------------|--------|
| compose.ts | 73 | 1 | 1 | 0% |
| context.ts | 780 | 27 | 18 | 33% |
| hono-base.ts | 539 | 8 | 5 | 38% |
| http-exception.ts | 78 | 2 | 1 | 50% |
| request.ts | 489 | 7 | 4 | 43% |
| router.ts | 103 | 10 | 4 | 60% |
| types.ts | 2488 | 52 | 22 | 58% |
| **合計** | **4550** | **107** | **55** | **49%** |

## エラー種別の集計

全 55 件の未対応項目を種別で集計:

| エラー種別 | 件数 | 説明 |
|-----------|------|------|
| `unsupported interface member` | 11 | interface のメソッドシグネチャ（プロパティ以外）|
| `unsupported type alias body` | 10 | 対応外の type alias 形式 |
| `unsupported type in union` | 9 | union 内の非リテラル・非キーワード型 |
| `non-nullable union types are not supported` | 4 | null/undefined を含まない複合 union |
| `unsupported type: TsTypeLit` / `TsConditionalType` / `TsLitType` | 4 | `convert_ts_type` で未対応の型 |
| `unsupported parameter pattern` | 2 | 未対応のパラメータパターン |
| `unsupported binary operator: "in"` | 1 | `in` 演算子 |
| `parameter has no type annotation` | 1 | 型注記なしパラメータ |
| `unsupported member property` | 2 | computed property アクセス |
| `spread requires struct in TypeRegistry` | 1 | TypeRegistry 未登録の struct での spread |
| `unsupported qualified type name` | 1 | `A.B` 形式の型名 |
| `unsupported statement` | 1 | ネスト関数宣言 |
| その他 | 8 | 上記の複合パターン |

## 影響度による優先順位

件数の多い上位 3 カテゴリを解消すると、55 件中 30 件（55%）が解消される:

### 1. interface のメソッドシグネチャ対応（11 件）

現在 interface のプロパティシグネチャのみ対応しており、メソッドシグネチャが含まれるとエラーになる。ただし interface にメソッドがある場合は既に trait 生成パスに分岐しているため、このエラーは「プロパティとメソッドが混在する interface」で発生している可能性が高い。

**対応案**: 混在 interface を struct + trait + impl に分割する、または method シグネチャを持つ field として扱う。

### 2. unsupported type alias body（10 件）

`convert_ts_type` で処理できない型が type alias の本体にある。具体的には:
- `TsTypeLit` が `convert_ts_type` の match に存在しない（type alias 内では struct に変換されるが、型注記位置では未対応）
- conditional type のフォールバック（Tier 2 で対応済みだが、`convert_ts_type` 経由のパスでは未対応）

**対応案**: `convert_ts_type` に `TsTypeLit` の arm を追加する（匿名 struct として扱う）。

### 3. unsupported type in union（9 件）

union メンバーが `TsTypeLit`（オブジェクト型リテラル）やジェネリック型参照の場合にエラー。discriminated union 対応で一部解消されたが、discriminant がない混合 union は未対応。

**対応案**: union 内の `TsTypeLit` をバリアントデータ型として扱うか、フォールバック出力する。

## 結論

- 現在の変換率は宣言レベルで **49%**（107 宣言中 52 宣言が変換成功）
- 上位 3 カテゴリ（interface メソッド混在、type alias body、union 型拡張）を解消すれば **約 75%** に向上する見込み
- discriminated union は Hono コアファイルでは直接使われていない（JSX 関連の型定義にのみ存在）
- 最も投資対効果が高いのは「interface メソッドシグネチャ混在への対応」（11 件）
