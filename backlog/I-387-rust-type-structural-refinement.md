# I-387: `RustType` の構造的精緻化 (TypeVar 導入 + Named 純化)

> **位置付け**: I-382 解消プロジェクト Phase B〜C。`generate_stub_structs` の完全削除
> (I-382 本体 / Phase D) を可能にするための前段 refactoring。
> 前提調査: [`report/i382/phase-a-findings.md`](../report/i382/phase-a-findings.md)

## Background

I-383 完了後 (2026-04-08) も Hono 158 fixture で dangling refs が 23 件残存しており、その
根本原因は **`RustType::Named { name }` が 3 つの異なる概念を混在させている**ことにある:

1. **型変数** (`T`, `U`, `E` — 型パラメータ scope 内の識別子)
2. **ユーザー定義型** (`HTTPException`, `Context` — registry 管理対象)
3. **リテラル std 型名** (`"String"`, `"Box"`, `"HashMap"`, `"usize"` — 既存 variant の誤用
   または未分類)

この混在により、下流コード (substitute / monomorphize / type_param_scope 判定 / trait 判定 /
synthetic_items の free var 抽出) が **名前文字列マッチと heuristic** で区別せざるを得ず、
以下の interim patch 3 件が Cluster 1a 修正中に導入された (`ideal-implementation-primacy.md`
の interim patch 管理対象):

- `src/external_types/mod.rs::convert_external_typedef` の `push_type_param_scope` 補完 (T2.A-i)
- `src/pipeline/type_resolver/helpers.rs::enter_type_param_scope` の外部 builtin loader 経路 (T2.A-ii)
- `src/pipeline/type_resolver/helpers.rs:50-111 collect_free_type_vars` heuristic (T2.A-iv)

また Phase A の INV-5 調査で、`src/` 配下の `RustType::Named` 構築サイト 251 箇所 (実装) の
分類結果が判明した:

| 分類 | 件数 | 代表例 | 本 PRD での扱い |
|---|---|---|---|
| (a) user 定義型 | ~120 | registry 参照経由 | 据え置き (Named のまま) |
| (b) 型変数由来 | ~30 | `transformer/classes/helpers.rs:34 p.name.clone()` | **TypeVar に置換** |
| (c1) 既存 variant の誤用 | ~80 (推定) | `"String"`, `"Box"`, `"Vec"`, `"Option"`, `"Result"` | **既存 variant に巻戻し** |
| (c2) 未分類 std 型 | ~70 (推定) | `"HashMap"`, `"BTreeMap"`, `"usize"`, `"bool"` | **新 variant 追加 + 置換** |
| (d) テスト / 個別 | ~180 | 各種 | 個別判断 |

**核心的認識**: (c1) は bug である。既存 `RustType::{String, Vec, Option, Result, Tuple, Bool,
F64}` variant があるにも関わらず `Named { name: "String" }` が構築されているのは、IR 設計が
部分的にしか守られていないことを示す broken window。

## Goal

`RustType` を「**すべての Rust 型概念が名前文字列ではなく型で区別される**」状態に精緻化し、
Phase A で特定された interim patch 3 件を構造的に削除する。

### 具体的測定基準

1. `RustType::Named { name, type_args }` が参照する `name` は **user 定義型のみ**
   (registry 登録済または未登録の曖昧参照)。Rust 標準型名・型変数名は Named に現れない
2. `TypeVar { name }` variant が導入され、`convert_ts_type` で
   `type_param_scope.contains(name)` 時に構築される
3. `RustType::Named { name: "String" }` のようなリテラル既存 variant 誤用が `src/` 配下で
   **0 件** (grep 検証)
4. interim patch 3 件 (T2.A-i / T2.A-ii / T2.A-iv) の関連コードが削除済、
   `// INTERIM:` コメントが `src/` 配下で 0 件
