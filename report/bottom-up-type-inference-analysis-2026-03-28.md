# ボトムアップ型推論の可能性分析（拡張版）

**日付**: 2026-03-28（2026-03-30 更新: 実装済みパターンの反映）
**Base commit**: 4f1c76a（調査時点）
**対象**: エラー＋フォールバックで不正確な型を付与している全箇所
**関連**: `report/fallback-type-inventory-2026-03-28.md` — 全フォールバック箇所のインベントリ（行番号は調査時点のもの）

> **⚠️ 注意**: 本レポートの行番号・件数は base commit 時点のもの。コード参照時は最新ソースを確認すること。設計的価値（Sink-Source モデル、10 の Sink パターン分類）は有効。

## 要約

本調査は、エラーとして報告される 40 件だけでなく、**現在「成功」しているが `Any`（→`serde_json::Value`）、`HashMap<String, V>`、`Unknown`、`todo!()` 等の不正確な型を付与しているケース全体**を対象に、ボトムアップ推論（値の利用箇所から型を逆推論）の適用可能性を分析する。

パイプライン全体で **50+ 箇所の不正確な型付与**が確認された。これらは以下の統一原理で改善可能:

> **「値が最終的に流れ込む先の型（sink type）から、その値の expected type を逆伝播する」**

この原理は、エラー解消だけでなく、**生成コード全体の型精度向上**に適用できる。

## 不正確な型付与の全体像

### 層 1: エラー（変換失敗）— 40 件

OBJECT_LITERAL_NO_TYPE として報告される。前回レポートで分析済み。

### 層 2: `RustType::Any` フォールバック（→ `serde_json::Value`）— 17+ 箇所

変換は「成功」するが、生成される Rust コードで `serde_json::Value` が使われ、型安全性が失われる。

| # | 箇所 | パターン | 利用可能な追加情報 |
|---|------|---------|------------------|
| 1 | `type_converter/mod.rs:105-106` | `any`/`unknown` キーワード型 | 値の利用箇所から具体的な型を推論可能（例: `any` パラメータが `.length` アクセスされる → string or array） |
| 2 | `type_converter/mod.rs:159-170` | TsMappedType の value type 変換失敗 | mapped type の constraint + key type 情報 |
| 3 | `type_converter/interfaces.rs:158-164` | call signature パラメータ無注釈 | 他パラメータ型、戻り値型、呼び出しパターン |
| 4 | `type_converter/interfaces.rs:168-179` | rest パラメータ無注釈 → `Vec<Any>` | 呼び出し側の実引数型 |
| 5 | `type_converter/type_aliases.rs:32-35` | conditional type true branch 失敗 | false branch、check type、extends type の構造 |
| 6 | `type_converter/type_aliases.rs:216-225` | 関数型リテラルのパラメータ無注釈 | 戻り値型、他パラメータ型 |
| 7 | `type_converter/unions.rs:495-510` | union の `any`/`object` メンバー | 他 union メンバーとの関係 |
| 8 | `type_converter/utilities.rs:432-438` | 変換不能な union メンバー → `Other(Any)` | 元の AST ノード構造 |
| 9 | `type_resolver/expressions.rs:37-39` | null リテラル → `Option<Any>` | **expected type から内側の型を推論可能**（例: `null as string` → `Option<String>`） |
| 10 | `type_resolver/expressions.rs:97-99` | 三項演算子の片方が Unknown → `Option<Any>` | **もう片方の分岐型を使用可能** |
| 11 | `type_resolver/expressions.rs:355-357` | optional chaining 結果 Unknown → `Option<Any>` | プロパティ名から TypeRegistry 参照 |
| 12 | `type_resolver/visitors.rs:46-54` | 関数パラメータ無注釈 → `Any` | **呼び出し側の実引数型から逆推論可能** |
| 13 | `transformer/functions/params.rs:31-35` | パラメータ型未解決 → `Any` | 関数本体での使用パターン |
| 14 | `transformer/functions/arrow_fns.rs:85-91` | アロー関数パラメータ → `Any` → any_narrowing | **親関数のパラメータ型から callback パラメータ型を推論可能** |
| 15 | `transformer/classes/members.rs:62` | クラスプロパティ無注釈 → `Any` | コンストラクタ代入、メソッド使用 |
| 16 | `transformer/classes/members.rs:181` | param property 無注釈 → `Any` | プロパティの使用箇所 |
| 17 | `transformer/functions/destructuring.rs:17-25` | 分割代入パラメータ無注釈 → `serde_json::Value` | 関数本体でのフィールドアクセスパターン |

