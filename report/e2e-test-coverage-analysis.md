# E2E テストカバレッジ分析レポート

**基準コミット**: `1165b08`

---

## 1. E2E テストの仕組み

E2E テストは「TS と Rust で同じロジックを実行し、stdout が一致すること」を検証するブラックボックステストである。

**パイプライン**:
1. `tests/e2e/scripts/{name}.ts` を読み込む
2. `transpile()` で Rust ソースに変換
3. `tests/e2e/rust-runner/` に書き出して `cargo run` で実行
4. 同じ TS を `tsx` で実行
5. 両者の stdout を行単位で比較

**検証の性質**:
- **入力**: TypeScript ソースコード（ファイル読み込み）
- **出力**: stdout（`console.log` / `println!` の出力文字列）
- **判定基準**: TS 実行結果と Rust 実行結果の stdout 完全一致

---

## 2. 現在のカバレッジ（20 スクリプト）

### 2.1 カテゴリ別整理

| カテゴリ | スクリプト | テスト対象の概要 |
|---------|-----------|----------------|
| 基本出力 | `hello.ts` | `console.log` 単一文字列 |
| 算術演算 | `arithmetic.ts` | 四則演算、剰余、Math.floor/ceil/abs/max/min |
| 文字列操作 | `string_ops.ts` | toUpperCase, toLowerCase, includes, startsWith, trim, split+join, 文字列結合(`+`) |
| 配列操作 | `array_ops.ts` | `.length`, `.push()`, 配列リテラル |
| 関数 | `functions.ts` | 再帰、デフォルト引数 |
| クロージャ | `closures.ts` | アロー関数（型注釈付き）、複数パラメータ |
| クラス | `classes.ts` | コンストラクタ、フィールドアクセス、メソッド呼び出し（Math.sqrt） |
| クラス継承 | `class_inheritance.ts` | extends, super(), 親メソッド呼び出し、子メソッド |
| テンプレートリテラル | `template_literals.ts` | バッククォート、式の埋め込み |
| ループ | `loops.ts` | while, for-of, C-style for |
| 制御フロー | `control_flow.ts` | if/else if/else, 三項演算子 |
| 分割代入 | `destructuring.ts` | オブジェクト分割代入、配列分割代入 |
| Optional chaining | `optional_chaining.ts` | `?.`、`??` との組み合わせ |
| Nullish coalescing | `nullish_coalescing.ts` | `??` 演算子 |
| Switch | `switch_match.ts` | switch/case/default、if/else if チェーン |
| Enum | `enum_basic.ts` | 数値 enum 定義、switch での使用 |
| エラーハンドリング | `error_handling.ts` | try/catch、throw、両ブランチ return |
| ループ制御 | `loop_control.ts` | do-while、break、continue |
| スプレッド | `spread_ops.ts` | 配列スプレッド（結合、要素追加） |
| ジェネリクス | `generics.ts` | ジェネリック関数（identity パターン） |

### 2.2 E2E でカバーされている「入出力パターン」

現在の E2E テストは **単一の入出力パターン** のみ:

| 入力源 | 出力先 | カバー状況 |
|--------|--------|-----------|
| TS ソースコード（ファイル） | stdout（console.log） | **カバー済み** |
| 標準入力（stdin） | stdout | **未カバー** (TODO I-49) |
| ファイル I/O | stdout + ファイル出力 | **未カバー** (TODO I-50) |
| HTTP リクエスト/レスポンス | HTTP レスポンス | **未カバー** (TODO I-51) |
| コマンドライン引数 | stdout | **未カバー** |
| 環境変数 | stdout | **未カバー** |

---

## 3. カバレッジギャップ分析

### 3.1 実装済みだが E2E テストがない変換機能

以下は **ユニットテスト（fixtures）にはあるが E2E テストがない** 機能。E2E テストがないということは「変換後の Rust コードが正しくコンパイル・実行できるか」が未検証。

#### 高重要度（実用上よく使われる構文）

