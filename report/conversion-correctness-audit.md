# 変換の論理的正当性チェック

**基準コミット**: 4a068cc（未コミットの変更あり）

## 概要

TypeScript → Rust 変換の全変換パスについて、型変換の正確性、文/式のセマンティクス保持、テストの網羅性・正確性を調査した。

## 1. 型変換の正確性

### Critical（コンパイル不可または意味的に誤り）

| # | 問題 | 詳細 | 影響 |
|---|------|------|------|
| T-1 | `number` → `f64` の整数コンテキスト | 配列インデックス `arr[idx]` で `idx: f64` になり `usize` が必要な箇所でコンパイル不可 | 整数を引数に取る TS コードが全て影響 |
| T-2 | `any`/`unknown` → `Box<dyn std::any::Any>` | 式の中で直接使用不可（`x + 1` 等）。TS の `any` は任意の式で使える | any を使う式が全て影響 |
| T-3 | 型注記位置の intersection がフォールバック | `A & B` → `A` に縮退（`B` の情報が消失）。TODO コメントは付くが情報損失は silent | Hono 2件 |

### High（コンパイル可能だが意味的に問題）

| # | 問題 | 詳細 | 影響 |
|---|------|------|------|
| T-4 | `object` → `serde_json::Value` | JSON 文脈以外で不適切。serde_json への依存を強制 | Hono 2件（ただし初版としては許容範囲） |
| T-5 | `Promise<T>` の展開が async 関数返り値のみ | type alias や union 内の `Promise<T>` は展開されない。`Response \| Promise<Response>` で enum の片方が空の `Promise` struct になる | union 内 Promise |
| T-6 | conditional type のフォールバックが `()` | 変換失敗時に `RustType::Unit` のプレースホルダーが生成される。コンパイルは通るが意味的に誤り | types.ts 5件 |
| T-7 | indexed access type `T['Key']` → `T::Key` | TS の indexed access が Rust の associated type と等価である保証がない | Hono 2件 |

### Medium（限定的な影響）

| # | 問題 | 詳細 |
|---|------|------|
| T-8 | タプルの optional 要素未対応 | `[string, number?]` → `(String, f64)` になり `Option` にならない |
| T-9 | `void` がパラメータ位置で未考慮 | `(x: void)` → `x: ()` は正しいが、union 内 `string \| void` は未テスト |
| T-10 | `never` が union 内で簡約されない | `T \| never` は `T` と等価だが enum バリアントが生成される |

## 2. 文・式のセマンティクス

### Critical

| # | 問題 | 詳細 | 影響 |
|---|------|------|------|
| S-1 | optional chaining が非 Option 型で壊れる | `x?.y` → `x.as_ref().map(\|_v\| _v.y)` は `x` が `Option` 前提。非 null オブジェクトでコンパイル不可 | optional chaining を使う TS コード全般 |
| S-2 | nullish coalescing が非 Option 型で壊れる | `x ?? y` → `x.unwrap_or_else(\|\| y)` は `x` が `Option` 前提。`0 ?? 5` でコンパイル不可 | nullish coalescing を使う TS コード全般 |
| S-3 | try/catch 内の break/continue | 即時実行クロージャ内で break/continue はコンパイル不可 | try/catch + ループの組み合わせ |
| S-4 | throw が常に return に変換 | 関数の返り値型が `Result` でない場合に型不一致。throw 検出は関数レベルで行うが分岐内の throw を見逃す可能性あり | 条件付き throw |

### High

| # | 問題 | 詳細 |
|---|------|------|
| S-5 | type assertion (`x as T`) が完全に無視 | `as` 部分が削除され、型情報が消失。TS で型を絞り込む用途のコードが壊れる |
| S-6 | `parseInt`/`parseFloat` が `.unwrap()` でパニック | TS では `NaN` を返すが、Rust では実行時パニック |
| S-7 | `const` が TS と Rust で意味が異なる | TS の `const` はオブジェクトの再代入不可だがフィールド変更可。Rust の `let` はフィールド変更も不可 |

### Medium

