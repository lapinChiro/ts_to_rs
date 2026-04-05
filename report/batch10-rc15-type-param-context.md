# Batch 10: RC-15 型パラメータコンテキスト欠如 — 調査レポート

**Base commit**: `701b498`
**調査日**: 2026-04-05

---

## 概要

RC-15（`convert_ts_type` に型パラメータコンテキストがない）の根本原因を調査し、I-273（E0404: 非trait型の trait bound 使用）と I-299（合成 enum の型パラメータ欠如）の修正設計を策定した。

ディレクトリコンパイル失��は **`types.rs` 1ファ��ルのみ**（11エラー）。I-273 + I-299 の修正で 8/11 エラーが解消されるが、残り 3 エラー（E0405, E0107）は別系統のため、本バッチだけでは dir compile 156→157 にはならない。ただし RC-15 は後続バッチの型変換品質に影響する設計基盤であり、先行修正の価値は高い。

---

## 1. ディレクトリコンパイル失敗の全体像

`/tmp/hono-dir-compile-output.json` の解析結果:

| 行 | エラ��� | コード | 内容 | 原因 |
|----|--------|--------|------|------|
| 521 | E0404 | I-273 | `ArrayBufferView<TArrayBuffer: ArrayBufferOrSharedArrayBuffer>` — enum を trait bound に使用 |
| 979 | E0404 | I-273 | `Uint8Array<TArrayBuffer: ArrayBufferOrSharedArrayBuffer>` — 同上 |
| 1208 | E0404 | I-273 | `HonoRequest<P: String, ...>` — String (struct) を trait bound に使用 |
| 1208 | E0405 | 別系統 | `HonoRequest<..., I: Input::out>` — 修飾パス `Input::out` が trait として解決不可 |
| 1404 | E0404 | I-273 | `TypedResponse<T, U: StatusCode, F: ResponseFormat>` — struct を trait bound に使用（2件） |
| 386-387 | E0425 | I-299 | `enum TOrVecT { T(T), VecT(Vec<T>) }` — T がスコープ外 |
| 21 | E0107 | 別系統 | `ArrayBufferView(ArrayBufferView)` — ジェネリクス引数不足 |
| 427-428 | E0107 | 別系統 | `ExtractValidatorOutput<VF>` — 引数 0 に対し 1 が渡される |

### E0405 の原因

`I: Input::out` は TS の `I extends Input['out']`（indexed access type）に由来。`resolve_indexed_access` がフィールド型を解決できず、`Input::out` という修飾パスをそのまま出力。これは indexed access 解決の問題であり、RC-15 とは別系統。

### E0107 の原因

`ArrayBufferView(ArrayBufferView)` — 合成 union enum のバリアントが `ArrayBufferView` を型引数なしで参照。`ArrayBufferView<TArrayBuffer>` はジェネリクス 1 つを要求するが、union 登録時に型引数が省略された。型引数の自動推論/デフォルト設定が未実装。

---

## 2. I-273: 非trait制約の trait bound 使用

### 発生メカニズム

```
TS: T extends number
  ↓ collect_type_params (src/registry/collection.rs:437-453)
TsTypeInfo::Number
  ↓ resolve_type_params (src/ts_type_info/resolve/typedef.rs:18-36)
  ↓ resolve_ts_type (src/ts_type_info/resolve/mod.rs:43)
RustType::F64
  ↓ generate_type_params (src/generator/mod.rs:571-584)
  ↓ generate_type (src/generator/types.rs)
"T: f64"  ← E0404
```

### 影響箇所

型パラメータ制約が IR に設定される 2 つのパス:

1. **TypeDef 解決パス**: `resolve_type_params` (`src/ts_type_info/resolve/typedef.rs:18-36`)
   - `resolve_typedef` → struct/enum/function の type_params 解決で呼出
   - ��出元: `src/registry/collection.rs` 内の TypeDef 登録

2. **SWC AST 直接変換パス**: `extract_type_params` (`src/pipeline/type_converter/utilities.rs:6-25`)
   - `convert_interface`, `convert_type_alias` で呼出
   - constraint の convert_ts_type → `is_valid_trait_bound` フィルタが必要