5. `cargo test --lib` 全 pass (2228 件維持)
6. Hono 158 fixture ベンチ regression 0 (clean files / compile rate / error instances)
7. `collect_free_type_vars` heuristic および `RUST_BUILTIN_TYPES` 文字列フィルタ依存が
   `src/` 配下で 0 箇所 (grep 検証)
8. probe 再投入で Cluster 1a (型パラメータ leak) が継続的に 0 件
9. /quality-check 通過 (clippy / fmt / test 0 warn 0 err)

## Scope

### In Scope

1. **`RustType::TypeVar { name: String }` variant 追加** (`src/ir/types.rs`)
2. **`RustType::StdCollection { kind: StdCollectionKind, args: Vec<RustType> }` variant 追加**
   (新 enum `StdCollectionKind = HashMap | BTreeMap | HashSet | BTreeSet | VecDeque | Rc | Arc | Mutex | RefCell | Box`)。理由:
   `Box` は既存 variant がない / `HashMap` 等は頻出する Rust std 型で構造化の実効性が高い
3. **`RustType::Primitive(PrimitiveType)` variant 追加** (新 enum `PrimitiveType = Usize | Isize |
   I8 | I16 | I32 | I64 | I128 | U8 | U16 | U32 | U64 | U128 | F32`)。
   `F64` / `Bool` / `String` / `Unit` は既存変種を維持 (`src/ir/expr.rs::PrimitiveType` は式
   定数用途で別物、命名は混同しないよう調整)
4. **`convert_ts_type` / `resolve_type_ref` の Named 分岐再設計** — type_param_scope 参照で
   TypeVar / Named / StdCollection / Primitive / 既存 variant に正しく振り分ける
5. **構築サイト 251 件の置換** (INV-5 分類に基づく):
   - (b) ~30 件: Named → TypeVar
   - (c1) ~80 件: Named (String 等リテラル) → 既存 variant (`RustType::String` 等)
   - (c2) ~70 件: Named → StdCollection / Primitive
   - (a) 据え置き、(d) 個別判断
6. **下流 pattern match の更新**:
   - `src/ir/substitute.rs::fold_rust_type` で TypeVar ブランチ追加 (substitution は TypeVar
     にのみ適用、Named は据え置き)
   - `src/generator/types.rs` で TypeVar / StdCollection / Primitive の Rust コード生成
   - `src/transformer/type_position.rs::wrap_trait_for_position` の `is_trait_type(name)`
     文字列判定を Named 限定にし、TypeVar / Primitive / StdCollection は非 trait 扱い
   - `src/transformer/expressions/patterns.rs:206` の HashMap/BTreeMap リテラルマッチを
     StdCollection 判定に置換
   - `src/pipeline/synthetic_registry/mod.rs::extract_used_type_params` を TypeVar walker に
     置換
7. **interim patch 3 件削除**:
   - T2.A-i: `src/external_types/mod.rs:264, 482` の `push_type_param_scope` 補完削除
   - T2.A-ii: `src/pipeline/type_resolver/helpers.rs:237-258 enter_type_param_scope`
     およびそれを呼ぶ 5 callers (`visitors.rs:101,405,498` / `expressions.rs:771,918`) の
     見直し — scope 管理を `type_converter::convert_ts_type` 内に集約
   - T2.A-iv: `src/pipeline/type_resolver/helpers.rs:50-111 collect_free_type_vars` 削除 +
     2 callers (`expressions.rs:809,944`) の TypeVar walker 化
8. **`RUST_BUILTIN_TYPES` 文字列フィルタ依存の除去** — TypeVar / Primitive / StdCollection の
   構造的区別で不要化
9. **新規テスト追加** (gap table 参照)
10. **doc comment 更新** — `RustType` の各 variant の責務を明示

### Out of Scope