### 層 3: `HashMap<String, V>` フォールバック — 2 箇所

mapped type の変換で常に `HashMap<String, V>` を生成。identity mapped type `{ [K in keyof T]: T[K] }` でさえ `HashMap` になる。

| # | 箇所 | パターン | より良い型 |
|---|------|---------|-----------|
| 1 | `type_converter/mod.rs:159-170` | `TsMappedType` → `HashMap<String, V>` | identity → `T`、key remapping → filtered struct、conditional value → conditional type alias |
| 2 | `type_converter/type_aliases.rs:271-283` | index signature `{ [key: string]: T }` → `HashMap<String, T>` | これは正確（HashMap が正しい表現） |

### 層 4: `ResolvedType::Unknown` 伝播 — 9+ 箇所

TypeResolver が式の型を解決できず Unknown を返す。下流の全ての型判定に波及する。

| # | 箇所 | パターン | 利用可能な追加情報 |
|---|------|---------|------------------|
| 1 | `call_resolution.rs:15-19` | 非 Expr callee（super, import） | super の場合は親クラス constructor |
| 2 | `call_resolution.rs:29-36` | 関数名がスコープに不在 | import 先の型定義、external types |
| 3 | `expressions.rs:50-52` | `as T` 型変換の型解決失敗 | 型名から TypeRegistry 参照 |
| 4 | `expressions.rs:119` | 三項演算子の分岐型不一致 | **両分岐の型から union を生成可能** |
| 5 | `expressions.rs:137-138` | Unknown オブジェクトのメンバーアクセス | プロパティ名から可能な型を推論 |
| 6 | `expressions.rs:394` | catch-all デフォルト | 式の構造から推論 |
| 7 | `visitors.rs:113-119` | 注釈なしパラメータ | **呼び出し側の引数型** |
| 8 | `visitors.rs:162-163` | 注釈・初期化なし変数 | **後続の代入・使用パターン** |
| 9 | `visitors.rs:198-199` | 注釈なしクラスプロパティ | コンストラクタ代入 |

### 層 5: `todo!()` 生成 — 3 パターン

型が不明なため変換を断念し、コンパイルエラーとなる `todo!()` を出力する。

| # | 箇所 | パターン | ボトムアップ推論の可能性 |
|---|------|---------|----------------------|
| 1 | `expressions/patterns.rs:143-146` | `typeof x` で x の型不明 → `todo!()` | typeof 文字列自体が型情報を持つ（`"string"` → String） |
| 2 | `expressions/patterns.rs:244-250` | `"key" in obj` で obj の型不明 → `todo!()` | `key` 名から可能な型を推論 |
| 3 | `expressions/patterns.rs:277-283` | `x instanceof Class` で x の型不明 → `todo!()` | Class 名から型情報を逆引き |

## ボトムアップ推論の統一原理

### Sink-Source 逆伝播モデル

全ての不正確な型付与は、以下の統一的なモデルで説明できる:

```
Source（値の生成箇所）  ─── 値の流れ ───→  Sink（値の消費箇所）
                                              │
                                              │ 型情報の逆伝播
                                              ↓
                                        Source の expected type を推論
```

### 10 の Sink パターン（網羅的リスト）

