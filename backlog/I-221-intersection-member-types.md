# I-221: intersection メンバー型の網羅的サポート

## Background

`extract_intersection_members`（`src/pipeline/type_converter/intersections.rs:8-78`）は `TsTypeLit`、`TsTypeRef`、`TsKeywordType` の 3 種類の AST ノードのみを処理し、それ以外を一律 `"unsupported intersection member type"` エラーにしている。一方、TypeScript の intersection 型には任意の型を含めることができ、Hono ベンチマークでは TsMappedType（5件）、TsUnionType（3件）、TsConditionalType（1件、TsParenthesized 内）が未対応でエラーとなっている。

`convert_ts_type`（`mod.rs:94-230`）は TsMappedType、TsUnionType、TsConditionalType、TsParenthesizedType を含む幅広い型を変換可能。intersection メンバー処理でもこの能力を活用すべきである。

また、TypeScript の `& {}` パターン（空オブジェクトとの intersection、型表示の簡略化トリック）がフィルタリングされておらず、`{ [K in keyof T]: T[K] } & {}` のような型が intersection として処理されて不要に失敗している。

## Goal

1. Hono ベンチマークの `INTERSECTION_TYPE` カテゴリが **9 → 0 件**
2. `A & (B | C)` パターンが数学的に正しい分配（distribution）で enum に変換される
3. `T & {}` パターンの空オブジェクトが除去され、単一メンバーとして正しく変換される
4. `convert_ts_type` で変換可能な任意の型が intersection メンバーとして処理可能

## Scope

### In Scope

- `extract_intersection_members` の前処理（空 TsTypeLit 除去、TsParenthesized アンラップ）
- 単一メンバー簡約（フィルタ後 1 メンバーなら intersection ではなく直接変換）
- union-in-intersection の分配法則変換（`A & (B | C)` → enum `(A ∩ B) | (A ∩ C)`）
- `convert_ts_type` による汎用フォールバック（mapped type、conditional type 等）
- 型エイリアス位置（`try_convert_intersection_type`）とアノテーション位置（`convert_intersection_in_annotation`）の両方

### Out of Scope

- identity mapped type の簡約（`{ [K in keyof T]: T[K] }` → `T`）— I-200 スコープ
- mapped type のフィールド抽出 — I-200 スコープ
- conditional type の完全な評価 — 既存の `convert_conditional_type` フォールバックで対応

## Design

### Technical Approach

#### 1. 前処理: 空 TsTypeLit 除去 + TsParenthesized アンラップ

intersection のメンバーリストに対して:
1. `TsParenthesizedType` を再帰的にアンラップ
2. メンバー 0 件の `TsTypeLit`（`{}` 空オブジェクト）を除去
3. 残り 1 件なら intersection ではない → 当該型を直接変換

**型エイリアス位置**（`try_convert_intersection_type`）: 単一メンバーの場合:
- Identity mapped type（`{ [K in keyof T]: T[K] }`）を検出 → type param `T` の `RustType::Generic` として返す（`type Simplify<T> = T`）
- それ以外: `convert_ts_type` で変換し `Item::TypeAlias` として返す。変換失敗時は `RustType::Any` フォールバック

**アノテーション位置**（`convert_intersection_in_annotation`）: 同様のロジック。

**Identity mapped type の検出条件**:
1. `mapped.name_type` が None（key remapping なし）
2. constraint が `keyof T`（`TsTypeOperator(KeyOf, TsTypeRef(T))` 形式）
3. value type が `T[K]`（`TsIndexedAccessType` で obj = constraint の T、key = mapped type param）
4. readonly/optional modifier が None

この検出は I-200（general mapped type）とは独立。identity mapped type は型パラメータに関係なく常に T と等価であり、mapped type の「変換」ではなく「簡約」。

#### 2. union-in-intersection の分配法則変換

`A & (B | C)` の数学的に正しいセマンティクスは `(A & B) | (A & C)` — 分配法則。

**Rust 表現**: 各分配結果を enum variant として表現する。ベースフィールド（非 union メンバーから抽出）を各 variant にマージする。

