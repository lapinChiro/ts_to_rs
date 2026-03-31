# 統合スナップショットテスト全件レビュー報告書

**日付**: 2026-03-31
**対象**: `tests/integration_test.rs` の全86フィクスチャ + 87スナップショット
**目的**: 各テストが「チェックすべき事柄をチェックする、意味のあるテスト」になっているかを検証する

---

## 1. レビュー観点

| 観点 | 説明 |
|------|------|
| **SILENT DROP** | 入力TSの要素（interface, type, function, variable, class）が出力Rustに存在しない |
| **SEMANTIC** | 変換結果がTSと異なるランタイム挙動を持つ（サイレント意味変更 = Tier 1 最危険） |
| **COMPILE ERROR** | 生成されたRustがコンパイル不可（Tier 2） |
| **WEAK TEST** | 入力が機能を十分にテストしていない、重要なエッジケースが欠落 |
| **ORPHAN** | フィクスチャファイルが存在するがテスト関数・スナップショットが未実装 |

---

## 2. 発見事項サマリ

| 深刻度 | 件数 | 内容 |
|--------|------|------|
| Tier 1: サイレント意味変更 | 7 | ランタイム挙動の相違がコンパイラに検出されない |
| Tier 2: コンパイルエラー | 8 | 生成Rustがコンパイル不可 |
| SILENT DROP | 5 | 入力要素が出力から欠落 |
| WEAK TEST | 30+ | テスト入力が機能カバレッジとして不十分 |
| ORPHAN | 2 | テスト未実装のフィクスチャ |

---

## 3. Tier 1: サイレント意味変更（最重要）

コンパイラが検出できず、ランタイムで異なる挙動を生む問題。

### 3.1 配列の参照意味論消失

| テスト | 問題 |
|--------|------|
| **builtin-api-batch** | `reverseArray`/`sortArray`: TSでは引数配列をin-place変更（参照意味論）。Rustでは値渡し `let mut arr = arr;` でローカル変更のみ。呼び出し元の配列は変更されない |
| **vec-method-expected-type** | `addTodo`: `items.push(newItem)` がTS側では呼び出し元配列を変更するが、Rust側は値渡しで呼び出し元に影響しない |

**影響**: 配列をin-placeで変更する全ての関数パターンに共通する構造的問題。

### 3.2 parseInt の部分パース挙動消失

| テスト | 問題 |
|--------|------|
| **number-parse-api** | `parseInt("123abc")` → TSは `123` を返す。`s.parse::<f64>()` → Rustは `Err` → `f64::NAN`。部分パースの挙動がサイレントに変わる |

### 3.3 switch の f64 パターンマッチ

| テスト | 問題 |
|--------|------|
| **switch** | `match x` で `f64` リテラルパターン (`1.0 =>`) を使用。浮動小数点のパターンマッチは非推奨であり、`NaN` の比較で予期しない挙動になる |

### 3.4 optional chaining の配列アクセス

| テスト | 問題 |
|--------|------|
| **optional-chaining** | `x?.[0]` → `x.as_ref().map(|_v| _v[0])`。TSでは境界外アクセスは `undefined`、Rustでは panic |

### 3.5 union型の prelude シャドウイング

| テスト | 問題 |
|--------|------|
| **union-type** | `type Result = Success | Failure` → `enum Result`。Rustの `std::result::Result` をシャドウイングし、同ファイル内の `Result` 使用箇所で予期しない型解決が起きる |

### 3.6 ループの非整数範囲

| テスト | 問題 |
|--------|------|
| **loops** | `for (let i = 0; i < n; i++)` → `for i in 0..n as i64`。`n` が非整数（例: `3.7`）の場合、TSは `i < 3.7` で4回目に `false`、Rustは `0..3` で3回 |

---

## 4. Tier 2: コンパイルエラー

生成Rustがコンパイル不可だが、開発者は問題に気づける。

| テスト | 問題 |
|--------|------|
| **array-builtin-methods** | `Some(doubled.iter().cloned().find(...))` → `Option<Option<f64>>` 二重ラップ。`filter` クロージャの参照問題 |
| **error-handling** | `riskyOperation()` の `Result` が `_try_result` に反映されず、catchブロック到達不能 |
| **ternary-union** | union戻り値型に対してenumラップなしのリテラル返却（`"hello"` vs `StringOrF64::String(...)`) |
| **trait-coercion** | `createGreeter()` が `None` を返すが戻り値型は `Box<dyn Greeter>`（`Option` ではない） |
| **type-narrowing** | `toFixed(2.0)` はRustの `f64` メソッドに存在しない |
| **union-fallback** | `Box<dyn Fn(f64) -> String>` に `PartialEq` derive（`dyn Fn` は `PartialEq` 未実装） |
| **external-type-struct** | `ArrayBufferOrString` enum に `toString()` メソッドが存在しない |
| **conditional-type** | 複数の条件型で型パラメータ `T` が消失（`StringOnly<T>` → `type StringOnly<T> = T;` で条件分岐消失） |

