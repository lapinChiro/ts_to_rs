# フォールバック型インベントリ: 不正確な型付与の網羅的分析

**日付**: 2026-03-28（2026-03-30 注記追加）
**Base commit**: 4f1c76a
**目的**: パイプライン全体でフォールバック（Any, Unknown, HashMap, serde_json::Value, todo!()）が発生する全箇所をコードからトレースし、各箇所で何が起きているか、どのような情報が利用可能かを記録する

> **⚠️ 注意**: 行番号は base commit 時点のもの。C-1〜C-3 の開発（`0f4a3c3` まで）で多数のコード変更があり、行番号はずれている。コード参照時は関数名・パターンで検索すること。設計的価値（フォールバック分類体系、カスケード分析）は引き続き有効。

## 方法論

`src/` 配下で `RustType::Any`, `ResolvedType::Unknown`, `serde_json::Value`, `unwrap_or` を含む全行（170 箇所）を収集し、**フォールバック型を生成する箇所**（production sites）を分類した。型を参照する箇所（consumption sites: `matches!()`, `assert`, 条件分岐等）は除外。

## フォールバック型の生成サイト一覧

### A. `RustType::Any` を直接生成する箇所（26 箇所）

#### A-1. TypeConverter（TS 型 AST → RustType 変換）

| # | ファイル:行 | トリガー | コード | 発生頻度の見込み |
|---|------------|---------|--------|-----------------|
| 1 | `type_converter/mod.rs:105-106` | TS `any`/`unknown` キーワード型 | `TsAnyKeyword \| TsUnknownKeyword => Ok(RustType::Any)` | 高（Hono に多数） |
| 2 | `type_converter/mod.rs:166` | mapped type の value type 変換失敗 | `.unwrap_or(RustType::Any)` | 中 |
| 3 | `type_converter/interfaces.rs:164` | call signature パラメータ注釈なし | `.unwrap_or(RustType::Any)` | 低 |
| 4 | `type_converter/interfaces.rs:179` | rest パラメータ注釈なし | `.unwrap_or(RustType::Vec(Box::new(RustType::Any)))` | 低 |
| 5 | `type_converter/type_aliases.rs:35` | conditional type の true branch 変換失敗 | `convert_ts_type(...).unwrap_or(RustType::Any)` | 中 |
| 6 | `type_converter/type_aliases.rs:222` | fn type literal のパラメータ注釈なし | `.unwrap_or(RustType::Any)` | 低 |
| 7 | `type_converter/type_aliases.rs:225` | 未対応 fn パラメータパターン | `param_types.push(RustType::Any)` | 中（FN_TYPE_PARAM 3件） |
| 8 | `type_converter/type_aliases.rs:352` | method signature のパラメータ注釈なし | `.unwrap_or(RustType::Any)` | 低 |
| 9 | `type_converter/unions.rs:501` | union の `any`/`unknown`/`object` メンバー | `("Any", RustType::Any)` | 中 |
| 10 | `type_converter/utilities.rs:436` | union の変換不能メンバー | `data: Some(RustType::Any)` | 低 |

#### A-2. TypeRegistry/Collection（型情報収集）

| # | ファイル:行 | トリガー | コード |
|---|------------|---------|--------|
| 11 | `registry/collection.rs:531` | TS パラメータの型注釈変換失敗 | `.unwrap_or(RustType::Any)` |
| 12 | `registry/collection.rs:646` | 非対応パラメータパターン（Rest 等） | `_ => (RustType::Any, None)` |
| 13 | `registry/functions.rs:75` | call signature パラメータ注釈なし | `.unwrap_or(RustType::Any)` |

#### A-3. External Types（ビルトイン型ロード）

| # | ファイル:行 | トリガー | コード |
|---|------------|---------|--------|
| 14 | `external_types/mod.rs:328` | JSON の `any`/`unknown` 型 | `ExternalType::Any \| Unknown => RustType::Any` |
| 15 | `external_types/mod.rs:330` | JSON の `null`/`undefined` 型 | `Option(Box::new(RustType::Any))` |
| 16 | `external_types/mod.rs:379` | null-only union | `RustType::Any` |

#### A-4. TypeResolver（型解決）

