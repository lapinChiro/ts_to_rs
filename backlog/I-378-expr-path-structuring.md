# I-378: `Expr` / `CallTarget` 値位置パスの完全構造化

## Background

I-375（Batch 11c-fix-2-a）は `Expr::FnCall::name: String` を `CallTarget::Path { segments, type_ref }` に置換し、call 文脈のパス意味論を構造化した。I-377（Batch 11c-fix-2-b）は `MatchPattern::EnumVariant::path: String` 等のパターン文脈の文字列パスを `Pattern::TupleStruct/UnitStruct/Struct { path: Vec<String> }` に置換し、`RUST_BUILTIN_TYPES` の `Some/None/Ok/Err` ハードコード除外を構造化により不要にした。

しかし I-377 self-review で **値位置のパス文字列 encoding が `Expr::Ident(String)` に残存** していることが判明した。`pipeline-integrity` ルール「IR に display-formatted 文字列を保存禁止」の根本原則違反であり、I-375/I-377 と同型の broken window を撲滅しないまま L3 他作業に進むのは、I-377 までに築いた構造化基盤の論理的整合性を破壊する。

### 実測した現存サイト（生産コードのみ・計 7 + 派生 1）

| # | Location | 現在の encoding | 真の意味 |
|---|---|---|---|
| 1 | `src/transformer/expressions/mod.rs:90` | `Expr::Ident("f64::NAN")` | `f64` プリミティブの associated const |
| 2 | `src/transformer/expressions/mod.rs:91` | `Expr::Ident("f64::INFINITY")` | `f64` プリミティブの associated const |
| 3 | `src/transformer/expressions/calls.rs:331` | `Expr::Ident("f64::NAN")` (Math fallback の引数) | 同上 |
| 4 | `src/transformer/expressions/member_access.rs:76` | `Expr::Ident("{enum}::{field}")` | enum unit variant の値式参照 |
| 5 | `src/transformer/expressions/member_access.rs:94` | `Expr::Ident("std::f64::consts::{name}")` | std module path const |
| 6 | `src/transformer/expressions/literals.rs:30` | `Expr::Ident("{enum}::{variant}")` | string literal union enum の値式参照 |
| 7 | `src/transformer/expressions/patterns.rs:69,84` | `Expr::Ident("{enum}::{variant}")` (×2) | enum 比較変換の右辺/左辺 |
| 8 (派生) | `src/generator/expressions/mod.rs:67` `is_type_ident()` | uppercase-head ヒューリスティック | I-375 と同型の generator 側残存 broken window |

3 つの異なる意味論（プリミティブ associated const / std const path / enum variant 値）が `Expr::Ident("a::b")` 単一形式に潰されており、walker は文字列に対する判定不能・generator は構文操作 `name.contains("::")` を強制される。サイト 8 は I-375 が `CallTarget` で撲滅したのと同じ uppercase-head ヒューリスティックを `Expr::Ident` 側に温存している。

### Root cause

`Expr::Ident(String)` が「単一識別子」と「修飾パス文字列」の二重責務を担い、構築時に判明している意味論（enum variant / プリミティブ const / std const）を捨てている。同様に `CallTarget::Path { segments, type_ref }` も I-375 の妥協形であり、segments が「ユーザー型 assoc fn / 外部 module path / builtin variant constructor / tuple struct ctor / 自由関数」の 5 意味論を `Vec<String> + Option<String>` フラグで多重表現している（5 行の docs 表でしか区別できない）。

## Goal

完了時点で以下が **構造的に成立**：

1. `Expr::Ident(String)` には `::` を一切含まない単一識別子のみが格納される。文法レベルで保証可能（grep `Expr::Ident\("[^"]*::` がプロダクションコード 0 ヒット）
2. `CallTarget::Path { segments, type_ref }` は廃止され、call 意味論ごとの専用 variant に分解される
3. `RUST_BUILTIN_TYPES` の `"Some", "None", "Ok", "Err"` 4 エントリが完全削除可能（walker が型レベルで builtin variant constructor を user type と区別する）
4. `is_type_ident()` uppercase-head ヒューリスティックが完全削除される
5. walker (`collect_type_refs_from_expr` / `IrVisitor` 実装) は **構造的に** ユーザー型参照のみを登録し、文字列判定・大文字判定・ハードコード除外リストを一切持たない
6. Hono ベンチマーク後退ゼロ、`./scripts/check-file-lines.sh` パス、`cargo test` 全テスト pass

## Scope

### In Scope

- `Expr` への 3 新 variant 追加（`EnumVariant` / `PrimitiveAssocConst` / `StdConst`）
- `CallTarget` 全面再設計（5 意味論への分解 + `UserTypeRef` newtype 導入）
- `IrVisitor` / `IrFolder` への `visit_user_type_ref` メソッド追加
- 既存 `Expr::Ident("a::b")` / `CallTarget::Path` 構築サイト 7 + N の全置換
- walker (`external_struct_generator/mod.rs::collect_type_refs_from_expr`) の `UserTypeRef` ベース化
- generator (`is_type_ident` 削除 + 新 variant の rendering)
- `RUST_BUILTIN_TYPES` から `Some/None/Ok/Err` 4 エントリ削除
- `substitute.rs` / `ir/visit.rs` / `ir/fold.rs` / `ir/test_fixtures.rs` の追従
- 全関連テストの IR 構築コード更新（`tests.rs`, `walker_tests.rs`, `expr_tests.rs`, `type_ref_tests.rs`, etc.）
- 新規追加ユニット/統合テスト（後述 Test Plan）

