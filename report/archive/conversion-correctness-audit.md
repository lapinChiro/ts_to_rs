# 変換の論理的正当性チェック

**基準コミット**: 2b03dc2（I-168, I-172, I-170, I-160 対応後）
**最終更新**: 2026-03-19

## 概要

TypeScript → Rust 変換の全変換パスについて、型変換の正確性、文/式のセマンティクス保持、テストの網羅性・正確性を調査した。前回（4a068cc / e7d2dc3 相当）からの差分を記録。

## 1. 型変換の正確性

### Critical（コンパイル不可または意味的に誤り）

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| T-1 | `number` → `f64` の整数コンテキスト | 配列インデックス `arr[idx]` で `idx: f64` になり `usize` が必要な箇所でコンパイル不可 | 未対応 |
| T-2 | `any`/`unknown` → `Box<dyn std::any::Any>` | 式の中で直接使用不可（`x + 1` 等）。TS の `any` は任意の式で使える | 未対応 |
| T-3 | 型注記位置の intersection がフォールバック | `A & B` → `A` に縮退（`B` の情報が消失） | 未対応 |

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
| ~~T-9~~ | ~~`void` がパラメータ位置で未考慮~~ | ~~union 内 `string | void` は未テスト~~ | **修正済み**（I-170: `is_nullable_keyword()` で統一） |
| ~~T-10~~ | ~~`never` が union 内で簡約されない~~ | | **修正済み**（Phase 1 以前に対応） |

## 2. 文・式のセマンティクス

### Critical

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| ~~S-1~~ | ~~optional chaining が非 Option 型で壊れる~~ | | **修正済み** |
| ~~S-2~~ | ~~nullish coalescing が非 Option 型で壊れる~~ | | **修正済み** |
| ~~S-3~~ | ~~try/catch 内の break/continue~~ | | **修正済み** |
| ~~S-4~~ | ~~throw の条件分岐内検出漏れ~~ | | **修正済み** |
| S-16 | **正規表現パターンのエスケープ不足** | `Regex::new("{pattern}")` でパターン内の `"` や `\` がエスケープされない。`/\d+/` → `Regex::new("\d+")` で文字列リテラル内のバックスラッシュが消失 | **新規** |

### High

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| S-5 | type assertion (`x as T`) が完全に無視 | `as` 部分が削除され型情報消失 | 未対応（プリミティブ cast は対応済み） |
| ~~S-6~~ | ~~`parseInt`/`parseFloat` のエラーハンドリング~~ | | **修正済み** |
| S-7 | `const` が TS と Rust で意味が異なる | TS の `const` はフィールド変更可、Rust の `let` は不可 | 部分対応（オブジェクト型は `let mut` に） |
| S-17 | **return 文の `Some()` 包みで三項演算子未検出** | `return cond ? x : null` で `already_option` が false になり `Some(if cond { x } else { None })` に二重包み | **新規** |
| S-18 | **`.test()` / `.match()` / `.exec()` の正規表現メソッド未対応** | `regex.test(str)` が Rust の `regex.test(str)` にパススルーされコンパイル不可。`.is_match()` / `.captures()` に変換すべき | **新規** |

### Medium

| # | 問題 | 詳細 | 状態 |
|---|------|------|------|
| ~~S-8~~ | ~~ネスト optional chaining が `Option<Option<T>>`~~ | | **修正済み** |
| S-9 | `Math.max(a, b, c)` が 3 引数以上でエラー | `f64::max` は 2 引数のみ | 未対応 |
| S-10 | テンプレートリテラルのエスケープシーケンス | `raw` フィールド使用の影響が不明 | 未対応 |
| S-11 | super() が位置ベースのフィールドマッピング | 引数順序と親クラスのフィールド宣言順の不一致 | 未対応 |
| S-12 | オブジェクトスプレッドが複数不可 | `{...a, ...b}` でエラー | 未対応 |
| S-13 | 三項演算子の型不一致 | `cond ? "text" : 123` で if 式の分岐型不一致 | 未対応 |
| S-14 | 代入式が条件式内で無効 | `while (x = getValue())` がコンパイル不可 | 未対応 |
| S-15 | async void のセマンティクス差異 | TS の async void は即座に返るが Rust の async fn は Future | 未対応 |
| S-19 | **`.replaceAll()` メソッド未対応** | `str.replaceAll("a", "b")` がパススルーされる。Rust の `str.replace("a", "b")` に変換すべき | **新規** |
| S-20 | **nullish coalescing が `contains_opt_chain()` で未検出** | `return x ?? y` で `already_option` が false になる。`??` は `BinaryOp` であり `OptChain` ではない | **新規** |

## 3. テストの品質

### テストが不正確な箇所

| # | テスト | 問題 | 状態 |
|---|--------|------|------|
| Q-1 | `builtin-api-batch` スナップショット | コンパイル不可の Rust コードを期待値に持つ | 既知（compile_test でスキップ） |
| Q-2 | `integration_test__union_type.snap` | `Promise<Response>` が空の struct として生成 | 未対応 |
| Q-3 | statement テストの `matches!()` 使用 | 構造のみチェックし内容を検証していない | 部分的に改善 |
| ~~Q-4~~ | ~~throw テストの検証不足~~ | | **改善済み** |

### テストの欠落

| # | 欠落テスト | 影響 | 状態 |
|---|-----------|------|------|
| Q-5 | **return 文の `Some()` 包みの単体テスト** | I-160 の実装が statement レベルで未テスト。E2E でのみ検証 | **新規** |
| Q-6 | 正規表現パターンにエスケープが必要なケースのテスト | `"` や `\` を含むパターンが壊れるが未テスト | **新規** |
| Q-7 | 文字列 literal union + void のテスト | `type X = "a" | "b" | void` の挙動が未テスト | **新規** |
| Q-8 | replace チェーン（`s.replace("a","b").replace("c","d")`）のテスト | 未テスト（動作するはずだが検証なし） | **新規** |