- `generate_stub_structs` 関数自体の削除 (= I-382 本体 / Phase D)
- DOM 型 (Cluster 1b) 16 件の解消 (= PRD-β / 別起票予定、`TypeDef::ExternalUnsupported` variant 導入)
- `__type` marker 1 件の解消 (= PRD-γ / 別起票予定)
- user 型 import 生成 (= PRD-δ / Phase D 本体)
- utility type (Omit/Pick/Record) の完全展開 (INV-9 の TODO、直交する別 PRD)
- `src/ir/expr.rs::PrimitiveType` (式定数用) との統合・整理 (命名衝突のみ回避)

## Design

### Technical Approach

#### 新 IR

```rust
// src/ir/types.rs
pub enum RustType {
    // --- 既存 (役割を純化) ---
    Unit,
    String,
    F64,
    Bool,
    Option(Box<RustType>),
    Vec(Box<RustType>),
    Result(Box<RustType>, Box<RustType>),
    Tuple(Vec<RustType>),
    Fn { params: Vec<RustType>, return_type: Box<RustType> },
    Ref { target: Box<RustType>, mutable: bool },
    DynTrait { name: String, type_args: Vec<RustType> },
    QSelf { trait_name: String, type_args: Vec<RustType>, assoc: String },
    Any,
    Never,

    // --- 既存だが責務を狭める ---
    /// User-defined nominal type (registry 登録済または曖昧参照)。
    /// Rust 標準型・型変数は含まない (それぞれ専用 variant を使用)。
    Named { name: String, type_args: Vec<RustType> },

    // --- 新規 ---
    /// Generic type parameter (lexically scoped).
    /// `convert_ts_type` が `type_param_scope` を参照して構築する。
    TypeVar { name: String },

    /// Rust 整数型 (i32/u64/usize 等)。F64/Bool/String は既存 variant を使用。
    Primitive(PrimitiveIntKind),

    /// 既存専用 variant を持たない std コレクション・スマートポインタ。
    StdCollection { kind: StdCollectionKind, args: Vec<RustType> },
}

pub enum PrimitiveIntKind {
    Usize, Isize,
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    F32,
}

pub enum StdCollectionKind {
    Box,
    HashMap, BTreeMap,
    HashSet, BTreeSet,
    VecDeque,
    Rc, Arc,
    Mutex, RwLock, RefCell, Cell,
}
```

#### convert_ts_type 分岐 (疑似コード)

```rust
// src/pipeline/type_converter/mod.rs (resolve_type_ref 内)
match name {
    n if synthetic.is_in_type_param_scope(n) => RustType::TypeVar { name: n.to_string() },
    "string" | "String" => RustType::String,
    "boolean" | "bool" => RustType::Bool,
    "number" => RustType::F64,
    n if PRIMITIVE_INT_MAP.contains_key(n) => RustType::Primitive(PRIMITIVE_INT_MAP[n]),
    n if STD_COLLECTION_MAP.contains_key(n) => RustType::StdCollection {
        kind: STD_COLLECTION_MAP[n],
        args: resolved_args,
    },
    n if reg.get(n).is_some() => RustType::Named { name: n.to_string(), type_args: resolved_args },
    n => RustType::Named { name: n.to_string(), type_args: resolved_args }, // 曖昧参照 — Phase D で error 化
}
```

#### interim patch 削除の構造的根拠

- **T2.A-i/ii (scope push 補完)**: 従来は Named の段階で scope 判定できなかったため、
  呼び出し側で事前 push していた。TypeVar variant 導入後は `convert_ts_type` 内で scope を
  参照し適切に分岐するため、外部からの push 補完は不要
- **T2.A-iv (collect_free_type_vars heuristic)**: 従来は Named と TypeVar を区別できない
  ため、文字列フィルタ (`RUST_BUILTIN_TYPES`) とサイズ制約で free var を heuristic 抽出
  していた。TypeVar walker (`RustType::TypeVar { name } => names.insert(name)`) で構造的に
  置換

### Design Integrity Review

per `.claude/rules/design-integrity.md`:

- **Higher-level consistency**: `RustType` は pipeline 全体 (parser → transformer → generator)
  で共有される中核 IR。今回の精緻化は生成コード (`src/generator/types.rs`) の出力を
  変更しないため、上位レイヤとの interface は維持される (意味論等価性は Test Plan で検証)
- **DRY**: 名前文字列による判定ロジック (`is_trait_type` / `RUST_BUILTIN_TYPES` フィルタ /
  `collect_free_type_vars` heuristic) が 4 箇所に重複しているが、TypeVar / Primitive /
  StdCollection 導入により一元的 pattern match で置換され、重複が解消する
- **Orthogonality**: 各 variant が「1 つの型カテゴリ」に対応する単一責務状態に近づく。
  `Named` は user 定義型のみ、`TypeVar` は型変数のみ、`StdCollection` は std コレクション
  のみ
- **Coupling**: `type_converter` → `synthetic_registry.is_in_type_param_scope` の read 依存
  が 1 箇所追加されるが、既存の push/restore API を使う側の読み出しであり、循環・逆方向
  依存は発生しない
- **Broken windows 発見と対応**:
  - (BW-1) `src/ir/expr.rs::PrimitiveType` と新 `PrimitiveIntKind` の命名衝突。式定数用の
    既存 enum とは別概念なので、本 PRD では `PrimitiveIntKind` とする。将来統合は別 PRD 候補
    として TODO 記録
  - (BW-2) `is_trait_type(name: &str)` 文字列判定が `src/transformer/type_position.rs:29-34`
    に存在。本 PRD で `match rust_ty { RustType::Named { name, .. } => is_trait_type(name),
    _ => false }` に変更 (Named 限定化)
  - (BW-3) `RUST_BUILTIN_TYPES` 定数が複数箇所で参照されている可能性。本 PRD 内で全削除

### Impact Area

**Primary**:
- `src/ir/types.rs` — RustType / PrimitiveIntKind / StdCollectionKind 定義
- `src/ir/substitute.rs` — fold_rust_type に TypeVar / StdCollection / Primitive ブランチ
- `src/pipeline/type_converter/mod.rs` — convert_ts_type の分岐再設計
- `src/ts_type_info/resolve/mod.rs::resolve_type_ref` — 3 階層判定 (I-383 で完了) と整合

**Secondary**:
- `src/generator/types.rs` — RustType → Rust コード生成
- `src/transformer/type_position.rs` — wrap_trait_for_position の Named 限定化
- `src/transformer/classes/helpers.rs:34` + 他 type_params.iter() サイト — TypeVar 構築
- `src/transformer/expressions/{patterns,member_access,literals,data_literals,calls}.rs`
  — std 型リテラルの variant 置換
- `src/external_types/mod.rs:264, 482` — T2.A-i 削除
- `src/pipeline/type_resolver/helpers.rs:50-258` — T2.A-ii/iv 削除
- `src/pipeline/synthetic_registry/mod.rs::extract_used_type_params` — TypeVar walker 化
- `src/external_struct_generator/mod.rs::collect_undefined_refs_inner` — TypeVar ブランチで
  scope check を除去

**Tertiary (構築サイト一括置換)**:
- `src/transformer/**/*.rs` (~30 files)
- `src/pipeline/type_converter/**/*.rs`
- `src/ts_type_info/resolve/**/*.rs`
- `src/external_types/mod.rs`

### Semantic Safety Analysis

per `.claude/rules/type-fallback-safety.md`:

本 PRD は**型解決の構造化**であり、**新規の fallback を導入しない**。構築サイト置換は
「同じ概念を別の variant で表現する」ものであり、下流の生成コード出力 (Rust source) は
不変である必要がある。各置換パターンの安全性:

| パターン | Before | After | 分類 | 根拠 |
|---|---|---|---|---|
| 型変数 | `Named { name: "T" }` | `TypeVar { name: "T" }` | **Safe** | generator 側で同一文字列出力、substitute 対象が明示化されるだけ |
| "String" literal | `Named { name: "String" }` | `RustType::String` | **Safe** | 既存 variant の生成出力が `String` なので完全同一 |
| "Box" literal | `Named { name: "Box", type_args: [T] }` | `StdCollection { kind: Box, args: [T] }` | **Safe** | 生成出力 `Box<T>` で同一 |
| "HashMap" literal | `Named { name: "HashMap", type_args: [K,V] }` | `StdCollection { kind: HashMap, args: [K,V] }` | **Safe** | 生成出力 `HashMap<K,V>` で同一 |
| "usize" literal | `Named { name: "usize" }` | `Primitive(Usize)` | **Safe** | 生成出力 `usize` で同一 |
| user type `HTTPException` | `Named { name: "HTTPException" }` | 据え置き | **Safe** | 変更なし |

**UNSAFE パターン**: なし (全置換が generate 同一出力)。

**検証手段**: Test Plan の golden test (Hono 158 fixture の生成 Rust ソースが diff なし)
+ `cargo test --lib` 全 pass + ベンチ clean files 維持。

### 命名と置換マップの確定 (実装前定義)

下記マッピングを grep 正規表現ベースの**ドライラン → レビュー → 実行**手順 (`bulk-edit-safety.md`
準拠) で置換する。

```
"String" / "string"             → RustType::String
"bool" / "boolean"              → RustType::Bool
"f64" / "number"                → RustType::F64
"usize"                         → Primitive(Usize)
"isize"                         → Primitive(Isize)
"i8"..."i128"                   → Primitive(I8..I128)
"u8"..."u128"                   → Primitive(U8..U128)
"f32"                           → Primitive(F32)
"Box"                           → StdCollection{Box}
"HashMap"                       → StdCollection{HashMap}
"BTreeMap"                      → StdCollection{BTreeMap}
"HashSet" / "BTreeSet" / "VecDeque" → StdCollection{...}
"Rc" / "Arc" / "Mutex" / "RwLock" / "RefCell" / "Cell" → StdCollection{...}
"Vec"                           → 既存 RustType::Vec
"Option"                        → 既存 RustType::Option
"Result"                        → 既存 RustType::Result
```

## Task List

### T1: RustType variant 拡張 (RED → GREEN)

- **Work**:
  - `src/ir/types.rs` に `TypeVar { name }`, `Primitive(PrimitiveIntKind)`,
    `StdCollection { kind, args }` variant 追加
  - `PrimitiveIntKind` / `StdCollectionKind` enum 定義 + doc comment
  - `RustType::Named` の doc comment を「user 定義型専用」に更新
  - 新 variant の Display / PartialEq / Eq / Hash / Clone 等 derive 確認
  - unit test `src/ir/tests/types_tests.rs` に新 variant 判定テスト 3 件追加
- **完了条件**: `cargo check` pass、新規テスト pass、既存テスト 2228 件 pass
- **Depends on**: なし

### T2: substitute / walk ロジック拡張

- **Work**:
  - `src/ir/substitute.rs::fold_rust_type` に TypeVar / Primitive / StdCollection
    ブランチ追加。TypeVar が substitution target となる
  - 新規テスト `test_substitute_replaces_type_var`,
    `test_substitute_leaves_std_collection`,
    `test_substitute_recurses_into_std_collection_args`
- **完了条件**: 上記テスト pass + 既存 substitute テスト全 pass
- **Depends on**: T1

### T3: generator/types.rs 拡張

- **Work**:
  - `src/generator/types.rs` で TypeVar / Primitive / StdCollection の Rust コード生成
  - TypeVar → 名前そのまま、Primitive → `usize` 等、StdCollection → `HashMap<K,V>` 等
  - generator テスト (`src/generator/tests/` または insta snapshot) に新 variant のケース追加
- **完了条件**: テスト pass、生成 Rust ソース形式が既存 Named 経由と同一
- **Depends on**: T1