### Out of Scope

- `Expr::Ident("None")`（`Lit::Null` 由来。`::` を含まないため別 broken window。後続 PRD で `Expr` への `OptionNoneLit` variant 追加で対応）
- `Pattern::TupleStruct/UnitStruct/Struct::path: Vec<String>`（I-377 で structural walker 化済み。`UserTypeRef` 化は別 PRD で `Expr`/`CallTarget` のパターンと共通化）
- I-376（per-file 外部型 stub の構造的重複、pipeline 層）— `plan.md` 上の次バッチ
- L3 他バッチ（11b 以降）

## Design

### Technical Approach

#### 1. `UserTypeRef` newtype 導入（基礎型）

`src/ir/mod.rs` に追加：

```rust
/// User-defined type への参照を表す newtype。
///
/// この型のインスタンスは「TypeRegistry に登録されたユーザー型を参照する」
/// という不変条件を構築サイトで保証する。`IrVisitor::visit_user_type_ref` は
/// この型のすべての出現を walker に通知し、walker は無条件に refs に登録する。
///
/// プリミティブ型 (`f64`, `i32`)、std module path (`std::f64::consts`)、
/// builtin enum variant (`Some`, `None`, `Ok`, `Err`)、外部 crate path
/// (`scopeguard::guard`) は **この型に格納してはならない**。これらは
/// `PrimitiveType` / `StdConst` / `BuiltinVariant` / `ExternalPath` で
/// 構造的に区別される。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserTypeRef(String);

impl UserTypeRef {
    pub fn new(name: impl Into<String>) -> Self { Self(name.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
    pub fn into_string(self) -> String { self.0 }
}
```

walker は `IrVisitor::visit_user_type_ref` を override するだけで網羅的に `refs` を構築できる。型システムが「user type 以外の名前が登録される」エラーを構造的に防ぐ。

#### 2. `Expr` への 3 新 variant 追加

`src/ir/expr.rs`:

```rust
pub enum Expr {
    // ... 既存 ...

    /// Enum variant の値式参照（payload なし）。
    /// 例: `Color::Red`, `Direction::Up`
    /// payload 付き variant 構築（`Color::Red(x)`）は `Expr::FnCall { target: CallTarget::EnumVariantCtor { .. } }`
    EnumVariant {
        enum_ty: UserTypeRef,
        variant: String,
    },

    /// プリミティブ型の associated constant。
    /// 例: `f64::NAN`, `f64::INFINITY`, `i32::MAX`
    PrimitiveAssocConst {
        ty: PrimitiveType,
        name: String,
    },

    /// std ライブラリ既知の定数 path。`Math.*` 由来のみが現状の構築サイト。
    StdConst(StdConst),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType { F64, I32, I64, U32, U64, Usize, Isize, Bool, Char }

impl PrimitiveType {
    pub fn as_rust_str(self) -> &'static str { /* "f64" / "i32" / ... */ }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdConst { F64Pi, F64E, F64Ln2, F64Ln10, F64Log2E, F64Log10E, F64Sqrt2 }

impl StdConst {
    /// `Math.PI` 等の TS 名から StdConst を引く。construction site の唯一の入口。
    pub fn from_math_member(field: &str) -> Option<Self> { /* マッピング */ }
    /// generator が rendering で使う Rust path。
    pub fn rust_path(self) -> &'static str { /* "std::f64::consts::PI" 等 */ }
}
```

`Math.*` のマッピング表が **`StdConst::from_math_member` の 1 箇所** に集約される（現在は `member_access.rs:83-91` にハードコード散在）。

#### 3. `CallTarget` 全面再設計

`src/ir/expr.rs` の `CallTarget` を以下に置換：

```rust
pub enum CallTarget {
    /// 自由関数呼び出し / 局所変数を関数として呼ぶ。
    /// 例: `foo(x)`, `_f(x)`, `__iife()`
    Free(String),

    /// `Option`/`Result` の builtin variant constructor。
    /// 例: `Some(x)`, `None`, `Ok(v)`, `Err(e)`
    /// generator: bare 形式で emit。walker: 何もしない（builtin であることが型で保証）
    BuiltinVariant(BuiltinVariant),

    /// std/外部 crate の module 修飾呼び出し。
    /// 例: `std::mem::take(x)`, `std::env::var("X")`, `scopeguard::guard(...)`
    /// walker: いずれの segment も user type ではない
    ExternalPath(Vec<String>),

    /// ユーザー定義型の関連関数呼び出し。
    /// 例: `MyClass::new(x)`, `Color::default()`
    /// walker: `ty` を user type ref として登録
    UserAssocFn { ty: UserTypeRef, method: String },

    /// ユーザー定義 tuple struct の constructor。
    /// 例: `Wrapper(x)` where `interface Wrapper { (x: T): U }`
    UserTupleCtor(UserTypeRef),

    /// ユーザー定義 enum variant の constructor（payload あり）。
    /// 例: `Color::Red(x)`, `Direction::Up(meta)`
    UserEnumVariantCtor { enum_ty: UserTypeRef, variant: String },

    /// `super(args)` — 親クラス constructor 呼び出し。
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinVariant { Some, None, Ok, Err }

impl BuiltinVariant {
    pub fn as_rust_str(self) -> &'static str { /* "Some" / "None" / "Ok" / "Err" */ }
}
```