```typescript
// TypeScript
type MethodOverrideOptions = {
  app: Hono<any, any, any>
} & (
  | { form?: string; header?: never; query?: never }
  | { form?: never; header: string; query?: never }
  | { form?: never; header?: never; query: string }
)
```

```rust
// Rust: 分配法則適用
enum MethodOverrideOptions {
    Variant0 { app: Hono, form: Option<String> },
    Variant1 { app: Hono, header: String },
    Variant2 { app: Hono, query: String },
}
```

**重要**: struct + enum フィールド方式（`struct { base_fields, _variant: Enum }`）は serde serialization で不正確。元の TypeScript では全フィールドが同一レベルに存在するため、分配法則でフラット化した enum が正しい。

**実装**:
1. メンバーを union / 非 union に分類
2. 非 union メンバーからベースフィールドを抽出（既存ロジック）
3. 非抽出可能な非 union メンバー（mapped, conditional 等）は `convert_ts_type` で変換し embedded フィールドとしてベースに追加
4. union メンバーの各 variant について:
   - TsTypeLit: フィールドを抽出
   - TsTypeRef: レジストリ解決 → フィールド抽出、または data として格納
   - 空 TsTypeLit: ベースフィールドのみの variant
   - その他: `convert_ts_type` で変換し data として格納
5. 各 variant にベースフィールドをマージ
6. 型エイリアス位置: `Item::Enum` を返す
7. アノテーション位置: 合成 enum を生成し `RustType::Named` を返す

**制約**: union メンバーが複数ある場合（`A & (B|C) & (D|E)`）は組み合わせ爆発するため、最初の union のみ分配し、残りは embedded フィールドにフォールバック。

**discriminant 検出**: 全 variant が TsTypeLit の場合、共通 discriminant フィールドを検索し、見つかれば `serde(tag)` 付き enum を生成。既存の `find_discriminant_field` ロジックを再利用。

#### 3. 汎用フォールバック

union メンバーが存在しない intersection（struct 出力）において、TsTypeLit/TsTypeRef/TsKeywordType 以外のメンバーに対して `convert_ts_type` で変換を試み、embedded フィールド `_i` として追加:

- TsMappedType → `HashMap<String, V>` フォールバック
- TsConditionalType → `convert_conditional_type` true branch フォールバック
- その他 → `convert_ts_type` 結果

**`convert_ts_type` 失敗時は `RustType::Any`（`serde_json::Value`）にフォールバック**。これにより:
- `RequiredRequestInit = Required<Omit<...>> & { [Key in ...]: RequestInit[Key] }` のように mapped type 内の indexed access が型パラメータキー（I-285 スコープ）で変換不可でも、intersection 全体の変換は成功する
- フィールド `_i: serde_json::Value` は型情報が欠落していることを明示するプレースホルダー
- I-200/I-285 の実装後、このフォールバックは自然に置き換わる

これは、フィールドの静的抽出が不可能な型（ジェネリック mapped type 等）に対する唯一の正しい表現。完全に変換不可能な場合の `Any` フォールバックは、変換全体を失敗させるよりも情報量が多い（ベースフィールドは保持される）。

### Design Integrity Review

- **Higher-level consistency**: `convert_type_alias` の変換試行順序（string literal → discriminated union → general union → intersection → ...）は変更不要。intersection 内の union 検出は intersection ハンドラの責務として正しい（外側は intersection 型なので union ハンドラは呼ばれない）
- **DRY**:
  - `find_discriminant_field` を `pub(super)` に昇格し、intersection 内 union の discriminant 検出に再利用
  - `extract_variant_info` も同様に再利用可能にする
  - ベースフィールド抽出ロジックは `extract_intersection_members` 内に留まる（intersection 固有）