### T4: convert_ts_type 分岐再設計

- **Work**:
  - `src/pipeline/type_converter/mod.rs` (および `ts_type_info/resolve/mod.rs::resolve_type_ref`)
    に PRIMITIVE_INT_MAP / STD_COLLECTION_MAP 定義
  - `is_in_type_param_scope` を最優先に判定 → TypeVar 構築
  - リテラル名マッチで既存 variant / Primitive / StdCollection に振り分け
  - 最後に registry lookup → Named 構築
  - `SyntheticTypeRegistry::is_in_type_param_scope` の read 公開 API を確認
    (未公開なら追加)
  - 新規テスト 5 件 (C1 branch coverage):
    - `test_convert_ts_type_returns_type_var_in_scope`
    - `test_convert_ts_type_returns_string_for_string_literal`
    - `test_convert_ts_type_returns_std_collection_for_hashmap`
    - `test_convert_ts_type_returns_primitive_for_usize`
    - `test_convert_ts_type_returns_named_for_user_type`
- **完了条件**: 5 テスト pass、既存 type_converter テスト全 pass
- **Depends on**: T1, T3

### T5: 構築サイト一括置換 — (c1) 既存 variant 巻戻し

- **Work**:
  - grep で `RustType::Named { name: "String"` / `"Box"` / `"Vec"` / `"Option"` /
    `"Result"` 等を全件抽出
  - `bulk-edit-safety.md` 手順 (dry run → diff review → execute → cargo check/test)
  - 対象 ~80 箇所を既存 variant に置換
- **完了条件**:
  - grep `RustType::Named \{ name: "(String|Box|Vec|Option|Result|bool|Bool|f64|F64)"`
    が 0 件 (テストコード除外)
  - `cargo test --lib` 全 pass
- **Depends on**: T1

### T6: 構築サイト一括置換 — (c2) Primitive / StdCollection

- **Work**:
  - grep で `"usize"` / `"i32"` / `"HashMap"` / `"BTreeMap"` 等を全件抽出
  - dry run → review → execute
  - 対象 ~70 箇所を Primitive / StdCollection に置換
- **完了条件**:
  - grep `RustType::Named \{ name: "(usize|i32|i64|HashMap|BTreeMap|HashSet|Rc|Arc|Mutex)"`
    が 0 件 (テストコード除外)
  - `cargo test --lib` 全 pass
- **Depends on**: T1, T5

### T7: 構築サイト一括置換 — (b) TypeVar

- **Work**:
  - `src/transformer/classes/helpers.rs:34` 等 `type_params.iter().map(|p| Named { name: p.name.clone() })`
    パターンを全件抽出 (~30 箇所)
  - `TypeVar { name: p.name.clone() }` に置換
- **完了条件**:
  - 対象 grep pattern が 0 件
  - `cargo test --lib` 全 pass
  - Hono ベンチ regression 0
- **Depends on**: T1, T4, T5, T6

### T8: interim patch 削除 — T2.A-i (convert_external_typedef)

- **Work**:
  - `src/external_types/mod.rs:264, 482` の `push_type_param_scope` 呼び出し削除
  - `// INTERIM:` コメント除去
  - 関連テスト: `src/external_types/tests/` で type_param 含む external typedef 変換が
    正しく TypeVar を生成することを確認する新規テスト 1 件
- **完了条件**: 対象 patch 削除、新規テスト pass、Hono ベンチ regression 0
- **Depends on**: T4, T7

### T9: interim patch 削除 — T2.A-ii (enter_type_param_scope)

- **Work**:
  - `src/pipeline/type_resolver/helpers.rs:237-258 enter_type_param_scope` 削除または
    TypeVar 化後の scope guard として再定義
  - 5 callers (`visitors.rs:101,405,498` / `expressions.rs:771,918`) を更新
  - scope 管理が `convert_ts_type` 内で一元化されることを確認