旧 `CallTarget::Path` / `simple` / `assoc` / `path` ヘルパは廃止。各構築サイトは意味論に応じた直接 variant を使う。これにより：

- walker は `IrVisitor::visit_user_type_ref` を override するだけで `UserAssocFn::ty` / `UserTupleCtor::0` / `UserEnumVariantCtor::enum_ty` / `Expr::EnumVariant::enum_ty` を一様に拾う
- `BuiltinVariant` / `ExternalPath` には `UserTypeRef` が含まれない → walker は構造的に register しない → `RUST_BUILTIN_TYPES` から `"Some","None","Ok","Err"` を削除可能

#### 4. `IrVisitor` / `IrFolder` 拡張

`src/ir/visit.rs`:
```rust
pub trait IrVisitor {
    // ... 既存 ...
    fn visit_user_type_ref(&mut self, _r: &UserTypeRef) {}
}
```

`walk_expr` の `Expr::EnumVariant` 分岐で `v.visit_user_type_ref(&enum_ty)` を呼び、`walk_call_target` を新設して各 variant の UserTypeRef フィールドを通知する。`IrFolder` 側も対称に `fold_user_type_ref` を追加（識別関数デフォルト）。

#### 5. walker (`external_struct_generator/mod.rs`)

`collect_type_refs_from_expr` を `IrVisitor` 実装の `TypeRefCollector` に置換（I-377 で導入済の枠組み）し、`visit_user_type_ref` だけを override：

```rust
impl IrVisitor for TypeRefCollector<'_> {
    fn visit_user_type_ref(&mut self, r: &UserTypeRef) {
        self.refs.insert(r.as_str().to_string());
    }
    // 他の visit_* メソッドはデフォルト walk_* に委譲
}
```

`collect_type_refs_from_expr` 内の `Expr::FnCall` 分岐 / `RUST_BUILTIN_TYPES` の Some/None/Ok/Err エントリを削除可能。

#### 6. generator

`src/generator/expressions/mod.rs`:

- `Expr::EnumVariant { enum_ty, variant }` → `format!("{}::{}", enum_ty.as_str(), variant)`
- `Expr::PrimitiveAssocConst { ty, name }` → `format!("{}::{}", ty.as_rust_str(), name)`
- `Expr::StdConst(c)` → `c.rust_path().to_string()`
- `CallTarget::Free(name)` → `name`
- `CallTarget::BuiltinVariant(v)` → `v.as_rust_str()`
- `CallTarget::ExternalPath(segs)` → `segs.join("::")`
- `CallTarget::UserAssocFn { ty, method }` → `format!("{}::{}", ty.as_str(), method)`
- `CallTarget::UserTupleCtor(ty)` → `ty.as_str()`
- `CallTarget::UserEnumVariantCtor { enum_ty, variant }` → `format!("{}::{}", enum_ty.as_str(), variant)`
- `CallTarget::Super` → `"super"`

`is_type_ident()` 関数および `MethodCall` 内の `sep = if is_type_ident { "::" } else { "." }` 分岐を削除する。`Foo.method()` static 呼び出しは Transformer が `Expr::FnCall { target: CallTarget::UserAssocFn { ty: UserTypeRef("Foo"), method: "method" } }` を構築する責務になる（事前検証必須・後述 T0）。

#### 7. transformer 構築サイト書き換え

| Site | 旧 | 新 |
|---|---|---|
| `mod.rs:90` | `Expr::Ident("f64::NAN")` | `Expr::PrimitiveAssocConst { ty: F64, name: "NAN" }` |
| `mod.rs:91` | `Expr::Ident("f64::INFINITY")` | `Expr::PrimitiveAssocConst { ty: F64, name: "INFINITY" }` |
| `calls.rs:331` | 同上 | 同上 |
| `member_access.rs:76` | `Expr::Ident("{name}::{field}")` | `Expr::EnumVariant { enum_ty: UserTypeRef::new(name), variant: field }` |
| `member_access.rs:94` | `Expr::Ident("std::f64::consts::{name}")` | `Expr::StdConst(StdConst::from_math_member(field).unwrap())` |
| `literals.rs:30` | `Expr::Ident("{name}::{variant}")` | `Expr::EnumVariant { enum_ty: UserTypeRef::new(name), variant }` |
| `patterns.rs:69` | `Expr::Ident("{enum}::{variant}")` | `Expr::EnumVariant { enum_ty: UserTypeRef::new(enum_name), variant }` |
| `patterns.rs:84` | 同上 | 同上 |
| `calls.rs:106-113` | `CallTarget::Path { type_ref: Some(...) }` | `CallTarget::UserTupleCtor` または `UserAssocFn` を意味論に応じて構築 |
| その他 `CallTarget::simple/assoc/path` 全呼び出し | 直接 variant 構築に置換 |