| # | 問題 | 詳細 |
|---|------|------|
| S-8 | ネストした optional chaining が `Option<Option<T>>` | `x?.y?.z` でネストした `map` が `Option` をフラット化しない |
| S-9 | `Math.max(a, b, c)` が `a.max(b, c)` に変換 | `f64::max` は 2 引数のみ。3 引数以上でコンパイル不可 |
| S-10 | テンプレートリテラルのエスケープシーケンス | `raw` フィールドを使用しており、エスケープの扱いが不明 |
| S-11 | super() が位置ベースのフィールドマッピング | 引数の順序が親クラスのフィールド宣言順と一致しないと誤ったマッピング |
| S-12 | オブジェクトスプレッドが複数不可 | `{...a, ...b}` で 2 つ目の spread がエラー |
| S-13 | 三項演算子の型不一致 | `cond ? "text" : 123` で if 式の分岐型が不一致になりコンパイル不可 |
| S-14 | 代入式が条件式内で無効 | `while (x = getValue())` がコンパイル不可 |
| S-15 | async void のセマンティクス差異 | TS の async void は即座に返るが、Rust の async fn は Future を返す |

## 3. テストの品質

### テストが不正確な箇所

| # | テスト | 問題 |
|---|--------|------|
| Q-1 | `builtin-api-batch` スナップショット | 7箇所でコンパイル不可の Rust コードを期待値として持つ（`compile_test.rs` で意図的にスキップ） |
| Q-2 | `integration_test__union_type.snap` | `Promise<Response>` が空の struct として生成される。unwrap されるべき |
| Q-3 | statement テストの `matches!()` 使用 | 構造のみチェックし、内容（式の正確性等）を検証していない |
| Q-4 | `test_convert_fn_decl_throw_wraps_return_type_in_result` | 返り値型の `Result` ラッピングのみチェック。本体内の return 文が `Ok(...)` でラップされているか未検証 |

### テストが存在しない重要なシナリオ

| # | シナリオ | 重要度 |
|---|---------|--------|
| M-1 | optional chaining を非 Option 型に適用 | Critical — 現在コンパイル不可のコードを生成するが、テストなし |
| M-2 | nullish coalescing を非 Option 型に適用 | Critical — 同上 |
| M-3 | try/catch 内の break/continue | Critical — コンパイル不可になるが、テストなし |
| M-4 | throw が条件分岐内のみに存在する関数 | High — Result ラッピングの検出漏れの可能性 |
| M-5 | `parseInt("abc")` の無効入力 | High — パニックするが、テストなし |
| M-6 | 配列インデックスに number 型を使用 | High — コンパイル不可になるが、テストなし |
| M-7 | type assertion の後に型依存の操作 | High — 型情報損失によるコンパイルエラー |
| M-8 | `const` オブジェクトのフィールド変更 | Medium — TS では可能、Rust では不可 |
| M-9 | `Math.max(a, b, c)` 3引数以上 | Medium — コンパイル不可 |
| M-10 | ネスト optional chaining `x?.y?.z` | Medium — `Option<Option<T>>` になる |
| M-11 | 三項演算子で異なる型の分岐 | Medium — コンパイル不可 |
| M-12 | arrow 関数のオブジェクト分割代入 | Medium — 通常関数では対応済みだが arrow では未対応 |

### コンパイルテストのスキップ状況

51 個の統合テストのうち 7 個（13.7%）がコンパイルテストをスキップ:

| テスト | スキップ理由 |
|--------|-------------|
| indexed-access-type | associated type が未定義 |
| builtin-api-batch | クロージャ/参照の型推論不足 |
| conditional-type | 未定義 trait への参照 |
| discriminated-union | serde マクロが必要 |
| interface-mixed | 空のメソッド本体が型チェック不可 |
| union-type | derive マクロ不足 |
| error-handling | scopeguard クレート必要 |

## 4. 総合評価

### 深刻度別の問題数

| 深刻度 | 型変換 | セマンティクス | テスト | 合計 |
|--------|--------|---------------|--------|------|
| Critical | 3 | 4 | 0 | **7** |
| High | 4 | 3 | 4 | **11** |
| Medium | 3 | 8 | 0 | **11** |
| **合計** | **10** | **15** | **4** | **29** |

### 対応の優先順位

**最優先（生成コードがコンパイルできない）:**
1. S-1/S-2: optional chaining / nullish coalescing の非 Option 型対応
2. S-3: try/catch 内の break/continue
3. T-1: number → f64 の整数コンテキスト（`as usize` の自動挿入）
4. S-6: parseInt/parseFloat のパニック回避

**高優先（意味的に誤り）:**
5. S-5: type assertion の情報保持
6. S-4: throw の条件分岐対応
7. T-2: any/unknown の実用的な変換先
8. T-5: Promise の union 内展開

**中優先（エッジケース）:**
9. S-8: ネスト optional chaining のフラット化
10. S-9: Math 関数の可変引数対応
11. S-7: const のミュータビリティ差異
12. T-6: conditional type フォールバックの改善