---

## 5. SILENT DROP: 入力要素の欠落

| テスト | 欠落要素 | 詳細 |
|--------|----------|------|
| **callable-interface** | `Factory` interface | construct signature (`new (config: string): Factory`) + `name` フィールドが完全欠落。`collecting` モードだがエラー報告もスナップショットに含まれていない |
| **callable-interface** | `GetCookie` オーバーロード | 2つのcall signatureのうち1つ目 `(c: string): string` が消失。2つ目のみ採用 |
| **typeof-const** | `const` 変数宣言 | `ENCODING_TYPES`, `Phase`, `Mimes`, `detectors` のランタイム値が出力にない。enum/struct型は生成されるが、対応する定数値が欠落 |
| **intersection-fallback** | 条件型のfalseブランチ | `WithConditional<T, U>` の `{ z: boolean }` ブランチが完全欠落 |
| **inline-type-literal-param** | 不要な `_TypeLit` 生成 | `_TypeLit0`, `_TypeLit1` が生成されるが、名前付き構造体 `CreateUserOpts`, `GreetOpts` と重複して使われない |

---

## 6. ORPHAN: テスト未実装フィクスチャ

| フィクスチャ | 状態 |
|-------------|------|
| **explicit-type-args.input.ts** | スナップショットなし、`integration_test.rs` にテスト関数なし |
| **private-member-expected-type.input.ts** | スナップショットなし、`integration_test.rs` にテスト関数なし |

これらは `tests/fixtures/` にファイルが存在するが、テストとして登録されていないため一切実行されない。

---

## 7. WEAK TEST: テスト網羅性不足

### 7.1 テストとして機能していない（入力が最小すぎる）

| テスト | 問題 |
|--------|------|
| **basic-types** | interface 1つのみ。`null`, `undefined`, `void`, `never`, `unknown`, `bigint`, タプル、リテラル型が全て欠落。テスト名と内容が乖離 |
| **keyword-types** | `any` と `unknown` のみ。`never`, `void`, `undefined` が未テスト。テスト名と内容が乖離 |
| **functions** | 関数2つのみ。void戻り値、複数return、rest パラメータ、オーバーロードなし |
| **mixed** | interface 1つ + function 1つ。テスト名 "mixed" の割に最小構成 |
| **nullish-coalescing** | 1ケースのみ。チェーン (`a ?? b ?? c`)、`??=` 演算子なし |
| **indexed-access-type** | 1ケースのみ。ネスト、数値インデックス、ユニオンキーなし |
| **do-while** | 1ケースのみ。break/continue、ネストなし |

### 7.2 重要なエッジケースが欠落

| テスト | 欠落エッジケース |
|--------|------------------|
| **closures** | 外部変数のキャプチャ（真のクロージャ）が一切ない。テスト名と内容が完全に乖離 |
| **optional-fields** | 構造体定義のみ。optional フィールドへのアクセスパターンが一切ない |
| **array-destructuring** | rest要素 (`[a, ...rest]`)、デフォルト値 (`[a = 0]`) なし |
| **object-destructuring** | ネスト分割代入、デフォルト値、rest パターンなし |
| **class-inheritance** | メソッドオーバーライド、`super.method()` 呼び出し、多段継承なし |
| **async-await** | await チェーン、try-catch内await、`Promise.all` なし |
| **import-export** | `import * as X`、リネームインポート、`export default`、re-export なし |
| **enum** | enum メンバーアクセス (`Color.Red`)、const enum なし |
| **string-methods** | `.slice()`, `.substring()`, `.indexOf()`, `.split()`, `.charAt()` 未テスト |
| **unary-operators** | `typeof`, `void`, `~` (bitwise NOT), `+x` (単項プラス) 未テスト |
| **update-expr** | prefix increment/decrement、式中での使用 (`arr[i++]`) なし |
| **regex-literal** | グローバルフラグ `/g`、`RegExp.test()`, 特殊文字なし |
| **unsupported-syntax** | `ExportDefaultExpr` 1ケースのみ。decorator, namespace, `with` 文等なし |
| **void-type** | `void` を含む union (`string | void`)、`Promise<void>` なし |
| **type-assertion** | `as unknown as T` (double assertion)、`as const` なし |
| **math-api** | `Math.min`, `Math.round`, `Math.random()`, 3引数以上の `Math.max` なし |

### 7.3 共通パターン: const → let mut の不要な可変性付与

以下のテストで `const` 宣言が `let mut` に変換されている:
- **object-literal**
- **string-to-string**
- **trait-coercion**
- **type-infer-unannotated**