`calls.rs` の TypeDef 分岐ロジックは `TypeDef::Struct { is_callable: true } → UserTupleCtor`、`TypeDef::Enum + variant 一致 → UserEnumVariantCtor`、それ以外 → 既存ロジック相当の分類を行う。

### Design Integrity Review

`.claude/rules/design-integrity.md` チェック：

- **Higher-level consistency**: parser → transformer → generator パイプライン整合性に合致。Transformer が意味論を判定し IR に構造化、Generator が rendering に専念。`pipeline-integrity.md` 「IR に display-formatted 文字列を保存禁止」を完全達成
- **DRY**: `Math.*` マッピング表が `StdConst::from_math_member` 1 箇所に集約。`is_type_ident` 削除で uppercase 判定の重複（generator + walker）を完全消滅。`UserTypeRef` 1 型で walker / fold / substitute の通知点を一元化
- **Orthogonality**: `Expr::EnumVariant` / `PrimitiveAssocConst` / `StdConst` / `CallTarget` 各 variant が単一意味論を担う。多重責務（旧 `Expr::Ident` の「単一識別子 ∪ 修飾パス」）を解消
- **Coupling**: 依存方向は `walker → IrVisitor → expr.rs (UserTypeRef)`。新規循環なし。`Transformer → registry::TypeDef` 依存は既存通り（call 種別判定で必要）
- **Broken windows 検出**:
  - サイト 8（`is_type_ident`）— **本 PRD で修正**
  - `Expr::Ident("None")`（`Lit::Null` 由来）— `::` を含まないため別クラス。**TODO 記録 + 別 PRD**
  - `Pattern::*::path: Vec<String>` の I-377 暫定形 — 構造的 walker 化済みで現状 broken window ではない。**修正不要**

検証済み、上記以外の設計上の問題なし。

### Impact Area

**新規ファイル**: なし（既存 `src/ir/expr.rs` / `mod.rs` 内に追加）

**変更ファイル**:
- `src/ir/expr.rs` — `CallTarget` 再設計 + `Expr` 3 新 variant + `PrimitiveType` / `StdConst` / `BuiltinVariant` / `UserTypeRef` 追加
- `src/ir/mod.rs` — `pub use` 追加
- `src/ir/visit.rs` — `walk_expr` 新 variant 追加 / `walk_call_target` 新設 / `visit_user_type_ref` フック
- `src/ir/fold.rs` — 対称な fold 拡張
- `src/ir/substitute.rs` — `Expr::substitute` の新 variant 対応（リーフなので識別折返し）
- `src/ir/test_fixtures.rs` — `CallTarget::simple` 等の helper 呼び出し置換
- `src/transformer/expressions/mod.rs` — `NaN` / `Infinity` 構築置換
- `src/transformer/expressions/calls.rs` — `CallTarget` 構築サイト全置換 + `f64::NAN` 引数置換
- `src/transformer/expressions/member_access.rs` — enum/Math 構築置換
- `src/transformer/expressions/literals.rs` — string→enum variant 構築置換
- `src/transformer/expressions/patterns.rs` — string 比較→enum variant 構築置換
- `src/generator/expressions/mod.rs` — 新 variant rendering + `is_type_ident` 削除 + `MethodCall` sep ロジック整理
- `src/pipeline/external_struct_generator/mod.rs` — `RUST_BUILTIN_TYPES` から Some/None/Ok/Err 削除 / `TypeRefCollector` simplification
- 全テストファイル: `src/ir/tests/expr_tests.rs`, `src/generator/expressions/tests.rs`, `src/pipeline/external_struct_generator/tests/*.rs`, `src/transformer/expressions/tests/calls/type_ref_tests.rs`, `tests/lowercase_class_reference_test.rs`

### Semantic Safety Analysis

**該当: 本 PRD は型 fallback / 型 approximation を導入しないが、IR 構造変更が生成 Rust に影響する可能性があるため安全性を分析する。**

| 変更 | 旧 generator 出力 | 新 generator 出力 | 意味論差 |
|---|---|---|---|
| `Expr::Ident("f64::NAN")` → `PrimitiveAssocConst{F64,"NAN"}` | `f64::NAN` | `f64::NAN` | **完全一致**（Safe） |
| `Expr::Ident("std::f64::consts::PI")` → `StdConst::F64Pi` | `std::f64::consts::PI` | `std::f64::consts::PI` | **完全一致**（Safe） |
| `Expr::Ident("Color::Red")` → `EnumVariant{Color,"Red"}` | `Color::Red` | `Color::Red` | **完全一致**（Safe） |
| `CallTarget::simple("Some")` → `BuiltinVariant(Some)` | `Some(args)` | `Some(args)` | **完全一致**（Safe） |
| `CallTarget::assoc("MyClass","new")` → `UserAssocFn{MyClass,"new"}` | `MyClass::new(args)` | `MyClass::new(args)` | **完全一致**（Safe） |
| `is_type_ident("Foo")` → 削除 | `Foo.method()` を `Foo::method()` に rewrite | Transformer が事前に `CallTarget::UserAssocFn` 構築 | **要事前検証**（後述 T0） |