| # | ファイル:行 | トリガー | コード | ボトムアップ推論の可能性 |
|---|------------|---------|--------|----------------------|
| 17 | `expressions.rs:38` | null リテラル | `Option(Box::new(RustType::Any))` | **有**: expected type から inner type を推論可（例: `null as string \| null` → `Option<String>`） |
| 18 | `expressions.rs:99` | 三項演算子の片方 Unknown | `Option(Box::new(RustType::Any))` | **有**: もう片方の分岐型を使用可 |
| 19 | `expressions.rs:356` | optional chaining 結果 Unknown | `Option(Box::new(RustType::Any))` | **有**: プロパティ名から TypeRegistry 参照で型推論可 |
| 20 | `expressions.rs:652,674,757` | 関数内パラメータ型（Fn 型構築時） | `.unwrap_or(RustType::Any)` | **有**: 呼び出し側の実引数型から逆推論可 |
| 21 | `visitors.rs:54` | 関数パラメータ注釈変換失敗 | `.unwrap_or(RustType::Any)` | **有**: 呼び出し側の引数型、関数本体での使用 |

#### A-5. Transformer（IR 生成）

| # | ファイル:行 | トリガー | コード | ボトムアップ推論の可能性 |
|---|------------|---------|--------|----------------------|
| 22 | `functions/params.rs:35` | パラメータ型解決失敗 | `ty: Some(RustType::Any)` | **有**: 呼び出し側引数型 |
| 23 | `functions/params.rs:170` | Optional パラメータ型＋デフォルト値推論失敗 | `unwrap_or(RustType::Any)` | **有**: デフォルト値の式型をより深く分析、呼び出し側 |
| 24 | `functions/arrow_fns.rs:88` | アロー関数パラメータ注釈なし | `p.ty = Some(RustType::Any)` | **有**: 親関数のコールバックパラメータ型から推論 |
| 25 | `classes/members.rs:62` | クラスプロパティ注釈なし | `None => RustType::Any` | **有**: コンストラクタ代入、メソッド使用パターン |
| 26 | `classes/members.rs:181` | param property 注釈なし | `unwrap_or(RustType::Any)` | **有**: クラス内使用パターン |
| 27 | `classes/members.rs:336` | メソッド戻り値型注釈なし | `None => RustType::Any` | **有**: return 文の式型 |
| 28 | `functions/mod.rs:226` | 戻り値型変換失敗（resilient モード） | `Ok(RustType::Any)` | **有**: return 文の式型、呼び出し側の期待型 |
| 29 | `any_narrowing.rs:96-97` | typeof `"object"`/`"function"` の variant | `RustType::Any` | **限定的**: オブジェクト/関数の具体型は typeof だけでは不明 |
| 30 | `any_narrowing.rs:133` | Other variant（enum fallback） | `data: Some(RustType::Any)` | **限定的**: narrowing されなかった残余型 |

### B. `serde_json::Value` を直接生成する箇所（3 箇所）

| # | ファイル:行 | トリガー | コード |
|---|------------|---------|--------|
| 1 | `type_converter/mod.rs:109-111` | TS `object` キーワード型 | `name: "serde_json::Value"` |
| 2 | `transformer/functions/destructuring.rs:20-22` | 分割代入パラメータ注釈なし | `name: "serde_json::Value"` |
| 3 | `generator/types.rs:52` | `RustType::Any` のコード生成 | `"serde_json::Value"` |

**注**: B-3 が最終出力段。A セクションの全ての `RustType::Any` が最終的にここを通って `serde_json::Value` になる。

### C. `HashMap<String, V>` フォールバック（2 箇所）

| # | ファイル:行 | トリガー | コード | 改善可能性 |
|---|------------|---------|--------|-----------|
| 1 | `type_converter/mod.rs:159-170` | 全ての TsMappedType | `HashMap<String, V>` | **有**: identity mapped type → T、keyof 制約解決でキー型精度向上 |
| 2 | `type_converter/type_aliases.rs:271-283` | index signature `{ [key: string]: T }` | `HashMap<String, T>` | 正確（改善不要） |

### D. `ResolvedType::Unknown` を生成する箇所（30+ 箇所）

#### D-1. 識別子解決失敗

| # | ファイル:行 | トリガー | ボトムアップ推論の可能性 |
|---|------------|---------|----------------------|
| 1 | `type_resolver/mod.rs:139` | 変数がスコープに不在 | **有**: import 先の型情報、external types |

#### D-2. 型注釈変換失敗