| 機能 | fixture | E2E 未カバーの理由・リスク |
|------|---------|--------------------------|
| **配列イテレータメソッド** (map/filter/find/some/every/reduce/forEach) | `array-methods.input.ts` | 変換後の `.iter().map(...).collect()` チェーンが正しく動作するか未検証。**最も重要なギャップ** |
| **async/await** | `async-await.input.ts` | `async fn` + `.await` の実行時動作が未検証 |
| **抽象クラス → trait** | `abstract-class.input.ts` | trait 定義 + impl の実行時動作が未検証 |
| **getter/setter** | `getter-setter.input.ts` | `fn name()` / `fn set_name()` パターンの動作未検証 |
| **import/export** | `import-export.input.ts` | 単一ファイル E2E では構造的にテスト不可（複数ファイル変換が必要） |
| **型アサーション** | `type-assertion.input.ts` | `as f64` キャスト等の実行時正確性が未検証 |
| **オブジェクトリテラル（struct 生成）** | `object-literal.input.ts` | 構造体初期化の実行時動作が未検証 |
| **オブジェクトスプレッド** | `object-spread.input.ts` | 配列スプレッドはあるが、オブジェクトスプレッドは未カバー |
| **オプショナルフィールド** | `optional-fields.input.ts` | `Option<T>` フィールドの実行時動作が未検証 |

#### 中重要度（特定パターンで重要）

| 機能 | fixture | E2E 未カバーの理由・リスク |
|------|---------|--------------------------|
| **文字列リテラル union → enum** | `string-literal-union.input.ts` | enum + `as_str()` の実行時動作が未検証 |
| **union type → enum** | `union-type.input.ts` | プリミティブ union enum の実行時動作が未検証 |
| **intersection type** | `intersection-type.input.ts` | マージ struct の実行時動作が未検証 |
| **discriminated union** | `discriminated-union.input.ts` | serde-tagged enum の実行時動作が未検証 |
| **conditional type** | `conditional-type.input.ts` | 型レベル変換の正確性（コンパイルチェックがスキップ中） |
| **interface（mixed）** | `interface-mixed.input.ts` | struct + trait + impl パターンの動作未検証（コンパイルチェックがスキップ中） |
| **単項演算子** (`!x`, `-x`) | `unary-operators.input.ts` | 実行時の型整合性が未検証 |
| **Number.isNaN/isFinite/isInteger, parseInt, parseFloat** | `number-parse-api.input.ts` | 実行時動作が未検証 |
| **console.error / console.warn** | `console-api.input.ts` | stderr 出力の検証が未カバー（現在 stdout のみ比較） |

#### 低重要度（エッジケースまたは稀なパターン）

| 機能 | fixture |
|------|---------|
| `void` 型 | `void-type.input.ts` |
| `any` / `unknown` / `never` 等のキーワード型 | `keyword-types.input.ts` |
| `indexed-access-type` | `indexed-access-type.input.ts` |
| インラインの型リテラルパラメータ | `inline-type-literal-param.input.ts` |
| TypeRegistry の型解決 | `type-registry.input.ts` |
| unsupported syntax の検出 | `unsupported-syntax.input.ts` |

### 3.2 E2E テスト内の「浅い」カバレッジ

既存の E2E テストスクリプトでも、各機能の **エッジケース** がカバーされていない:

| スクリプト | カバー済み | 未カバーのエッジケース |
|-----------|-----------|---------------------|
| `array_ops.ts` | length, push | **map/filter/find/reduce/some/every/forEach/sort/indexOf/slice/splice が全て未テスト** |
| `closures.ts` | 型注釈付きアロー関数 | 変数キャプチャ（クロージャの本質）、高階関数 |
| `destructuring.ts` | 基本的なオブジェクト/配列分割代入 | リネーム(`{x: newX}`)、デフォルト値(`{x = 0}`)、ネスト、rest パターン |
| `nullish_coalescing.ts` | 基本的な `??` | `undefined` 入力のテストが関数経由のみ（直接の `undefined ?? fallback` パターンなし） |
| `optional_chaining.ts` | `?.` + `??` | `undefined` オブジェクト自体への `?.` 、メソッド呼び出し `obj?.method()` |
| `enum_basic.ts` | 数値 enum + switch | 文字列 enum、enum メンバーの直接出力（I-66 の問題） |
| `generics.ts` | identity 関数 | ジェネリック struct、複数型パラメータ、制約付きジェネリクス |
| `spread_ops.ts` | 配列スプレッド | オブジェクトスプレッド |
| `error_handling.ts` | try/catch/throw | finally（未実装？）、ネストした try/catch |
| `classes.ts` | 基本クラス | static メソッド/プロパティ、getter/setter |
| `string_ops.ts` | 主要メソッド | replace、endsWith、split の各種パターン |
| `arithmetic.ts` | Math の主要関数 | Math.pow, Math.sqrt, Math.round, Math.PI 等の定数 |
| `functions.ts` | 再帰、デフォルト引数 | 複数のデフォルト引数、関数を引数に取るパターン |