**T0 における必須事前検証**: Transformer が現状すべての static method 呼び出しに対して `Expr::MethodCall { object: Expr::Ident("Foo"), .. }` ではなく `Expr::FnCall { target: CallTarget::assoc("Foo", method) }` を構築していることを grep + Hono 出力差分で確認する。もし `MethodCall` 経由のサイトが残っていれば、そちらを Transformer 側で先に修正する（このサブタスクが T0）。検証で漏れが見つかった場合は **削除より修正を優先** し、`is_type_ident` を残さない。

**Verdict**: 上記すべて Safe（出力 Rust ソースは byte-for-byte 同一、または T0 検証で同一性を担保）。silent semantic change なし。

## Implementation Progress

### Phase 構成（Phase 1 開始時に確定）

PRD 全 13 タスク (T0〜T12) を **3 フェーズに分割** して進める。各フェーズ末でユーザーがレビュー＋コミット。理由: `CallTarget::simple/assoc/path` ヘルパ削除の瞬間からビルド全断 → T4〜T10 完了まで cargo check 不能の長い壊れた中間状態が発生するため、安全状態 (build/test pass) で区切る。

| Phase | 含むタスク | 状態 | 説明 |
|---|---|---|---|
| **Phase 1** | T1, T2 (+ self-review) | ✅ 完了 | additive のみ。新型 4 + Expr 3 variant 追加。既存サイト未変更のためビルド・テスト pass 状態 |
| **Phase 2** | T0, T3, T4, T5, T6, T7, T8, T9, T10 | ⏸ 未着手 | 破壊的フェーズ。`CallTarget` 全面再設計、Transformer 構築サイト書き換え、walker simplification、`is_type_ident` 削除、テスト追従 |
| **Phase 3** | T11, T12 | ⏸ 未着手 | Hono ベンチ、quality-check、TODO/plan.md 更新 |

### Phase 1 完了内容（self-review 後の最終状態）

**実装**:
- T1: `UserTypeRef` (debug_assert で不変条件構造的保証) / `PrimitiveType` (9 variant) / `StdConst` (7 variant + `from_math_member` / `rust_path`) / `BuiltinVariant` (4 variant) 追加
- T2: `Expr::EnumVariant` / `Expr::PrimitiveAssocConst` / `Expr::StdConst` 追加。`is_trivially_pure` / `is_copy_literal` を実意味論で拡張（PRD-DEVIATION D-1 参照、PRD T2 spec 修正済）
- 連動更新: `IrVisitor::visit_user_type_ref` フック / `IrFolder::fold_user_type_ref` フック / `walk_expr` 両方の variant 分岐 / generator 3 variant rendering / walker (`collect_type_refs_from_expr`) `EnumVariant` 登録 / mutability tracker leaf 処理 / `test_fixtures::all_exprs` 3 fixture 追加
- ファイル分割: `src/ir/visit.rs` 1052 行→ 546 行に。test mod を `src/ir/visit_tests.rs` に extract（`#[path]` 属性で参照）

**Phase 1 self-review で検出した defect 8 件をすべて修正**:
- D-A (silent semantic change): `is_trivially_pure` / `is_copy_literal` を実意味論に修正 → PRD T2 spec 修正
- D-B: catch-all アンチパターン → 新 variant 明示追加で防御
- D-C: `UserTypeRef::new` 不変条件 type-level 保証 → `debug_assert!` 追加
- D-D: `visit_user_type_ref` 発火テスト追加
- D-E: `fold_user_type_ref` 発火テスト追加
- D-F: generator rendering 単体テスト 3 ケース追加
- D-G: walker registration / 非登録テスト 3 ケース追加
- D-H: ファイル行数閾値違反 → visit_tests.rs 分離

**Phase 1 品質ゲート**:
- `cargo test --lib`: 2190 passed (Phase 1 開始時 2171 → +19; +12 が Phase 1 追加, +7 が self-review 追加)
- `cargo clippy --all-targets --all-features -- -D warnings`: 警告 0
- `cargo fmt --all --check`: pass
- `./scripts/check-file-lines.sh`: 全ファイル 1000 行以内

### Phase 2 着手前の必須事項

1. **PRD-DEVIATION D-1 を事前確認**: Phase 2 の T11 baseline 比較で expected diff (`unwrap_or_else(|| f64::NAN)` → `unwrap_or(f64::NAN)` 系) を grep で先に列挙する
2. **Phase 1 で残存する Phase 2 解消対象**:
   - `BuiltinVariant` 型は Phase 1 では production 構築サイトなし → Phase 2 の T3 で `CallTarget::BuiltinVariant` が消費する。Phase 1 段階では tests のみで使用される過渡状態
   - `external_struct_generator/mod.rs::collect_type_refs_from_expr` の `EnumVariant::enum_ty` 登録ロジックは Phase 2 の T7 で `IrVisitor` ベース `TypeRefCollector` に統合される（`visit_user_type_ref` override に集約）
   - `IrVisitor::visit_user_type_ref` フック自体の真価は Phase 2 で `CallTarget::UserAssocFn` / `UserTupleCtor` / `UserEnumVariantCtor` などの user variant でも発火するように `walk_call_target` 経由で配線された時点で発揮される（Phase 1 では `Expr::EnumVariant` のみが発火源）

## Task List

### T0: `is_type_ident` 削除可能性の事前検証