**注意**: `collect_type_param_constraints` (`src/pipeline/type_resolver/helpers.rs:112-126`) は TypeResolver の型解決用（`type_param_constraints: HashMap<String, RustType>`）。コード生成用ではないのでフィルタ不要。

### TypeScript 制約パターンと Rust 対応

| TS 制約 | 解決後 RustType | Rust bound 有効性 | 対応 |
|---------|----------------|-------------------|------|
| `T extends number` | `F64` | ❌ primitive | 制約除外 |
| `T extends string` | `String` | ❌ primitive | 制約除外 |
| `T extends boolean` | `Bool` | ❌ primitive | 制約除外 |
| `T extends SomeInterface` | `Named("SomeTrait")` | ✅ trait | 維持 |
| `T extends SomeClass` | `Named("SomeClass")` | ❌ struct | 制約除外 |
| `T extends SomeEnum` | `Named("SomeEnum")` | ❌ enum | 制約除外 |
| `T extends UnknownType` | `Named("UnknownType")` | 不明 | trait と仮定 |

---

## 3. I-299: 合成 enum の型パラメータ欠如

### 発生メカニズム

```
TS: type Foo<T> = T | T[]
  ↓ resolve_ts_type → union::resolve_union
  ↓ register_union([Named("T"), Vec(Named("T"))])
  ↓ Item::Enum { name: "TOrVecT", variants: [...] }  ← type_params なし!
```

### IR の欠陥

`Item::Enum` に `type_params` フィールドが**存在しない** (`src/ir/mod.rs:422-431`):

```rust
Enum {
    vis: Visibility,
    name: String,
    serde_tag: Option<String>,
    variants: Vec<EnumVariant>,
    // ← type_params がない!
}
```

他の全 Item バリアント（Struct, Trait, Impl, TypeAlias, Fn）は `type_params: Vec<TypeParam>` を持つ。pipeline-integrity.md の「新フィールドは全バリアントに一貫適用」に反した状態。

### `register_union` の制約

`register_union` (`src/pipeline/synthetic_registry/mod.rs:81-128`) は:
- `member_types: &[RustType]` のみを受け取る
- 型パラメータスコープ情報がない
- `Named("T")` が型パラメータ参照か実型かを区別できない

---

## 4. `resolve_ts_type` の呼出構造

`resolve_ts_type` (`src/ts_type_info/resolve/mod.rs:35-162`) の内部・外部呼出:

### 内部再帰呼出 (mod.rs 内)

| 行 | コンテキスト |
|----|------------|
| 61 | Array inner |
| 68 | Tuple elements |
| 85 | Function params |
| 87 | Function return_type |
| 152 | Readonly inner |
| 296 | Mapped value |

### サブモジュール呼出

- `union.rs`: union メンバー解決
- `intersection.rs`: intersection メンバー解決
- `utility.rs`: ユーティリティ型（Partial, Required, Pick, Omit）
- `indexed_access.rs`: T[K] 解決
- `conditional.rs`: 条件型解決

### 外部呼出

- `typedef.rs`: TypeDef 解決全体
- `convert_ts_type` (`src/pipeline/type_converter/mod.rs:88-95`): 公開エントリポイント

### `convert_ts_type` の外部呼出元 (約60箇所)

主要カテゴリ:
- `type_resolver/visitors.rs`: 変数��言・パラメータの型注釈 (15箇所)
- `type_resolver/expressions.rs`: 式の型注釈 (8箇所)
- `type_converter/*.rs`: 型定義変換 (18箇所)
- `transformer/classes/members.rs`: クラスメンバー変換 (5箇所)
- `transformer/expressions/*.rs`: 式変換 (8箇所)
- `transformer/functions/*.rs`: 関数変換 (4箇所)
- `transformer/statements/*.rs`: 文変換 (3箇所)

---

## 5. 設計方針

### Phase 1: I-273 — 非trait制約フィルタ

**場所**: `resolve_type_params` + `extract_type_params`

**変更**: 制約を RustType に変換後、`is_valid_trait_bound` で検証。無効なら `None`。

```rust
fn is_valid_trait_bound(ty: &RustType, reg: &TypeRegistry) -> bool
```