### 3.3 テスト基盤の構造的制約

| 制約 | 影響 | 解決策 |
|------|------|--------|
| **単一ファイルのみ** | import/export、複数モジュールの変換がテスト不可 | 複数ファイル E2E ランナーの構築 |
| **stdout 比較のみ** | stderr（console.error/warn）、戻り値、副作用がテスト不可 | stderr 比較の追加、終了コード検証 |
| **`main()` 関数必須** | トップレベルの文、モジュールパターンがテスト不可 | 別パターンのランナーを用意 |
| **Mutex による直列実行** | 20 スクリプトが直列のため遅い | スクリプトごとに独立した Cargo プロジェクトを生成 |
| **rust-runner の固定 Cargo.toml** | 外部クレート依存のテスト（serde, tokio 等）が制限的 | 動的な Cargo.toml 生成 |
| **浮動小数点出力の完全一致** | `f64` の表示差異（`3` vs `3.0`）で偽陽性の可能性 | 許容誤差付き比較、または正規化 |

---

## 4. 優先度付き改善提案

### Tier 1: 高優先度（既存機能の信頼性に直結）

1. **配列イテレータメソッドの E2E テスト追加**
   - map, filter, find, some, every, reduce, forEach, sort, indexOf, slice
   - 変換後の `.iter()` チェーンは構文的に複雑で、ユニットテストだけでは不十分
   - 実際のユーザーコードで最もよく使われるパターンの一つ

2. **既存スクリプトのエッジケース拡充**（特に `array_ops.ts`, `destructuring.ts`）
   - `array_ops.ts`: 現在 2 操作のみ → 主要な配列メソッドを追加
   - `destructuring.ts`: リネーム、デフォルト値、rest パターンの検証

3. **オブジェクトリテラル / オブジェクトスプレッドの E2E テスト**
   - struct 初期化と struct update syntax の実行時検証

### Tier 2: 中優先度（カバレッジ拡大）

4. **async/await の E2E テスト**（tokio 依存の追加が必要）
5. **抽象クラス / getter/setter の E2E テスト**
6. **数値変換 API の E2E テスト**（parseInt, parseFloat, isNaN 等）
7. **文字列 enum / union type の E2E テスト**
8. **stderr 検証の追加**（console.error/warn 用）

### Tier 3: 低優先度（テスト基盤の拡張 — TODO I-49/50/51 に対応）

9. **stdin 入力テスト**（I-49）
10. **ファイル I/O テスト**（I-50）
11. **HTTP テスト**（I-51）
12. **複数ファイル（import/export）テスト**

---

## 5. まとめ

| 指標 | 値 |
|------|-----|
| E2E スクリプト数 | 20 |
| ユニットテスト fixture 数 | 51 |
| E2E でカバーされている機能カテゴリ | 20 |
| fixture にあるが E2E にない機能 | **約 15 カテゴリ** |
| E2E の入出力パターン | **stdout のみ**（stdin, file I/O, HTTP, stderr 全て未カバー） |
| 最大のギャップ | **配列イテレータメソッド**（最頻出の変換パターンが E2E 未検証） |
| テスト基盤の最大の制約 | **単一ファイル・stdout 比較のみ** |

E2E テストは「変換パイプライン全体の正確性」を保証する最終防衛線として機能しているが、現在のカバレッジは **基本的な構文パターンに偏っており、実用上よく使われる複合パターン（配列メソッドチェーン、オブジェクト操作、async/await）が欠落している**。