- **Work**: `src/transformer/` 全体で `Expr::MethodCall { object: Expr::Ident(name), .. }` を構築するサイトを grep し、`name` が type identifier であり得るすべてのケースを列挙。Hono fixture 全 158 ファイルに対して現状の生成出力を保存（baseline）し、`is_type_ident` の挙動が依存している箇所を特定する。漏れが見つかった場合は Transformer 側で `CallTarget::UserAssocFn` 構築に修正するためのサブタスクを T9 に追加
- **Completion criteria**: (1) `is_type_ident` を `unreachable!()` に差し替えても全テストが通ることを確認、または (2) 漏れサイトの修正リストが文書化される
- **Depends on**: なし
- **Prerequisites**: baseline 生成保存

### T1: `UserTypeRef` / `PrimitiveType` / `StdConst` / `BuiltinVariant` 追加

- **Work**: `src/ir/expr.rs` に 4 型を追加。`StdConst::from_math_member` / `rust_path`、`PrimitiveType::as_rust_str`、`BuiltinVariant::as_rust_str` を実装。`UserTypeRef::new` / `as_str` / `into_string` を実装。`UserTypeRef::new` は `debug_assert!` で `::` を含む文字列・空文字を拒否し、不変条件を構築サイトで構造的に保証する（**Phase 1 self-review で追加**）
- **Completion criteria**: 4 型の単体テスト追加（各メソッドの正常系・境界値・不変条件 panic）。`cargo test` pass
- **Depends on**: なし
- **Status**: ✅ 完了 (Phase 1)

### T2: `Expr` への 3 新 variant 追加

- **Work**: `src/ir/expr.rs` `enum Expr` に `EnumVariant` / `PrimitiveAssocConst` / `StdConst` を追加。
  `is_trivially_pure` / `is_copy_literal` を実意味論に合わせて拡張（**PRD-DEVIATION D-1 参照** — 当初 spec の「3 variant とも false」は既存 `Expr::Ident("f64::NAN") → is_trivially_pure() == true` の見落としによる defect であり、Phase 2 で `data_literals.rs` の spread/dead-code 経路に regression を誘発するため修正）:
  - `is_trivially_pure`: 3 variant とも `true`（定数参照、副作用ゼロ。`Expr::Ident("f64::NAN")` の既存挙動を維持し silent semantic change を防ぐ）
  - `is_copy_literal`: `PrimitiveAssocConst` / `StdConst` は `true`（プリミティブ Copy 値で eager 評価安全。Phase 2 で Option default が `unwrap_or_else(|| f64::NAN)` → `unwrap_or(f64::NAN)` に改善され byte-diff 発生 → T11 で承認）、`EnumVariant` は親 enum の Copy 性 unknown のため保守的に `false`
- **Completion criteria**: `cargo check` pass。`is_trivially_pure` / `is_copy_literal` の 3 variant 単体テストが上記表通りに pass
- **Depends on**: T1
- **Status**: ✅ 完了 (Phase 1)

### T3: `CallTarget` 再設計

- **Work**: 旧 `CallTarget::Path` / `simple` / `assoc` / `path` / `as_simple` / `is_path` を完全削除し、新 7 variant を追加。コンパイル不能箇所はこの時点で大量に発生するが、T4-T8 で順次解消する
- **Completion criteria**: 新型定義の単体テスト追加（各 variant の構築・パターンマッチ）
- **Depends on**: T1

### T4: `IrVisitor` / `IrFolder` 拡張

- **Work**: `src/ir/visit.rs` に `visit_user_type_ref` フック追加 + `walk_expr` の新 variant 分岐 + `walk_call_target` 新設。`src/ir/fold.rs` に対称な拡張（`fold_user_type_ref` / `fold_call_target` 各 variant 折返し）
- **Completion criteria**: visit/fold の単体テスト：各新 variant が visit_user_type_ref を 1 回だけ呼ぶこと、`BuiltinVariant` / `ExternalPath` / `Free` / `Super` では呼ばれないことを assertion
- **Depends on**: T2, T3

### T5: `substitute.rs` 追従

- **Work**: `Expr::substitute` の match に新 variant 3 + 新 `CallTarget` variant 7 を追加。すべてリーフ扱い（識別折返し）。旧 `CallTarget::Path { type_ref }` の置換ロジックは廃止
- **Completion criteria**: 既存 `substitute_test_*` の `CallTarget` テストを新形式に書き換え + 新 variant が round-trip することの assertion 追加
- **Depends on**: T2, T3

### T6: Transformer 構築サイト書き換え

- **Work**: 上記「7. transformer 構築サイト書き換え」表 9 行をすべて新型構築に置換。`calls.rs` 内の TypeDef 分岐ロジックは `Struct{is_callable}/Enum/その他` 判定で `UserTupleCtor`/`UserAssocFn`/`UserEnumVariantCtor`/`Free`/`BuiltinVariant`/`ExternalPath` を構築する関数 `classify_call_target(reg, name) -> CallTarget` に集約
- **Completion criteria**: `cargo check` pass。Transformer ユニットテスト全 pass。`grep -rn 'Expr::Ident("[^"]*::' src/transformer/` が 0 ヒット
- **Depends on**: T2, T3

### T7: walker simplification + RUST_BUILTIN_TYPES クリーンアップ