- **Orthogonality**: union 分配ロジックは intersection モジュール内の新関数として実装。union モジュールの変更は `find_discriminant_field` / `extract_variant_info` の可視性変更のみ
- **Coupling**: `convert_ts_type` への依存追加はフォールバックパスのみ。`convert_ts_type` は既に `convert_intersection_in_annotation` から呼ばれているため、新規依存ではない
- **Broken windows**: `extract_intersection_members` の `TsTypeRef` 分岐（line 37-67）で `swc_ecma_ast::TsEntityName::Qualified` が未処理で `"unsupported qualified type name in intersection"` エラーを返す。これは I-36（qualified type name）の対象だが、intersection 内での qualified name はベンチマークでは未検出のため TODO に記録済み。本 PRD では対象外

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `src/pipeline/type_converter/intersections.rs` | `extract_intersection_members` 前処理追加、汎用フォールバック追加、union 分配関数追加 |
| `src/pipeline/type_converter/intersections.rs` | `try_convert_intersection_type` 前処理呼び出し + union 分岐 |
| `src/pipeline/type_converter/intersections.rs` | `convert_intersection_in_annotation` 前処理呼び出し + union 分岐 |
| `src/pipeline/type_converter/unions.rs` | `find_discriminant_field`, `extract_variant_info` の可視性を `pub(super)` に変更 |
| テスト | `tests/fixtures/intersection-*.input.ts` + snapshot 追加 |

## Task List

### T1: 前処理 — 空 TsTypeLit 除去 + TsParenthesized アンラップ + 単一メンバー簡約 + identity mapped type 検出

- **Work**:
  1. `intersections.rs` に `fn unwrap_parenthesized(ty: &TsType) -> &TsType` ヘルパー追加（再帰的アンラップ）
  2. `intersections.rs` に `fn is_empty_type_lit(ty: &TsType) -> bool` ヘルパー追加
  3. `intersections.rs` に `fn try_simplify_identity_mapped_type(mapped: &TsMappedType) -> Option<RustType>` 追加:
     - constraint が `keyof T` 形式かチェック（`TsTypeOperator(KeyOf, TsTypeRef(ident))`）
     - value type が `T[K]` 形式かチェック（`TsIndexedAccessType` で obj = T, key = mapped param）
     - `name_type` が None かチェック（key remapping なし）
     - 全条件合致: `Some(RustType::Generic(T.to_string()))` を返す
  4. `try_convert_intersection_type` 冒頭で:
     - intersection.types をアンラップ + 空 TsTypeLit 除去してフィルタ
     - 残り 1 件の場合:
       - TsMappedType: `try_simplify_identity_mapped_type` → 成功なら `Item::TypeAlias { ty: T }` を返す
       - それ以外 or identity 検出失敗: `convert_ts_type` → 成功なら `Item::TypeAlias`、失敗なら `Item::TypeAlias { ty: RustType::Any }`
     - 残り 0 件: `Err`
  5. `convert_intersection_in_annotation` 冒頭で同様のフィルタ:
     - 残り 1 件: identity mapped type チェック → `convert_ts_type` → RustType 直接返却
  6. テスト: `intersection-empty-object.input.ts` で:
     - `type Simplify<T> = { [K in keyof T]: T[K] } & {}` → `type Simplify<T> = T`
     - `type WithEmpty = { x: number } & {}` → `struct WithEmpty { x: f64 }`（空 TsTypeLit 除去後、通常 intersection 処理）

- **Completion criteria**:
  - `type Simplify<T> = { [K in keyof T]: T[K] } & {}` が `type Simplify<T> = T` に変換される（identity mapped type 検出）
  - `type SimplifyDeep<T> = { [K in keyof T]: T[K] } & {}` が `type SimplifyDeep<T> = T` に変換される
  - `type WithEmpty = { x: number } & {}` が `struct WithEmpty { pub x: f64 }` に変換される
  - `cargo test` パス + snapshot 一致

- **Depends on**: None

### T2: 汎用フォールバック — `convert_ts_type` による未対応メンバーの embedded 変換