判定ロジック:
- プリミティブ型 (F64, String, Bool, Unit, Any, Never) → false
- 複合型 (Vec, Option, Tuple, Fn, Result, Ref) → false
- Named 型: TypeRegistry で `is_interface: true` の場合のみ true
- DynTrait → true
- 未登録 Named 型 → true（外部 trait の可能性を保守的に許容）

**影響範囲**: 2関数のみ。blast radius 極小。

### Phase 2: I-299 — Item::Enum 型パラメータ対応

**Step A**: `Item::Enum` に `type_params: Vec<TypeParam>` 追加

影響:
- `Item::Enum` 構築箇所全て（既存は全て `type_params: vec![]` を追加）
- `src/generator/mod.rs` の Enum 生成で `generate_type_params` 呼出追加
- テスト中の `Item::Enum` 構築

**Step B**: `resolve_ts_type` を内部実装分離

```rust
// 既存 API (後方互換、型パラメータなし)
pub fn resolve_ts_type(info, reg, synthetic) -> Result<RustType> {
    resolve_ts_type_inner(info, reg, synthetic, &[])
}

// 内部実装 (型パラメータ名伝播)
fn resolve_ts_type_inner(info, reg, synthetic, type_param_names: &[String]) -> Result<RustType>
```

blast radius: resolve モジュール内の 30-40 箇所の再帰呼出を `_inner` に変更。外��� API 変更なし。

**Step C**: `register_union` に `type_param_names` パラメータ追加

```rust
pub fn register_union(&mut self, member_types: &[RustType], type_param_names: &[String]) -> String
```

`uses_param()` で使用されている型パラメータを検出し、`Item::Enum.type_params` に設定。

blast radius: `register_union` の呼出���を更新（`resolve_union` + テスト）。

### Phase 3: Generator 対応

`generate_item` の `Item::Enum` 分岐で `type_params` をレンダリング。

---

## 6. コンパイルテスト影響

### fixture 直接影響

- `generic-class`: `NumberBox<T: f64>` → `NumberBox<T>` に修正。E0404 解消。
  - ただし `self.value * 2.0` は `T` に `Mul` trait bound がないためコンパイルエラー残存
  - → fixture のスキップ解除は別途 trait bound 自動推論が必要

### Hono ディレクトリコンパイ���

- types.rs: E0404 �� 6 + E0425 × 2 = 8 エラー解消
- E0405 × 1 + E0107 × 2 = 3 エラー残存
- dir compile 改善: 156/158 のまま（types.rs はまだコンパイル不可）

---

## 7. 関連イシュー・バッチ化検討

### 本���ッチに含めるべき

- I-273 + I-299: RC-15 の核心。設計基盤。

### 本バッチに含めるべきでない

- E0405 (`Input::out`): indexed access 解決の問題。RC-15 とは別系統
- E0107 (ArrayBufferView generics): 型引数推論の問題。I-311 (RC-9) と関連
- I-340 (Generic Clone bound): trait bound 自動推論。別系統

### 先行すべきイシューの有無

なし。I-273 + I-299 は他のイシューに先行依存しない。

---

## 8. リスク

1. **Item::Enum に type_params 追加**: 全 Enum 構築箇所の更新が必要。漏れがあるとコンパイルエラー（ただしコンパイラが検出するので安全）
2. **`is_valid_trait_bound` の Unknown Named 型判定**: 外部型を trait と仮定するため、false positive で E0404 が残る可能性。ただし TypeRegistry に登録されていない型の制約使用は稀
3. **resolve_ts_type 内部分離の blast radius**: 30-40 箇所の変更。機械的だが漏れ注意

---

## 参考

- `src/ts_type_info/resolve/typedef.rs:18-36` — resolve_type_params
- `src/pipeline/type_converter/utilities.rs:6-25` — extract_type_params
- `src/pipeline/synthetic_registry/mod.rs:81-128` — register_union
- `src/generator/mod.rs:571-584` — generate_type_params
- `src/ir/mod.rs:422-431` — Item::Enum
- `/tmp/hono-dir-compile-output.json` — dir compile エラーデータ
- `/tmp/hono-dir-compile-check/src/types.rs` — 生成された types.rs