- **Work**: `external_struct_generator/mod.rs::collect_type_refs_from_expr` を `TypeRefCollector` の `IrVisitor` 実装（`visit_user_type_ref` のみ override）に置換。`Expr::FnCall` 分岐の `type_ref` 抽出ロジック削除。`RUST_BUILTIN_TYPES` から `"Some","None","Ok","Err"` 4 エントリを削除
- **Completion criteria**: walker_tests 全 pass。`integration_test.rs::test_type_narrowing` / `test_async_await` / `test_error_handling` / `test_narrowing_truthy_instanceof` の 4 件（I-375 申し送りで暫定復元したエントリの保護対象）が削除後も pass
- **Depends on**: T4, T6

### T8: Generator 新 variant rendering + `is_type_ident` 削除

- **Work**: `src/generator/expressions/mod.rs` に新 variant の rendering を追加。`is_type_ident` 関数および `MethodCall` 内の `sep` 分岐を削除（T0 で検証済の前提下）。`Expr::FnCall` の rendering を新 `CallTarget` variant に対応
- **Completion criteria**: generator unit tests 全 pass + 新 variant 各 1 ケースの rendering スナップショット追加。`grep is_type_ident src/generator/` 0 ヒット
- **Depends on**: T2, T3, T0

### T9: T0 で発見した漏れサイトの Transformer 修正（条件付き）

- **Work**: T0 で `MethodCall { Ident, .. }` 経由の static method 呼び出しが見つかった場合、Transformer 側で `CallTarget::UserAssocFn` 構築に書き換え
- **Completion criteria**: T0 baseline と新生成出力が完全一致
- **Depends on**: T0
- **Prerequisites**: T0 で漏れが検出されること（漏れがなければスキップ）

### T10: テストコード全面追従

- **Work**: 以下ファイルの `CallTarget::simple/assoc/path` 呼び出し / `Expr::Ident("a::b")` 構築をすべて新形式に置換：
  - `src/ir/tests/expr_tests.rs`
  - `src/generator/expressions/tests.rs`
  - `src/pipeline/external_struct_generator/tests/walker_tests.rs`
  - `src/pipeline/external_struct_generator/tests/refs_from_bodies_tests.rs`
  - `src/transformer/expressions/tests/calls/type_ref_tests.rs`
  - `tests/lowercase_class_reference_test.rs`
- **Completion criteria**: `cargo test` 全 pass
- **Depends on**: T6, T7, T8

### T11: Hono ベンチ + ファイル行数チェック + quality-check

- **Work**: `./scripts/hono-bench.sh` 実行 → 後退ゼロ確認 / `./scripts/check-file-lines.sh` 実行 / `cargo fix` → `cargo fmt` → `cargo clippy --all-targets --all-features -- -D warnings` → `cargo test` → `cargo llvm-cov`
- **Completion criteria**: クリーン 114/158 維持、エラーインスタンス 54 維持、警告 0、テスト pass、行数閾値内、カバレッジ閾値内
- **Depends on**: T10

### T12: TODO 追記 + plan.md 更新

- **Work**: 「Out of Scope」項目（`Expr::Ident("None")` 等）を `TODO` に `[broken-window:Lit::Null]` タグで追記。`plan.md` の `11c-fix-2-d` を完了マーク + 次バッチ `11c-fix-2-c` (I-376) を「次」に
- **Completion criteria**: ドキュメント更新完了
- **Depends on**: T11

## Test Plan

### T1 単体（新規）

- `UserTypeRef::new("Foo").as_str() == "Foo"`
- `PrimitiveType::F64.as_rust_str() == "f64"` 全 9 variant
- `StdConst::from_math_member("PI") == Some(F64Pi)`、未知 field は `None`
- `StdConst::F64Pi.rust_path() == "std::f64::consts::PI"` 全 7 variant
- `BuiltinVariant::Some.as_rust_str() == "Some"` 全 4 variant

### T2/T3 単体（新規）

- `Expr::EnumVariant { enum_ty: UserTypeRef::new("Color"), variant: "Red".into() }.is_trivially_pure() == false`（同 PrimitiveAssocConst, StdConst）
- 各 `CallTarget` variant の構築 + `match` パターンが網羅的にコンパイル可能

### T4 単体（新規）

- `TypeRefCollector` モック visitor:
  - `Expr::EnumVariant { enum_ty: UserTypeRef::new("Color"), .. }` を visit → `["Color"]` 登録
  - `Expr::PrimitiveAssocConst { F64, "NAN" }` を visit → `[]`（空）
  - `Expr::StdConst(F64Pi)` を visit → `[]`
  - `CallTarget::Free("foo")` → `[]`
  - `CallTarget::BuiltinVariant(Some)` → `[]`
  - `CallTarget::ExternalPath(vec!["std","mem","take"])` → `[]`
  - `CallTarget::UserAssocFn { ty: UserTypeRef::new("MyClass"), .. }` → `["MyClass"]`
  - `CallTarget::UserTupleCtor(UserTypeRef::new("Wrapper"))` → `["Wrapper"]`
  - `CallTarget::UserEnumVariantCtor { enum_ty: UserTypeRef::new("Color"), .. }` → `["Color"]`
  - `CallTarget::Super` → `[]`

### T6 統合（新規）

