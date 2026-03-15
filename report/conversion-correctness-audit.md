# 変換の論理的正当性チェック

**基準コミット**: 4a068cc（初版）
**最終更新**: Phase 1 完了後（基準コミット e7d2dc3 相当の修正状況を反映）

## 概要

TypeScript → Rust 変換の全変換パスについて、型変換の正確性、文/式のセマンティクス保持、テストの網羅性・正確性を調査した。

## 1. 型変換の正確性

### Critical（コンパイル不可または意味的に誤り）

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| T-1 | `number` → `f64` の整数コンテキスト | 配列インデックス `arr[idx]` で `idx: f64` になり `usize` が必要な箇所でコンパイル不可 | 未対応 |
| T-2 | `any`/`unknown` → `Box<dyn std::any::Any>` | 式の中で直接使用不可（`x + 1` 等）。TS の `any` は任意の式で使える | 未対応 |
| T-3 | 型注記位置の intersection がフォールバック | `A & B` → `A` に縮退（`B` の情報が消失） | 未対応（TODO #2） |

### High（コンパイル可能だが意味的に問題）

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| T-4 | `object` → `serde_json::Value` | JSON 文脈以外で不適切 | 未対応（初版として許容） |
| T-5 | `Promise<T>` の展開が async 関数返り値のみ | type alias や union 内で未展開 | 未対応 |
| T-6 | conditional type のフォールバックが `()` | 変換失敗時に `RustType::Unit` のプレースホルダー | 未対応 |
| T-7 | indexed access type `T['Key']` → `T::Key` | TS の indexed access が Rust の associated type と等価である保証がない | 未対応 |

### Medium（限定的な影響）

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| T-8 | タプルの optional 要素未対応 | `[string, number?]` → `(String, f64)` になり `Option` にならない | 未対応 |
| T-9 | `void` がパラメータ位置で未考慮 | union 内 `string \| void` は未テスト | 未対応 |
| ~~T-10~~ | ~~`never` が union 内で簡約されない~~ | | **修正済み**（Phase 1 以前に対応） |

## 2. 文・式のセマンティクス

### Critical

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| ~~S-1~~ | ~~optional chaining が非 Option 型で壊れる~~ | | **修正済み**（Phase 1: `type-env-opt-chain`） |
| ~~S-2~~ | ~~nullish coalescing が非 Option 型で壊れる~~ | | **修正済み**（Phase 1: `type-env-opt-chain`） |
| S-3 | try/catch 内の break/continue | 即時実行クロージャ内で break/continue はコンパイル不可 | 未対応 |
| ~~S-4~~ | ~~throw の条件分岐内検出漏れ~~ | | **修正済み**（Phase 1: `contains-throw-recursion`） |

### High

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| S-5 | type assertion (`x as T`) が完全に無視 | `as` 部分が削除され型情報消失 | 未対応（プリミティブ cast は対応済み） |
| S-6 | `parseInt`/`parseFloat` のエラーハンドリング | ~~`.unwrap()` でパニック~~ → `.unwrap_or(f64::NAN)` に修正済み | **修正済み** |
| S-7 | `const` が TS と Rust で意味が異なる | TS の `const` はフィールド変更可、Rust の `let` は不可 | 部分対応（オブジェクト型は `let mut` に） |

### Medium

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| ~~S-8~~ | ~~ネスト optional chaining が `Option<Option<T>>`~~ | | **修正済み**（Phase 1: `and_then` でフラット化） |
| S-9 | `Math.max(a, b, c)` が 3 引数以上でエラー | `f64::max` は 2 引数のみ | 未対応 |
| S-10 | テンプレートリテラルのエスケープシーケンス | `raw` フィールド使用の影響が不明 | 未対応 |
| S-11 | super() が位置ベースのフィールドマッピング | 引数順序と親クラスのフィールド宣言順の不一致 | 未対応 |
| S-12 | オブジェクトスプレッドが複数不可 | `{...a, ...b}` でエラー | 未対応（TODO #3） |
| S-13 | 三項演算子の型不一致 | `cond ? "text" : 123` で if 式の分岐型不一致 | 未対応 |
| S-14 | 代入式が条件式内で無効 | `while (x = getValue())` がコンパイル不可 | 未対応（TODO #4） |
| S-15 | async void のセマンティクス差異 | TS の async void は即座に返るが Rust の async fn は Future | 未対応 |

## 3. テストの品質

### テストが不正確な箇所

| # | テスト | 問題 | 状態 |
|---|--------|------|------|
| Q-1 | `builtin-api-batch` スナップショット | コンパイル不可の Rust コードを期待値に持つ | 既知（compile_test でスキップ） |
| Q-2 | `integration_test__union_type.snap` | `Promise<Response>` が空の struct として生成 | 未対応 |
| Q-3 | statement テストの `matches!()` 使用 | 構造のみチェックし内容を検証していない | 部分的に改善 |
| Q-4 | throw テストの検証不足 | ~~返り値型のみチェック~~ → body 内の Ok ラッピングも検証するテスト追加済み | **改善済み** |

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

### 対応の優先順位（未対応のみ）

**最優先（生成コードがコンパイルできない）:**
1. S-3: try/catch 内の break/continue
2. T-1: number → f64 の整数コンテキスト（`as usize` の自動挿入）

**高優先（意味的に誤り）:**
3. S-5: type assertion の情報保持
4. T-2: any/unknown の実用的な変換先
5. T-5: Promise の union 内展開

**中優先（エッジケース）:**
6. S-9: Math 関数の可変引数対応
7. S-7: const のミュータビリティ差異
8. T-6: conditional type フォールバックの改善