- **完了条件**: 対象 patch 削除、既存 5 テスト + 新規 2 件 pass
- **Depends on**: T4, T7

### T10: interim patch 削除 — T2.A-iv (collect_free_type_vars)

- **Work**:
  - `src/pipeline/type_resolver/helpers.rs:50-111 collect_free_type_vars` 削除
  - `RUST_BUILTIN_TYPES` 定数削除
  - 2 callers (`expressions.rs:809,944`) を TypeVar walker (新規ヘルパー
    `collect_type_vars(ty: &RustType) -> HashSet<String>`) に置換
  - TypeVar walker の unit test 3 件追加 (空 / 単一 / 入れ子)
- **完了条件**: 対象 patch 削除、grep `RUST_BUILTIN_TYPES` が 0 件、新規 3 テスト pass
- **Depends on**: T4, T7, T9

### T11: synthetic_registry / external_struct_generator 整合

- **Work**:
  - `src/pipeline/synthetic_registry/mod.rs::extract_used_type_params` を T10 の TypeVar
    walker に置換
  - `src/external_struct_generator/mod.rs::collect_undefined_refs_inner` で TypeVar 変種を
    スキップするブランチ追加 (そもそも TypeVar は未定義参照ではない)
  - `register_struct_dedup` / `register_intersection_enum` / `register_union` の
    type_params 抽出ロジックが walker 経由で統一されることを確認
- **完了条件**: Cluster 1a probe 再投入で dangling ref 0 件継続、
  Hono ベンチ regression 0
- **Depends on**: T10

### T12: 下流 pattern match 更新 — trait 判定 / リテラル判定

- **Work**:
  - `src/transformer/type_position.rs::wrap_trait_for_position` を `is_trait_type` 判定
    から Named 限定に変更
  - `src/transformer/expressions/patterns.rs:206` の HashMap/BTreeMap リテラルマッチを
    StdCollection 判定に置換
  - 他 file:line の pattern match (member_access.rs / literals.rs / data_literals.rs /
    calls.rs) を順次更新
  - 各箇所に unit test を追加 (C1 coverage)
- **完了条件**: 対象箇所の pattern match が TypeVar / Primitive / StdCollection を正しく
  扱う、テスト pass
- **Depends on**: T1, T5, T6

### T13: ドキュメント同期

- **Work**:
  - `plan.md`, `report/i382/master-plan.md`, `report/i382/history.md` を Phase C 完了状態に
    更新
  - interim patch 管理表 (master-plan.md) から T2.A-i/ii/iv を削除
  - `RustType` doc comment で各 variant の責務を明示
- **完了条件**: 全ドキュメントが最新実装と整合
- **Depends on**: T11, T12

### T14: /quality-check + Hono ベンチ

- **Work**:
  - `cargo fix` / `cargo fmt` / `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test` 全件
  - `./scripts/hono-bench.sh` 実行しベースライン (114/158 clean) と比較
- **完了条件**: /quality-check 0 err 0 warn、ベンチ regression 0
- **Depends on**: T13

## Test Plan

### 新規テスト (gap 埋め)

| ID | 対象 | テクニック | テスト名 |
|---|---|---|---|
| G1 | `convert_ts_type` 5 分岐 | C1 | `test_convert_ts_type_returns_{type_var,string,std_collection,primitive,named}_*` |
| G2 | `substitute` TypeVar branch | C1 + 代入 | `test_substitute_{replaces_type_var,leaves_std_collection,recurses_into_args}` |
| G3 | `collect_type_vars` walker | equivalence partition | `test_collect_type_vars_{empty,single,nested,multi}` |
| G4 | `generator` 新 variant 出力 | snapshot | `test_generate_{type_var,primitive,std_collection}_outputs_same_as_named_literal` |
| G5 | `wrap_trait_for_position` Named 限定 | boundary | `test_wrap_trait_ignores_{type_var,primitive,std_collection}` |
| G6 | Hono fixture regression | golden | 既存 insta snapshot が diff なし |
| G7 | Cluster 1a probe | integration | `test_no_dangling_type_params_in_synthetic_items` (Cluster 1a 11 件の rehearsal) |
| G8 | interim patch 削除後の scope 管理 | integration | `test_external_typedef_with_generics_converts_to_type_var`, `test_arrow_expr_free_vars_collected_via_walker` |