| # | ファイル:行 | トリガー | ボトムアップ推論の可能性 |
|---|------------|---------|----------------------|
| 2 | `expressions.rs:52` | `as T` の型変換失敗 | **有**: T の型名から TypeRegistry 参照 |
| 3 | `expressions.rs:138` | メンバーアクセスの型注釈変換失敗 | **有**: プロパティ名 + 使用パターン |
| 4 | `visitors.rs:119` | パラメータ注釈変換失敗 | **有**: A-21 と同じ |
| 5 | `visitors.rs:163` | 変数注釈なし＋初期化なし | **有**: 後続代入パターン（2パス必要） |

#### D-3. 式型解決失敗

| # | ファイル:行 | トリガー | ボトムアップ推論の可能性 |
|---|------------|---------|----------------------|
| 6 | `expressions.rs:119` | 三項演算子の分岐型不一致 | **有**: 両分岐型から union 生成可 |
| 7 | `expressions.rs:212-234` | オブジェクトリテラルのフィールド/スプレッド型解決失敗 | **有**: 一部フィールドの型が解決済みなら部分的 struct 生成可 |
| 8 | `expressions.rs:280,331,333,337` | optional chaining の各種失敗 | **有**: プロパティ名 + TypeRegistry |
| 9 | `expressions.rs:392,394` | catch-all（未対応式パターン） | **限定的**: 式の構造分析 |
| 10 | `expressions.rs:491` | Unknown オブジェクトのメンバーアクセス | **有**: プロパティ名から候補型を推論 |
| 11 | `expressions.rs:512,521,532,546,547,554` | typeof/template literal 等 | **限定的** |
| 12 | `expressions.rs:583,784,810` | class/array 等 | **限定的** |

#### D-4. 呼び出し解決失敗

| # | ファイル:行 | トリガー | ボトムアップ推論の可能性 |
|---|------------|---------|----------------------|
| 13 | `call_resolution.rs:18` | 非 Expr callee | **有**: super → 親クラス constructor |
| 14 | `call_resolution.rs:36` | 関数名スコープ不在 | **有**: import/external 参照 |
| 15 | `call_resolution.rs:45,54,59,68,87` | メソッド解決失敗各種 | **有**: レシーバ型 + メソッド名 |
| 16 | `call_resolution.rs:263,265` | オーバーロード戻り値解決失敗 | **有**: 引数型からの選択 |

#### D-5. 宣言型解決失敗

| # | ファイル:行 | トリガー | ボトムアップ推論の可能性 |
|---|------------|---------|----------------------|
| 17 | `visitors.rs:199` | クラスプロパティ注釈なし | **有**: コンストラクタ + メソッド |
| 18 | `visitors.rs:282` | static this（意図的 Unknown） | 不要（設計上） |

### E. `todo!()` 生成（3 パターン）

| # | ファイル:行 | トリガー | ボトムアップ推論の可能性 |
|---|------------|---------|----------------------|
| 1 | `expressions/patterns.rs:143-146` | typeof 被演算子型不明 | **有**: typeof 文字列自体が型情報 |
| 2 | `expressions/patterns.rs:244-250` | `in` 演算子の対象型不明 | **有**: フィールド名 → 型候補 |
| 3 | `expressions/patterns.rs:277-283` | instanceof 左辺型不明 | **有**: クラス名 → 型情報 |

## フォールバックのカスケード分析

### カスケードパス 1: パラメータ → 関数本体 → 生成コード

```
[A-21] 関数パラメータ注釈変換失敗 → RustType::Any
  ↓
TypeResolver がパラメータを Any としてスコープ登録
  ↓
関数本体でパラメータ使用時:
  - メンバーアクセス `param.field` → [D-10] Unknown（Any のフィールド型不明）
  - メソッド呼び出し `param.method()` → [D-15] Unknown（Any のメソッド不明）
  - 関数引数 `fn(param)` → 引数の expected type が Any → 下流も Any
  ↓
[B-3] Generator: パラメータ型 = serde_json::Value
  + 本体の変数型も連鎖的に serde_json::Value
```

**影響範囲**: 1つのパラメータの Any が、その関数本体の全ての関連式に波及する。

### カスケードパス 2: null リテラル → Option の inner 型

```
[A-17] null リテラル → Option(Any)
  ↓
変数 `const x = someExpr || null` の型 = someExpr の型 OR Option(Any)
  ↓
TypeResolver が変数型を Union(T, Option(Any)) として登録
  ↓
union の生成: enum { T(T), Option(serde_json::Value) }  ← 不要に複雑
```

**expected type が利用可能な場合**: `const x: string | null = null` → `Option<String>` で正確。