- `tests/lowercase_class_reference_test.rs` に「`class myClass { static foo() {} }; myClass.foo()` → walker が `myClass` を refs 登録」確認の walker 直接検証テストを追加（I-375 の Priority B 申し送り完遂）
- `tests/enum_value_path_test.rs` 新規: `enum Color { Red, Green }; const c = Color.Red;` → 生成 Rust に `Color::Red` が含まれ、walker が `Color` を refs 登録すること
- `tests/math_const_test.rs` 新規: `const x = Math.PI;` → 生成 Rust に `std::f64::consts::PI`、walker は何も登録しないこと
- `tests/nan_infinity_test.rs` 新規: `const n = NaN; const i = Infinity;` → `f64::NAN` / `f64::INFINITY`、walker 何もなし

### T7 回帰テスト

- I-375 申し送りの 4 件（`integration_test.rs::test_type_narrowing` / `test_async_await` / `test_error_handling` / `test_narrowing_truthy_instanceof`）が `RUST_BUILTIN_TYPES` から Some/None/Ok/Err 削除後も pass

### T8 単体（新規 / 既存修正）

- 既存 `generator/expressions/tests.rs` の static method call スナップショット（`Foo.method()` → `Foo::method()` 系）を、新 IR (`CallTarget::UserAssocFn`) ベースに書き換え
- `is_type_ident` テスト（存在すれば）削除

### T11 E2E

- Hono 全 158 ファイル変換出力 byte-for-byte 比較（baseline = T0 で保存したもの）。差分が出る場合は **意味論的等価性** を 3 ケース手動検証（diff の各 hunk について「旧出力と新出力が同じ Rust 意味論を持つ」ことを確認）

## Completion Criteria

- [ ] `grep -rn 'Expr::Ident("[^"]*::' src/` プロダクションコード 0 ヒット
- [ ] `grep -rn 'CallTarget::Path\|CallTarget::simple\|CallTarget::assoc\|CallTarget::path' src/` 0 ヒット
- [ ] `grep -rn 'is_type_ident' src/` 0 ヒット
- [ ] `RUST_BUILTIN_TYPES` から `"Some", "None", "Ok", "Err"` 4 エントリ削除済
- [ ] `cargo test` 全 pass（テスト数は新規追加分 ≥ +20 件）
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 警告 0
- [ ] `cargo fmt --all --check` pass
- [ ] `./scripts/check-file-lines.sh` pass（行数閾値内）
- [ ] `cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89` pass
- [ ] `./scripts/hono-bench.sh` 実行: クリーン率 114/158 (72.2%) **後退なし**、エラーインスタンス 54 **後退なし**、ディレクトリコンパイル 157/158 **後退なし**
- [ ] `tests/lowercase_class_reference_test.rs` に walker 直接検証テストが追加されている
- [ ] `tests/enum_value_path_test.rs` / `math_const_test.rs` / `nan_infinity_test.rs` 新規追加
- [ ] `TODO` に `Expr::Ident("None")` (Lit::Null 由来) の broken window が `[broken-window:Lit::Null]` タグで記録されている
- [ ] `plan.md` の Batch `11c-fix-2-d` が完了マーク済、次バッチ `11c-fix-2-c` (I-376) が「次」に設定されている

### Impact 検証（実コードパストレース）

3 代表エラーインスタンスではなく、本 PRD は **構造的 broken window 撲滅** が目的であるため、現状の Hono ベンチにエラー数の数値変動を期待しない（クリーン率 / エラー数 / コンパイル数の維持が成功条件）。代わりに以下 3 つの「現状動作するが原理的に脆い」コードパスをトレースし、新設計でその脆さが構造的に消滅することを文書で示す：

1. **walker false-positive リスク**: 現状 `RUST_BUILTIN_TYPES` から Some/None/Ok/Err を**仮に**削除すると `integration_test.rs` 4 件が回帰する（I-375 申し送りで実証済）。新設計では `CallTarget::BuiltinVariant` に `UserTypeRef` が含まれない → walker は型レベルで Some を登録不可 → 4 エントリ削除しても回帰**不可能**
2. **`is_type_ident` 偽陰性**: `class myClass {}; myClass.foo()` のような lowercase class の static 呼び出しは現状 `Foo.method()` 経路に乗らず、Transformer が `CallTarget::assoc` を構築する I-375 修正でカバーされる。新設計でも同経路に依存するが、`is_type_ident` 削除により `Expr::MethodCall` 経由の偶発的 fallback がなくなる → 偽陰性のリスクが原理的に消滅
3. **`Math.unknownField` のサイレント生成**: 現状 `member_access.rs:91 _ => None` で `Expr::Ident` 経路に落ちず通常の member access になる。新設計でも `StdConst::from_math_member` が `None` を返した場合は同経路 → 動作維持。これは **動作変更なし** であることをトレースで担保

## References

- I-375 PRD: `Batch 11c-fix-2-a` の `CallTarget::Path` 導入経緯
- I-377 PRD: `Batch 11c-fix-2-b` の `Pattern` 構造化と `IrVisitor` 導入
- `.claude/rules/pipeline-integrity.md`: IR に display-formatted 文字列を保存禁止
- `.claude/rules/conversion-correctness-priority.md`: silent semantic change > compile error > unsupported
- `src/pipeline/external_struct_generator/mod.rs:15-36` の `RUST_BUILTIN_TYPES` 暫定復元コメント（I-375 申し送り）