### コンパイルテストのスキップ状況

51 → 60 個の統合テストのうち 1 個がコンパイルテストをスキップ:

| テスト | スキップ理由 |
|--------|-------------|
| indexed-access-type | associated type が未定義 |

（前回 7 個 → 1 個に大幅改善）

## 4. 今回のセッションでの変更の評価

| 変更 | 正確性 | テスト品質 | 残課題 |
|------|--------|-----------|--------|
| I-168 正規表現フラグ | IR 設計・メソッド選択は正しい | フラグ組み合わせテストは十分 | **パターンエスケープ (S-16)** が Critical |
| I-172 文字列 replace | `replacen(..., 1)` は正しい | 基本ケースはカバー | チェーンテスト (Q-8) が欠落 |
| I-170 void フィルタ DRY | `is_nullable_keyword()` で統一は正しい | type alias テスト追加済み | void の意味論的議論は残るが実用上は正しい |
| I-160 return Option 包み | 基本ロジックは正しい | **statement レベルの単体テストなし (Q-5)** | 三項演算子 (S-17)、nullish coalescing (S-20) の二重包みリスク |

## 5. 総合評価

### 対応の優先順位（未対応のみ、新規発見を含む）

**最優先（サイレントな意味変更 or 生成コードがコンパイルできない）:**
1. **S-16**: 正規表現パターンのエスケープ不足（Critical: コンパイル不可 or サイレント）
2. **S-17**: return Some() の三項演算子未検出（High: 二重包みでコンパイル不可）
3. T-1: number → f64 の整数コンテキスト

**高優先（機能の欠落）:**
4. **S-18**: `.test()` / `.match()` / `.exec()` 未対応（パススルーでコンパイル不可）
5. S-5: type assertion の情報保持
6. T-2: any/unknown の実用的な変換先
7. T-5: Promise の union 内展開

**中優先（エッジケース・テスト品質）:**
8. **S-19**: `.replaceAll()` 未対応
9. **S-20**: nullish coalescing の Option 検出
10. **Q-5**: return Some() の単体テスト追加
11. S-9: Math 関数の可変引数対応