コンパイラ warning で検出されるためTier 2だが、テストスナップショットがこの不正確な変換を「正しい」として固定している。

---

## 8. 構造的問題

### 8.1 collecting モードのスナップショットが unsupported 情報を含まない

`callable-interface` は `collecting` モードで実行されているが、スナップショットには変換成功した出力のみが記録され、`_unsupported` の内容（Factory の construct signature がサポート外としてレポートされているかどうか）が検証されていない。

```rust
// 現在のマクロ (collecting variant)
let (output, _unsupported) = transpile_collecting(&input).unwrap();
insta::assert_snapshot!(output);  // output のみ検証
// _unsupported は無視される
```

`collecting` モードを使用する全テスト（`callable_interface`, `intersection_empty_object`, `intersection_fallback`, `intersection_union_distribution`, `interface_methods`, `narrowing_truthy_instanceof`, `trait_coercion`, `anon_struct_inference`, `instanceof_builtin`）が同じ問題を持つ。入力のどの部分がサポート外として報告されているかがテストされていない。

### 8.2 スナップショットテストの限界

スナップショットテストは「出力が以前と同じか」を検証するが、「出力が正しいか」は検証しない。初回スナップショット承認時に不正確な出力が「正解」として固定されると、以降のテストは不正確な出力を守り続ける。本レビューで発見された Tier 1/Tier 2 問題の多くが、この構造に起因する。

---

## 9. param-properties 固有の問題

| 問題 | 詳細 |
|------|------|
| `public` → `pub` なし | TS の `public name` が Rust で `pub` 修飾子なしで出力 |
| `protected` → `pub(crate)` | 近似的だが正確ではない（Rust に protected 相当はない） |
| デフォルト値消失 | `WithDefault` の `= 10` が `new()` シグネチャに反映されない。TS `new WithDefault()` → Rust では `WithDefault::new(10.0)` が必要 |

---

## 10. 全テスト一覧と判定

### 凡例
- **OK**: 問題なし
- **S1**: Tier 1 サイレント意味変更
- **S2**: Tier 2 コンパイルエラー
- **SD**: Silent Drop
- **WT**: Weak Test
- **OR**: Orphan