### カスケードパス 3: mapped type → HashMap → 全ての使用箇所

```
[C-1] TsMappedType → HashMap<String, V>
  ↓
Simplify<T> = { [K in keyof T]: T[K] } → HashMap<String, Value>  ← T が正解
  ↓
type Foo = Simplify<{ name: string }> → HashMap<String, Value>  ← { name: String } が正解
  ↓
関数パラメータ型で使用: (x: Foo) → x: HashMap<String, Value>
  ↓
x.name アクセス → HashMap の get → Option<&Value>  ← String が正解
```

**影響範囲**: mapped type の不正確な変換が、その型を使用する全箇所に波及する。

### カスケードパス 4: コールバックパラメータ → 本体全体

```
[A-24] arr.map(item => item.name)
  ↓
arr の型 = Vec<SomeStruct>（解決済み）
map のコールバック型 = (item: SomeStruct) => U（シグネチャ上）
  ↓
しかし item の型注釈なし → [A-24] item: Any
  ↓
item.name → [D-10] Unknown
  ↓
return item.name → Unknown → 型注釈必要
```

**コールバックパラメータの Any は、コールバック本体の全式に波及する。**

### カスケードパス 5: クラスプロパティ → メソッド全体

```
[A-25] class Foo { private bar; }  ← 注釈なし → Any
  ↓
this.bar アクセス → serde_json::Value
  ↓
this.bar.baz → serde_json::Value に .baz は存在しない → コンパイルエラー
```

## ボトムアップ推論の適用可能性サマリ

### フォールバック生成サイトの分類

| 分類 | サイト数 | ボトムアップで改善可能 | 具体例 |
|------|---------|---------------------|--------|
| **TS の `any`/`unknown` 型** | 5 (A-1,9,14,15,16) | **有**: 利用パターンから具体型を推論 | `any` パラメータに `.length` アクセス → string or array |
| **型注釈なし（パラメータ、プロパティ）** | 10 (A-3,4,6,7,8,11,12,13,21-28) | **有**: 呼び出し側引数型、使用パターン | callback パラメータ型、代入パターン |
| **型変換失敗** | 5 (A-2,5,10,D-2,3) | **限定的**: 変換自体の改善が先 | conditional type 失敗 → 構造分析 |
| **式型解決失敗（Unknown）** | 18 (D-1〜18) | **有**: 利用箇所の型情報、プロパティ名 | 三項分岐型不一致 → union 生成 |
| **null/undefined** | 4 (A-15,17,18,19) | **有**: expected type の inner 型 | null → Option<ExpectedType> |
| **mapped type** | 1 (C-1) | **有**: identity 検出、constraint 解決 | `Simplify<T>` → `T` |
| **todo!() 生成** | 3 (E-1,2,3) | **有**: ガード情報自体が型情報 | typeof "string" → String |

### 改善可能性の高い箇所（推定インパクト順）

1. **コールバックパラメータ型推論**（A-24 + カスケード 4）: 単一サイトの修正で、コールバック本体全体の型精度が向上。Hono のミドルウェアパターン（`(c, next) => { ... }`）に広く適用
2. **null の inner 型推論**（A-17 + カスケード 2）: expected type から `Option<T>` の T を推論。広範囲に適用
3. **三項演算子の union 化**（D-6）: 分岐型不一致 → Unknown ではなく union 生成
4. **配列メソッド要素型**（L カテゴリ + カスケード 4）: `Vec<T>.push(x)` → x の expected type = T
5. **mapped type の identity 検出**（C-1 + カスケード 3）: `{ [K in keyof T]: T[K] }` → T（I-221 で対応予定）
6. **`any` パラメータの narrowing 拡張**（A-1 + カスケード 1）: any_narrowing を typeof/instanceof 以外にも拡張（メンバーアクセスパターン等）

## 参照

全箇所のソースコード位置は本レポート内の表に記載。主要ファイル:
- `src/pipeline/type_converter/` — 型変換フォールバック（A-1〜10）
- `src/pipeline/type_resolver/` — 型解決フォールバック（A-17〜21, D-1〜18）
- `src/transformer/` — IR 生成フォールバック（A-22〜30, B-1〜2, E-1〜3）
- `src/registry/` — 型情報収集フォールバック（A-11〜13）
- `src/external_types/` — ビルトイン型フォールバック（A-14〜16）
- `src/generator/types.rs:52` — `RustType::Any` → `"serde_json::Value"` の最終変換