| # | Sink パターン | 型情報の源泉 | 対象層 | 推定改善件数 |
|---|-------------|------------|--------|------------|
| **S1** | return 文 | 関数戻り値型注釈 | 層1(G:9), 層2(#12,14) | ~12 |
| **S2** | `\|\|`/`??` の右辺 | 左辺の解決済み型 | 層1(H:8) | ~8 |
| **S3** | 関数/メソッド引数 | パラメータ型シグネチャ | 層1(D:9), 層2(#3,4,6), 層4(#7) | ~15 |
| **S4** | 配列メソッド引数 | `Vec<T>` の要素型 | 層1(L:3) | 3 |
| **S5** | 代入 RHS | LHS の型（変数型 or フィールド型） | 層1(K:1), 層4(#8) | ~3 |
| **S6** | `as T` 式 | T の型 | 層1(F:2), 層4(#3) | ~3 |
| **S7** | typeof/instanceof ガード | ガード文字列/クラス名 | 層5(全3), 層2(#1) | ~4 |
| **S8** | プロパティアクセス | フィールド名 + TypeRegistry | 層2(#11,15,16), 層4(#5) | ~5 |
| **S9** | 三項演算子の対分岐 | もう片方の分岐型 | 層2(#10), 層4(#4) | ~3 |
| **S10** | コールバックパラメータ | 親関数のパラメータ型のシグネチャ | 層2(#14,17), 層4(#7) | ~8 |

### Sink パターンの詳細設計

#### S1: return 文 → 関数戻り値型

**現状**: `visitors.rs:404-408` で `current_fn_return_type` を return 式に伝播。

**不足点**:
- **コールバック関数の戻り値型**: 外側関数のパラメータが `callback: (x: T) => ReturnType` の場合、内側コールバックの `current_fn_return_type` に `ReturnType` が設定されない
- 根本原因: `visit_arrow_expr`/`visit_fn_expr` が `current_fn_return_type` を設定する際、明示的な戻り値型注釈がない場合は None。外側コンテキストの expected type からコールバック戻り値型を推論する仕組みがない

**改善設計**:
```
visit_call_expr で callback 引数を検出
  → callback パラメータの関数型シグネチャを取得
  → 戻り値型を抽出
  → callback 本体の visit 時に current_fn_return_type として設定
```

対象コード: `visitors.rs` の `visit_arrow_expr` / `visit_fn_expr` + `call_resolution.rs`

#### S3: 関数/メソッド引数 → パラメータ型（拡張）

**現状**: `set_call_arg_expected_types`（`call_resolution.rs:101-190`）が Ident/Member callee の引数に expected type を設定。

**不足点**:
- パラメータ型がジェネリクス（`E['Bindings'] | {}`）の場合、Named struct に解決されず propagation が止まる
- 具体化: 呼び出し側の型引数（`app.fetch<ConcreteType>(...)` 等）から型パラメータを解決し、パラメータ型を具体化する必要がある

**改善設計**: 呼び出し側の実引数から型引数を推論する（TypeScript の type argument inference 相当）。これは高度だが、単純なケース（型引数が 1 つで、引数から直接推論可能）から段階的に対応可能。

#### S4: 配列メソッド引数 → `Vec<T>` の要素型

**現状**: `lookup_method_sigs`（`call_resolution.rs:212-229`）は `TypeDef::Struct.methods` を検索。`Vec<T>` は Struct として登録されていないため、`push`/`unshift` のシグネチャが見つからない。

**改善設計**:
```
lookup_method_params に Vec 特殊処理を追加:
  if obj_type == Vec<T> && method_name ∈ {"push", "unshift", "splice", ...}:
    return vec![T]  // 要素型をパラメータ型として返す
```

`call_resolution.rs:236-246` の `lookup_method_params` に `Vec` ビルトインメソッドのパラメータ型を返す分岐を追加。

#### S7: typeof/instanceof ガード → ガード文字列/クラス名

**現状**: `expressions/patterns.rs` で `typeof x` の x の型が不明な場合 `todo!()` を生成。

**改善設計**: typeof 文字列自体が型情報を持つ:
- `typeof x === "string"` → x は String
- `typeof x === "number"` → x は f64
- `typeof x === "boolean"` → x は bool
- `typeof x === "object"` → x は serde_json::Value（or 具体型）
- `instanceof Class` → x は Class 型

この情報は TypeResolver の narrowing_events に既に存在するが、**narrowing_events の設定前に型が必要な箇所**（typeof 式自体の変換時）では利用できない。

**解決**: typeof 式の変換時に、被演算子の型を narrowing_events ではなくガード文字列から直接推論。

#### S10: コールバックパラメータ → 親関数のパラメータ型シグネチャ

**現状**: アロー関数/関数式のパラメータに型注釈がない場合、`Any` として処理。

**不足点**: `arr.map(item => ({ name: item.name }))` のような場合:
- `arr` は `Vec<SomeStruct>` — 型は解決済み
- `map` のコールバックパラメータ型は `SomeStruct` — シグネチャ上は `(item: T) => U`
- しかし `item` の型は `Any` として処理される

**改善設計**:
```
set_call_arg_expected_types で callback 引数を検出:
  if arg_i は FnExpr/ArrowExpr:
    param_type_i = パラメータ型シグネチャの i 番目
    if param_type_i は Fn { params, return_type }:
      callback の各パラメータに params[j] の型を設定
      callback の current_fn_return_type に return_type を設定
```

## 設計判断: システム vs アドホック

### 結論: **統一フレームワークとして構築すべき**

理由:

1. **10 の Sink パターンは全て同一の原理**（sink type → source expected type の逆伝播）に基づく。アドホックに個別実装すると、同じ propagation ロジックが 10 箇所に散在する
2. **新パターンの追加が宣言的になる**: 新しい sink を発見した場合、フレームワークに 1 つのルールを追加するだけ
3. **影響範囲が広い**: エラー 40 件 + フォールバック 50+ 箇所 = 90+ 箇所に影響。個別修正では一貫性を保てない

### アーキテクチャ設計

**修正方針**: 既存の TypeResolver のシングルパスを維持しつつ、`propagate_expected` の拡張として実装。2 パス目は不要。

```
[既存] TypeResolver::resolve_file()
  │
  ├── visit_stmt() → visit_expr() → resolve_expr()
  │     │
  │     ├── [既存] トップダウン: expected_types.insert(span, type)
  │     ├── [既存] ボトムアップ: expr_types.insert(span, type)
  │     │
  │     └── [新規] Sink 検出時の逆伝播:
  │           ├── S1: visit_return_stmt → callback 戻り値型推論
  │           ├── S3: set_call_arg_expected_types → ジェネリクス具体化
  │           ├── S4: set_call_arg_expected_types → Vec メソッド要素型
  │           ├── S5: visit_assign_expr → LHS フィールド型 → RHS expected
  │           ├── S7: convert_typeof → ガード文字列から被演算子型
  │           └── S10: set_call_arg_expected_types → callback パラメータ型
  │
  └── [結果] FileTypeResolution
        ├── expected_types: HashMap<Span, RustType>  ← より多くのエントリ
        ├── expr_types: HashMap<Span, RustType>      ← より正確な型
        └── (その他は変更なし)
```

**重要**: この設計は**既存のシングルパスの中で**実装可能。なぜなら:
- S1（return）: return 文の visit 時に `current_fn_return_type` は既に設定済み — コールバック検出の追加のみ
- S3（関数引数）: `set_call_arg_expected_types` は引数 visit 前に呼ばれる — ジェネリクス具体化の追加のみ
- S4（配列メソッド）: 同上 — Vec メソッド分岐の追加のみ
- S5（代入）: 代入文 visit 時に LHS 型は解決済み — RHS への propagation 追加のみ
- S7（typeof）: typeof 式の変換時にガード文字列は既知 — 被演算子型の推論追加のみ
- S10（コールバック）: S3 と同じタイミング — callback パラメータ型の設定追加のみ

**2 パス目が必要なケース**: `const x = {}; x.field = value` のように、宣言後の使用パターンから型を逆推論する場合のみ。Hono ベンチマークでは **2 件のみ**（`utils/html.ts:129`, `validator/validator.ts:46`）。費用対効果が低いため 2 パス目は不要。

### 実装優先度（2026-03-30 更新: 実装済みステータス反映）

| 優先度 | Sink パターン | 改善件数 | 難易度 | ステータス |
|--------|-------------|---------|--------|-----------|
| **最高** | S10: コールバックパラメータ型推論 | ~20 | 中 | 未着手 |
| **最高** | S4: Vec メソッド要素型 | 3 | 低 | ✅ I-289（B-0a）で実装済み |
| **高** | S1: return 文（コールバック戻り値型） | ~12 | 中 | 未着手 |
| **高** | S5: 代入 RHS → LHS 型 | ~4 | 低 | 未着手 |
| **高** | S6: `as T` 型逆伝播 | ~3 | 低 | 未着手 |
| **中** | S9: 三項演算子対分岐型 | ~3 | 低 | 未着手 |
| **中** | S7: typeof/instanceof ガード | ~4 | 中 | 未着手 |
| **中** | S8: プロパティアクセス | ~5 | 中 | 未着手 |
| **低** | S2: `\|\|`/`??` ジェネリクス制約解決 | ~8 | 高 | 部分実装: I-286c S2（C-2: 型パラメータ制約解決） |
| **低** | S3: 型引数推論 | ~15 | 高 | 部分実装: I-286c S3（C-3: 明示的/推論型引数） |

**注**: S10 と S1 は重複（コールバック内の return）。S10 を実装すれば S1 の大部分も解消。

### I-221 との関係

I-221 の intersection フォールバック（`convert_ts_type` 失敗時の `RustType::Any`）に対して:
- **直接的な改善なし**: intersection メンバー型の変換失敗は「型推論」ではなく「型変換」の問題（I-285/I-200 で解決）
- **間接的な改善あり**: I-221 で intersection が変換成功するようになった後、その結果型を利用する箇所（関数引数、return 文等）で sink-source 逆伝播が効く

## 参照

### 層 2: Any フォールバック箇所
- `src/pipeline/type_converter/mod.rs:105-106,159-170` — any/unknown キーワード、mapped type
- `src/pipeline/type_converter/interfaces.rs:158-164,168-179` — call signature パラメータ
- `src/pipeline/type_converter/type_aliases.rs:32-35,216-225` — conditional type、関数型リテラル
- `src/pipeline/type_converter/unions.rs:495-510` — union の any/object メンバー
- `src/pipeline/type_resolver/expressions.rs:37-39,97-99,355-357` — null、三項、optional chaining
- `src/pipeline/type_resolver/visitors.rs:46-54` — 関数パラメータ
- `src/transformer/functions/params.rs:31-35,169-170` — パラメータ型未解決
- `src/transformer/functions/arrow_fns.rs:85-91` — アロー関数パラメータ
- `src/transformer/classes/members.rs:62,181` — クラスプロパティ
- `src/transformer/functions/destructuring.rs:17-25` — 分割代入パラメータ

### 層 4: Unknown 伝播箇所
- `src/pipeline/type_resolver/call_resolution.rs:15-19,29-36` — callee 解決失敗
- `src/pipeline/type_resolver/expressions.rs:50-52,119,137-138,394` — 各種式の型解決失敗
- `src/pipeline/type_resolver/visitors.rs:113-119,162-163,198-199` — 宣言の型解決失敗

### 層 5: todo!() 生成箇所
- `src/transformer/expressions/patterns.rs:143-146,244-250,277-283` — typeof/in/instanceof