| # | テスト名 | 判定 | 主な指摘 |
|---|----------|------|----------|
| 1 | abstract-class | OK | |
| 2 | anon-struct-inference | OK | |
| 3 | any-type-narrowing | S2, WT | `serde_json::Value` → enum 代入不可。typeof が `"string"` のみ |
| 4 | array-builtin-methods | S2 | `Some(find(...))` 二重Option、filter参照 |
| 5 | array-destructuring | WT | rest要素、デフォルト値なし |
| 6 | array-literal | OK | |
| 7 | array-methods | OK | |
| 8 | array-spread | OK | |
| 9 | as-type-expected | OK | |
| 10 | assignment-expected-type | OK | |
| 11 | async-await | WT | await チェーン、Promise.all なし |
| 12 | basic-types | WT | interface 1つのみ。基本型の大半が欠落 |
| 13 | break-continue | OK | |
| 14 | builtin-api-batch | **S1** | 配列の参照意味論消失 |
| 15 | call-signature-rest | OK | |
| 16 | callable-interface | **SD**, S2 | Factory欠落、GetCookieオーバーロード縮退 |
| 17 | class-default-params | OK | |
| 18 | class-inheritance | WT | オーバーライド、super.method()、多段継承なし |
| 19 | classes | OK | |
| 20 | closures | WT | 外部変数キャプチャなし（テスト名と乖離） |
| 21 | conditional-type | S2 | 型パラメータ消失（複数箇所） |
| 22 | console-api | OK | |
| 23 | default-params | WT | オブジェクト型デフォルト値なし |
| 24 | discriminated-union | WT | switch文での使用なし |
| 25 | do-while | WT | 1ケースのみ |
| 26 | enum | WT | メンバーアクセス、const enum なし |
| 27 | error-handling | **S2** | try-catch の Result 未反映、catch到達不能 |
| 28 | explicit-type-args | **OR** | テスト未実装 |
| 29 | external-type-struct | S2 | 空Date struct、toString未定義 |
| 30 | fn-expr | OK | |
| 31 | function-calls | WT | メソッドチェーン、再帰なし |
| 32 | functions | WT | 非常にシンプル |
| 33 | general-for-loop | WT | break/continue、ネストなし |
| 34 | generic-class | WT | メソッド、制約、複数型パラメータなし |
| 35 | generics | OK | |
| 36 | getter-setter | OK | |
| 37 | import-export | WT | リネーム、default export、re-export なし |
| 38 | indexed-access-type | WT | 1ケースのみ |
| 39 | inline-type-literal-param | SD | 不要な `_TypeLit` 重複生成 |
| 40 | instanceof-builtin | S2, WT | 生成struct にメソッドなし |
| 41 | interface-methods | OK | |
| 42 | interface-mixed | OK | |
| 43 | intersection-empty-object | OK | |
| 44 | intersection-fallback | **SD** | 条件型falseブランチ欠落 |
| 45 | intersection-methods | OK | |
| 46 | intersection-type | OK | |
| 47 | intersection-union-distribution | S2, WT | union のネスト化、重複struct生成 |
| 48 | keyword-types | WT | `never`, `void`, `undefined` 未テスト |
| 49 | loops | **S1** | 非整数range の反復回数差異 |
| 50 | math-api | WT | `Math.min`, `Math.round` 等欠落 |
| 51 | mixed | WT | 最小構成 |
| 52 | multi-var-decl | OK | |
| 53 | narrowing-truthy-instanceof | WT | typeof、null check なし |
| 54 | nullable-return | OK | |
| 55 | nullish-coalescing | WT | 1ケースのみ |
| 56 | number-parse-api | **S1** | parseInt 部分パース挙動消失 |
| 57 | object-destructuring | WT | ネスト、デフォルト値、rest なし |
| 58 | object-literal | S2 | const → let mut |
| 59 | object-spread | OK | |
| 60 | optional-chaining | **S1**, WT | 配列境界外panic。チェーン、optional method call なし |
| 61 | optional-fields | WT | アクセスパターンなし |
| 62 | param-properties | S2 | public → pub なし、デフォルト値消失 |
| 63 | private-member-expected-type | **OR** | テスト未実装 |
| 64 | regex-literal | WT | `/g`、特殊文字なし |
| 65 | string-literal-union | WT | 数値リテラルunion、関数での使用なし |
| 66 | string-methods | WT | `.slice()`, `.indexOf()` 等未テスト |
| 67 | string-to-string | S2 | const → let mut |
| 68 | switch | **S1** | f64 パターンマッチの浮動小数点問題 |
| 69 | ternary | WT | 型が分岐で異なるケースなし |
| 70 | ternary-union | S2, SD | enum ラップなし返却、未使用enum生成 |
| 71 | trait-coercion | S2 | `None` 返却 vs `Box<dyn Greeter>` 型 |
| 72 | type-alias-utility | WT | `Required`, `Pick`, `Omit` 等未テスト |
| 73 | type-assertion | WT | double assertion、`as const` なし |
| 74 | type-infer-unannotated | OK | |
| 75 | type-narrowing | S2 | `toFixed` 未定義 |
| 76 | type-registry | WT | ジェネリクス、循環参照なし |
| 77 | typeof-const | **SD** | const変数のランタイム値が欠落 |
| 78 | unary-operators | WT | `typeof`, `void`, `~` 未テスト |
| 79 | union-fallback | S2 | `dyn Fn` に PartialEq derive |
| 80 | union-type | **S1** | `Result` prelude シャドウイング |
| 81 | unsupported-syntax | WT | 1ケースのみ |
| 82 | update-expr | WT | prefix、式中使用なし |
| 83 | var-type-alias-arrow | OK | |
| 84 | var-type-arrow | OK | |
| 85 | vec-method-expected-type | **S1** | 配列 push の参照意味論消失 |
| 86 | void-type | WT | union内void、Promise<void> なし |

---

## 11. 統計

| 判定 | 件数 | 割合 |
|------|------|------|
| OK（問題なし） | 22 | 25.6% |
| WT のみ（テスト不足） | 30 | 34.9% |
| S1（サイレント意味変更） | 7 | 8.1% |
| S2（コンパイルエラー） | 15 | 17.4% |
| SD（サイレントドロップ） | 5 | 5.8% |
| OR（テスト未実装） | 2 | 2.3% |

※複数の判定が重複するテストあり。OK 以外のテストが **64件 (74.4%)** を占める。

---

## 12. 推奨アクション

### 即座に対応すべき（Tier 1 サイレント意味変更）

1. **配列の参照意味論問題** — `builtin-api-batch`, `vec-method-expected-type` で顕在化。`&mut Vec<T>` 渡しまたは戻り値での返却が必要
2. **parseInt 挙動** — `number-parse-api` の部分パースをカスタム関数で再現するか、unsupported として報告
3. **f64 パターンマッチ** — `switch` の数値case を `if-else` チェーンに変換
4. **prelude シャドウイング** — `union-type` の `Result` 等、Rust予約名との衝突検出

### 構造的改善

5. **collecting モードの unsupported 検証追加** — `_unsupported` を捨てずにスナップショット化するか、最低限 unsupported 件数をアサート
6. **orphan フィクスチャの処理** — `explicit-type-args`, `private-member-expected-type` をテスト登録するか削除
7. **テスト入力の拡充** — WEAK TEST 判定の30+テストについて、エッジケースを追加