- **Work**:
  1. `extract_intersection_members` のキャッチオール `_ =>` 分岐を修正:
     ```rust
     _ => {
         let rust_type = convert_ts_type(ty, synthetic, reg)
             .unwrap_or(RustType::Any);
         fields.push(StructField {
             vis: None,
             name: format!("_{i}"),
             ty: rust_type,
         });
     }
     ```
     `convert_ts_type` 失敗時は `RustType::Any` にフォールバック。理由: mapped type 内の indexed access が型パラメータキー（I-285 未実装）で失敗するケースがあり、intersection 全体を失敗させるよりもベースフィールドを保持して `serde_json::Value` フォールバックする方が情報量が多い
  2. `extract_intersection_members` のループ冒頭で `let ty_inner = unwrap_parenthesized(ty.as_ref());` を追加し、以降の match で `ty_inner` を使用
  3. テスト: `intersection-fallback.input.ts` で:
     - mapped type member → embedded `_i` フィールド
     - conditional type in annotation position → embedded フィールド
     - convert_ts_type 失敗ケース → `_i: serde_json::Value` フォールバック

- **Completion criteria**:
  - `RequiredRequestInit = Required<Omit<...>> & { [K in ...]: ... }` が struct に変換される（mapped type member が `_1: serde_json::Value` フィールド — indexed access with type param key は I-285 待ち）
  - `ContextVariableMap & (conditional)` がアノテーション位置で変換成功（conditional → HashMap 変換 or Any）
  - `cargo test` パス

- **Depends on**: T1（前処理ヘルパー）

### T3: union 分配法則 — `A & (B | C)` → enum

- **Work**:
  1. `unions.rs`: `find_discriminant_field` と `extract_variant_info` を `pub(super)` に変更
  2. `intersections.rs` に `fn distribute_intersection_with_union(...)` 新関数を追加:
     - 引数: ベースフィールド `Vec<StructField>`、union 型 `&TsUnionType`、synthetic, reg
     - 処理:
       a. union の各 variant を分類（TsTypeLit → フィールド抽出、TsTypeRef → レジストリ解決 or data、空 TsTypeLit → ベースのみ）
       b. 全 variant が TsTypeLit の場合: `find_discriminant_field` で discriminant 検出試行
       c. 各 variant にベースフィールドをマージし `EnumVariant { fields: base + variant_fields }` を生成
       d. discriminant 検出成功時: `serde(tag)` 付き enum。失敗時: untagged enum
     - 戻り値: `(Vec<EnumVariant>, Option<String>)` — (variants, serde_tag)
  3. `try_convert_intersection_type` に union 検出分岐を追加:
     - フィルタ済みメンバーから union メンバーを検出
     - union が 1 つ: `distribute_intersection_with_union` → `Item::Enum` を返す
     - union が 2+: 最初の union のみ分配、残りは embedded フィールドにフォールバック
  4. `convert_intersection_in_annotation` に同様の union 検出分岐:
     - 合成 enum を生成し `RustType::Named` を返す
  5. テスト:
     - `intersection-union-distribution.input.ts`: `type X = { base: string } & ({ a: number } | { b: boolean })` → enum with base field in each variant
     - discriminant 付き variant のテスト
     - TsTypeRef variant のテスト

- **Completion criteria**:
  - `MethodOverrideOptions`（base + 3 TsTypeLit variants）が enum に変換され、各 variant にベースフィールド `app` が含まれる
  - `APIGatewayProxyResult`（base + TsTypeRef union）が enum に変換される
  - `NetAddrInfo`（base + union with empty variant）が enum に変換され、空 variant はベースフィールドのみ
  - discriminant 検出時に `serde(tag)` が付与される
  - `cargo test` パス

- **Depends on**: T1, T2

### T4: E2E ベンチマーク検証

- **Work**:
  1. `cargo build --release && ./scripts/hono-bench.sh` 実行
  2. `python3 scripts/inspect-errors.py --category INTERSECTION_TYPE` で 0 件を確認
  3. 全体エラーインスタンス数の変化を確認（91 → 82 以下）
  4. 回帰確認: 既存カテゴリの件数に増加がないこと

- **Completion criteria**:
  - `INTERSECTION_TYPE` が 0 件
  - 全体エラーインスタンスが 82 以下
  - 既存カテゴリに回帰なし

- **Depends on**: T1, T2, T3

## Test Plan

### 単体テスト（fixture + snapshot）

