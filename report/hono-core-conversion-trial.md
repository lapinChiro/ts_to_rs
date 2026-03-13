# Hono コアファイル変換試行レポート

- **基準コミット**: `0936916`（ts_to_rs）
- **Hono コミット**: `0c0bf8d`（honojs/hono main, shallow clone）

## 概要

Hono のコアファイル 7 つを `ts_to_rs --report-unsupported` で変換した。全ファイルとも変換自体は完了（exit code 0）したが、いずれも部分的な変換にとどまり、未対応の構文・パターンが多数検出された。

## ファイル別変換結果

| ファイル | 出力行数 | rustfmt | unsupported 件数 | 主な問題 |
|----------|---------|---------|------------------|----------|
| `compose.ts` | 3 | OK | 1 | ほぼ全体がスキップ（関数式の変換不足） |
| `context.ts` | 30 | NG | 21 | import のハイフン問題、型エイリアス大量未対応 |
| `http-exception.ts` | 6 | NG | 1 | import のハイフン問題、パラメータパターン |
| `request.ts` | 43 | NG | 4 | import のハイフン問題、型エイリアス |
| `hono-base.ts` | 18 | NG | 8 | 型注記なしパラメータ、`in` 演算子、tuple 型 |
| `hono.ts` | 10 | NG | 1 | import のハイフン問題、パラメータパターン |
| `types.ts` | 66 | NG | 42 | 型エイリアスが大量に未対応 |

## 未対応箇所の分類と集計

### 1. import パスのハイフン → Rust モジュール名の非互換（6/7 ファイルで発生）

**問題**: `hono-base`, `http-status`, `http-exception` 等のハイフン入りパスが `use crate::hono-base::...` とそのまま出力され、Rust の識別子として不正。

**影響**: ほぼ全ファイルで rustfmt が失敗する直接原因。

**対応**: import パスのハイフンをアンダースコアに変換する（`hono-base` → `hono_base`）。

**TODO 対応**: 新規項目。

### 2. 型エイリアスの非オブジェクト型ボディ（34 件）

**問題**: `type Foo = Bar | Baz` や `type Foo = Bar & Baz`、`type Fn = (x: T) => U` 等、オブジェクトリテラル以外の型エイリアスが未対応。

**内訳**:
- conditional type（`type Foo = T extends U ? X : Y`）
- function type（`type Fn = (x: T) => U`）
- mapped type（`type Foo = { [K in keyof T]: ... }`）
- template literal type（`type Foo = \`${T}\``）
- non-nullable union type（`type Foo = A | B` で null/undefined を含まない）

**影響**: `types.ts` だけで 30 件以上。Hono は高度な型システムを活用しており、型レベルプログラミングの変換は本質的に困難。

**TODO 対応**: 新規項目（ただし大部分は Rust に直接対応がなく、段階的対応が必要）。

### 3. interface のメソッドシグネチャ（14 件）

**問題**: `interface` 内の `method(args): ReturnType` 形式のメンバーが未対応。現在はプロパティシグネチャ（`prop: Type`）のみ対応。

**影響**: `context.ts` の `ContextInterface` や `types.ts` の各種 interface で発生。

**TODO 対応**: 新規項目。

### 4. unsupported parameter pattern（4 件: http-exception, request, hono, hono.ts）

**問題**: 関数パラメータの分割代入パターン（`{ key, value }: Options`）が未対応。

**該当箇所**:
- `http-exception.ts:46` — `constructor` のオブジェクト分割代入パラメータ
- `request.ts:36` — コンストラクタのオブジェクト分割代入パラメータ
- `hono.ts:16` — コンストラクタのオブジェクト分割代入パラメータ

**TODO 対応**: 既存項目あり（「関数パラメータ位置のオブジェクト分割代入」）。

### 5. 型注記なしパラメータ（1 件: hono-base.ts）

**問題**: `const notFoundHandler = (c) => { ... }` のように型注記がないパラメータが `fn notFoundHandler(c)` と出力され、Rust として不正。

**対応**: 型注記がない場合に `Box<dyn std::any::Any>` 等のフォールバック型を生成するか、エラーとして報告する。

**TODO 対応**: 新規項目。

### 6. tuple 型（1 件: hono-base.ts）

**問題**: `[H, RouterRoute]` のような tuple 型が未対応。

**TODO 対応**: 既存項目あり（「tuple 型」）。

### 7. `in` 演算子（1 件: hono-base.ts）

**問題**: `'getResponse' in err` が未対応。

**TODO 対応**: 既存項目あり（「`in` 演算子」、保留）。

### 8. TsIndexedAccessType（2 件: context.ts）

**問題**: `E['Bindings']` のようなインデックスアクセス型が未対応。

**TODO 対応**: 新規項目。

### 9. qualified type name（1 件: context.ts）

**問題**: `Response.json()` のような修飾型名が未対応。

**TODO 対応**: 新規項目。

### 10. spread in object literal の位置制限（1 件: context.ts）

**問題**: オブジェクトリテラル内で spread が先頭以外の位置にある場合に未対応。

**TODO 対応**: 新規項目。

### 11. ExportNamed（1 件: hono-base.ts）

**問題**: `export { ... }` 形式の名前付きエクスポートが未対応。

**TODO 対応**: 新規項目。

### 12. 関数式（const fn = () => {}）の変換不足（compose.ts）

**問題**: `compose.ts` は `export const compose = <E>(...) => { ... }` という関数式で全体が構成されているが、出力は import 文 3 行のみで関数本体が完全にスキップされている。

**TODO 対応**: 新規項目（アロー関数式のトップレベル export）。

## TODO 既存項目との対応

| TODO 既存項目 | 今回検出 | 頻度 |
|--------------|---------|------|
| 関数パラメータ位置のオブジェクト分割代入 | Yes | 4 件 |
| tuple 型 | Yes | 1 件 |
| `in` 演算子 | Yes | 1 件 |
| デフォルト引数値 | 間接的（パラメータパターンに含まれる可能性） | — |

## TODO に存在しない新規未対応項目

| 項目 | 頻度 | 優先度 |
|------|------|--------|
| **import パスのハイフン→アンダースコア変換** | 6/7 ファイル | **最高** |
| **型エイリアスの非オブジェクト型ボディ** | 34 件 | 高（ただし段階的） |
| **interface のメソッドシグネチャ** | 14 件 | 高 |
| **型注記なしパラメータのフォールバック** | 1 件 | 中 |
| **TsIndexedAccessType（`E['Bindings']`）** | 2 件 | 中 |
| **ExportNamed（`export { ... }`）** | 1 件 | 中 |
| **トップレベル関数式の変換** | 1 件 | 中 |
| **spread の非先頭位置** | 1 件 | 低 |
| **qualified type name** | 1 件 | 低 |

## ブロッカー優先順位

影響範囲と修正コストを考慮した優先順位:

1. **import パスのハイフン変換** — 全ファイルに影響、修正は単純（文字列置換）
2. **interface のメソッドシグネチャ** — trait メソッド宣言として変換可能、既存の interface 変換の拡張
3. **関数パラメータのオブジェクト分割代入** — Hono コンストラクタで頻出、TODO に既存
4. **型エイリアスの非オブジェクト型ボディ** — 件数は最多だが、Hono の高度な型は Rust に直接対応がないものが多い。function type（`(x: T) => U` → `Fn(T) -> U`）と union type の基本対応から段階的に進めるのが現実的
5. **tuple 型** — 実装コストが低い、TODO に既存
6. **トップレベル関数式** — `compose.ts` 全体がスキップされる原因