### 既存テスト

- `cargo test --lib` 全件 (2228) 継続 pass
- generator insta snapshot は `cargo insta review` で全件承認要 (生成出力が同一であることを
  確認)

## Completion Criteria

1. Goal 節の測定基準 1-9 すべて達成
2. T1-T14 すべて完了
3. 新規テスト G1-G8 全件 pass
4. `cargo test --lib` 2228 件以上 pass (新規 ~20 件追加)
5. `cargo clippy --all-targets --all-features -- -D warnings` 0 warn
6. `cargo fmt --all --check` pass
7. `./scripts/hono-bench.sh` 結果: clean files >= 114 / 158 (ベースライン維持)、
   error instances <= 54 (regression なし)
8. probe 再投入で Cluster 1a dangling refs = 0
9. grep 検証 (src/ 配下、テストコード除外):
   - `RustType::Named \{ name: "(String|Box|Vec|Option|Result|bool|f64|usize|HashMap)"` = 0
   - `RUST_BUILTIN_TYPES` = 0
   - `collect_free_type_vars` = 0
   - `// INTERIM: T2\.A-` = 0
10. `report/i382/master-plan.md` / `plan.md` が Phase C 完了状態に更新済

### 代表 3 件の execution path 追跡 (label-based 推定禁止のため)

本 PRD の impact 見積 (Cluster 1a 継続 0 + interim patch 3 件削除) が label ベースではなく
実 execution path に基づいていることを以下で確認する:

1. **Cluster 1a の `M` 型パラメータ leak (T2.A-iv で解消済)**
   実行パス: `convert_ts_type("M")` → T4 実装後は
   `is_in_type_param_scope("M") == true` → `TypeVar { name: "M" }` 直接構築 →
   `synthetic_registry::extract_used_type_params` (T11 で TypeVar walker 化済) が `M` を
   収集 → `Item::Enum { type_params: vec!["M"] }` → probe で dangling 検出されず
   (`collect_undefined_refs_inner` で TypeVar を exclude)。
   **→ interim patch 削除後も 0 件維持が保証される**

2. **T2.A-i の外部 builtin JSON loader 経路**
   実行パス: `convert_external_typedef` が
   `push_type_param_scope(vec!["T"])` を**呼ばなく**ても、T4 実装の
   `convert_ts_type` は**interface 単位で既に push されている scope** を参照するため
   TypeVar が正しく構築される。
   **→ T8 で patch 削除しても regression しない**

3. **T2.A-iv の `collect_free_type_vars` heuristic 経路**
   実行パス: arrow 式の expected_type flatten 時、T10 の TypeVar walker が
   `RustType::TypeVar { name } => names.insert(name)` を収集。
   従来の `RUST_BUILTIN_TYPES` フィルタ + registry check は不要。
   **→ T10 で heuristic 削除しても同一の free var set を得る**

上記 3 件は `report/i382/phase-a-findings.md` の INV-4 trace 結果と整合しており、
execution path レベルで fix 妥当性が verify 済。

---

## 参照

- Phase A 調査: [`report/i382/phase-a-findings.md`](../report/i382/phase-a-findings.md)
- マスタープラン: [`report/i382/master-plan.md`](../report/i382/master-plan.md)
- 最上位原則: `.claude/rules/ideal-implementation-primacy.md`
- PRD レビュー: `.claude/rules/prd-design-review.md`
- bulk 編集安全手順: `.claude/rules/bulk-edit-safety.md`