| テストファイル | パターン | 検証内容 |
|-------------|---------|---------|
| `intersection-empty-object.input.ts` | `type A = { x: number } & {}` | 空 TsTypeLit 除去 → struct |
| `intersection-empty-object.input.ts` | `type B<T> = { [K in keyof T]: T[K] } & {}` | identity mapped type → `type B<T> = T` |
| `intersection-empty-object.input.ts` | `type C<T> = { [K in keyof T]: SomeTransform<T[K]> } & {}` | 非 identity → type alias (HashMap or Any fallback) |
| `intersection-fallback.input.ts` | `type C = Required<Omit<X, Y>> & { [K in Y]?: X[K] }` | mapped type member → embedded |
| `intersection-fallback.input.ts` | annotation 位置の conditional type in intersection | アノテーション位置フォールバック |
| `intersection-union-distribution.input.ts` | `type D = { base: T } & ({ a: U } \| { b: V })` | 分配法則 → enum |
| `intersection-union-distribution.input.ts` | `type E = { base: T } & (A \| B)` | TsTypeRef union variant |
| `intersection-union-distribution.input.ts` | `type F = { base: T } & ({ a: U } \| {})` | 空 variant → base のみ |
| `intersection-union-distribution.input.ts` | discriminant 付き union | serde(tag) 検証 |

### 境界値テスト

- 全メンバーが空 TsTypeLit: `type X = {} & {}` → 空 struct
- union のみ（ベースフィールドなし）: `type X = ({ a: T } | { b: U })` → これは union であり intersection ではない（`try_convert_intersection_type` に到達しない）
- 複数 union メンバー: `type X = { a: T } & (B | C) & (D | E)` → 最初の union のみ分配

### エラーケース

- `convert_ts_type` も失敗するメンバー: エラーメッセージが "unsupported intersection member type" であること

## Completion Criteria

1. `cargo test` 全パス（新規テスト含む）
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
3. `cargo fmt --all --check` パス
4. Hono ベンチマーク `INTERSECTION_TYPE` が **9 → 0 件**
5. 全体エラーインスタンスが **91 → 82 以下**
6. 回帰なし（既存カテゴリの件数が増加しないこと）

### コードパストレース検証（5 件）

1. **Item 8 (Simplify\<T\>)**: `{ [K in keyof T]: T[K] } & {}` → T1 前処理で `{}` 除去 → 単一メンバー → `try_simplify_identity_mapped_type` 検出: constraint=`keyof T`, value=`T[K]`, name_type=None → `RustType::Generic("T")` → `Item::TypeAlias { ty: T }` = `type Simplify<T> = T` ✓
2. **Item 1 (APIGatewayProxyResult)**: `{ fields } & (WithHeaders | WithMultiValueHeaders)` → T3 union 検出 → `distribute_intersection_with_union` → TsTypeLit フィールド 5 件をベースに、TsTypeRef union 2 variant → enum 2 variant with base fields ✓
3. **Item 3 (context.ts)**: `ContextVariableMap & (conditional)` → アノテーション位置 → T1 TsParenthesized アンラップ → T2 conditional member: `convert_ts_type(conditional)` → `convert_conditional_type` → true branch `Record<string, any>` → `HashMap<String, Value>` → embedded field `_1: HashMap<String, Value>` ✓
4. **Item 6 (RequiredRequestInit)**: `Required<Omit<...>> & { [Key in Props]?: RequestInit[Key] }` → 2 メンバー残存 → union なし → struct パス → T2 fallback: TsMappedType member → `convert_ts_type(mapped)` → value type `RequestInit[Key]` で Key=type param → `convert_indexed_access_type` fail → `unwrap_or(RustType::Any)` → `_1: serde_json::Value` フィールド ✓
5. **Item 5 (MethodOverrideOptions)**: `{ app } & ({ form } | { header } | { query })` → T3 union 検出 → base fields = `[app: Hono]` → union 3 variant (TsTypeLit) → discriminant 検索 → discriminant なし → 各 variant にベースフィールド app をマージ → untagged enum 3 variant ✓
