# I-380: Pattern 構造化 + walker 完全 IrVisitor 化

## Background

I-377 で `MatchPattern::Verbatim(String)` / `Stmt::IfLet::pattern: String` 等を構造化 `Pattern` enum に置換し、`MatchPattern::EnumVariant::path: String` の文字列依存を排除した。しかし I-377 は時間制約から **`Pattern::TupleStruct { path: Vec<String>, .. }` / `Pattern::UnitStruct { path: Vec<String> }` / `Pattern::Struct { path: Vec<String>, .. }`** を最終形態とせず、`path: Vec<String>` で「セグメント列」として保持する暫定形を採用した。この暫定形には以下の本質的な問題が残る:

### 残存している broken window

#### 1. `PATTERN_LANG_BUILTINS` ハードコード除外リスト

`src/pipeline/external_struct_generator/mod.rs:526`:
```rust
const PATTERN_LANG_BUILTINS: &[&str] = &["Some", "None", "Ok", "Err"];
```

walker `collect_type_refs_from_pattern` は `path.first()` と `PATTERN_LANG_BUILTINS` を文字列比較し、builtin variant constructor を refs から除外する。これは I-377 が `RUST_BUILTIN_TYPES` から同じ 4 entry を構造的に削除したのと**同種の妥協**であり、Pattern 側に取り残されている。

#### 2. `Pattern::is_none_unit()` の文字列ベース判定

`src/ir/pattern.rs:114`:
```rust
pub fn is_none_unit(&self) -> bool {
    matches!(self, Pattern::UnitStruct { path } if path.len() == 1 && path[0] == "None")
}
```

I-377 で「`pattern_string == "None"` 文字列比較の置き換え」と謳ったが、実体は単に **構造化されたパスでの文字列比較** に置き換わっただけ。`PATTERN_LANG_BUILTINS` と同じ妥協が `is_none_unit` という API 内に潜伏している。

#### 3. walker の手書き再帰群が `IrVisitor` 化されていない

I-378 T7 で `collect_type_refs_from_expr` は `TypeRefCollector { refs: &mut HashSet }` (`IrVisitor` 実装) に置換したが、以下は依然として手書き再帰のまま:

- `collect_type_refs_from_pattern`
- `collect_type_refs_from_rust_type`
- `collect_type_refs_from_stmts`
- `collect_type_refs_from_stmt`
- `collect_type_refs_from_match_arm`
- `collect_type_refs_from_item`
- `collect_type_refs_from_method`
- `collect_type_refs_from_type_params`

`TypeRefCollector` は内部で `visit_pattern` / `visit_rust_type` を override してこれらを呼び出す**部分的な統合**にとどまっている。これにより:

- 新 `Item` / `Stmt` / `Pattern` variant 追加時に walker の更新を 2 箇所 (IR `walk_*` + 手書き `collect_*`) で行わなければならない
- IR 走査骨格 (`walk_*`) の単一ソース原則が部分的にしか機能していない
- 新規 walker (例: 別目的の `IrVisitor` 実装) が同じ走査ロジックを再実装する必要がある

### Root cause

`Pattern::TupleStruct { path: Vec<String>, .. }` の `path` フィールドが「**user type への参照**」と「**Rust 言語組み込み variant constructor (Some/None/Ok/Err)**」の 2 意味論を文字列ベースで多重表現している。I-378 が `CallTarget::UserAssocFn { ty: UserTypeRef }` と `CallTarget::BuiltinVariant(BuiltinVariant)` で call 側を構造的に分離したのと同じことを Pattern 側でも行う必要がある。

walker の `IrVisitor` 化は Pattern 構造化が完了していなくても可能だが、`PATTERN_LANG_BUILTINS` の構造的除去ができないため**中途半端な改善にしかならない**。両者を同一 PRD で実施することで、broken window を 1 回で完全撲滅できる。

## Goal

完了時点で以下が **構造的に成立**:

1. `Pattern::TupleStruct` / `UnitStruct` / `Struct` の `path: Vec<String>` フィールドが廃止され、構造化された `PatternCtor` enum (builtin variant / user enum variant / user tuple struct) で表現される
2. `PATTERN_LANG_BUILTINS` ハードコード除外リストがプロダクションコードから完全削除される
3. `Pattern::is_none_unit()` が構造的判定 (`matches!(self, ... PatternCtor::Builtin(BuiltinVariant::None) ...)`) になる
4. `external_struct_generator/mod.rs` 内の `collect_type_refs_from_*` 8 関数すべてが削除され、`TypeRefCollector` の `IrVisitor` override に集約される
5. walker は `walk_call_target` / `walk_pattern` 経由で `visit_user_type_ref` を一様に発火し、手書き再帰の更新漏れリスクが構造的に消滅する
6. Hono ベンチマーク後退ゼロ、`./scripts/check-file-lines.sh` パス、`cargo test` 全 pass

## Scope

### In Scope

- `Pattern::TupleStruct` / `UnitStruct` / `Struct` の `path: Vec<String>` を構造化 `ctor` フィールドに置換
- `PatternCtor` enum 新設 (variants: `Builtin(BuiltinVariant)`, `UserEnumVariant { enum_ty: UserTypeRef, variant: String }`, `UserStruct(UserTypeRef)`)
- `IrVisitor::walk_pattern` / `IrFolder::walk_pattern` への `walk_pattern_ctor` 追加 (`visit_user_type_ref` / `fold_user_type_ref` を経由)
- `Pattern::is_none_unit` / `Pattern::none()` / `Pattern::some_binding` 等のヘルパー API の構造化追従
- `external_struct_generator/mod.rs` の walker 完全 IrVisitor 化:
  - `collect_type_refs_from_pattern` 削除 → `TypeRefCollector::visit_pattern` override
  - `collect_type_refs_from_rust_type` を `TypeRefCollector::visit_rust_type` override に統合 (Self 除外 / DynTrait 登録の特殊ロジックを移行)
  - `collect_type_refs_from_stmts` / `_from_stmt` / `_from_match_arm` / `_from_item` / `_from_method` / `_from_type_params` を削除 → `IrVisitor` のデフォルト walk に委ねる
  - `Expr::StructInit::name` の特殊登録は `TypeRefCollector::visit_expr` の既存 override に集約
- `PATTERN_LANG_BUILTINS` 定数削除 (構造的に builtin と user variant が区別されるため不要)
- Transformer 全 Pattern 構築サイト書き換え (推定 ~30 サイト、I-377 と同規模)
- 全関連テストの新形式追従

### Out of Scope

- `Item::StructInit::name: String` の `"Enum::Variant"` 構造化 — 別 broken window (TODO `[broken-window:StructInit::name]`)、独立 PRD
- `Pattern::Literal(Expr)` の Expr 部分の更なる構造化 — I-377 / I-378 で十分構造化済み
- I-379 (Lit::Null 構造化) — 独立 PRD、本 PRD と並行可能 (依存関係なし)

## Design

### Technical Approach

#### 1. `PatternCtor` 新型導入

`src/ir/pattern.rs` に追加:

```rust
use super::{BuiltinVariant, UserTypeRef};

/// Pattern の constructor 種別を構造化する。
///
/// I-380 で I-377 の暫定形 `path: Vec<String>` を分解した結果。各 variant は
/// 単一の意味論を担い、walker は variant 形状から user type 参照を構造的に判定
/// できる (`PATTERN_LANG_BUILTINS` ハードコード除外リスト不要)。
///
/// `CallTarget` の対応する分類との対称性:
/// - `Builtin(BuiltinVariant)` ↔ `CallTarget::BuiltinVariant(_)`
/// - `UserEnumVariant { enum_ty, variant }` ↔ `CallTarget::UserEnumVariantCtor { .. }`
/// - `UserStruct(ty)` ↔ `CallTarget::UserTupleCtor(_)` (callable interface tuple struct)
#[derive(Debug, Clone, PartialEq)]
pub enum PatternCtor {
    /// `Some(_)` / `None` / `Ok(_)` / `Err(_)` — Option/Result builtin variant
    Builtin(BuiltinVariant),

    /// `Color::Red(_)` — ユーザー定義 enum の variant constructor
    UserEnumVariant {
        enum_ty: UserTypeRef,
        variant: String,
    },

    /// `Foo { .. }` / `Wrapper(_)` — ユーザー定義 struct (callable interface tuple struct
    /// または non-enum struct pattern)
    UserStruct(UserTypeRef),
}
```

#### 2. `Pattern` 既存 variant の `path` フィールド置換

`Pattern::TupleStruct` / `Struct` / `UnitStruct` の `path: Vec<String>` を `ctor: PatternCtor` に置換:

```rust
pub enum Pattern {
    // ... 既存 ...
    TupleStruct {
        ctor: PatternCtor,
        fields: Vec<Pattern>,
    },
    Struct {
        ctor: PatternCtor,
        fields: Vec<(String, Pattern)>,
        rest: bool,
    },
    UnitStruct {
        ctor: PatternCtor,
    },
}
```

`is_none_unit` は構造的判定に置換:
```rust
pub fn is_none_unit(&self) -> bool {
    matches!(
        self,
        Pattern::UnitStruct { ctor: PatternCtor::Builtin(BuiltinVariant::None) }
    )
}
```

#### 3. `IrVisitor` / `IrFolder` 拡張

`walk_pattern` の対応 arm から `walk_pattern_ctor` を呼ぶ:

```rust
pub fn walk_pattern_ctor<V: IrVisitor + ?Sized>(v: &mut V, ctor: &PatternCtor) {
    match ctor {
        PatternCtor::UserEnumVariant { enum_ty, .. } => v.visit_user_type_ref(enum_ty),
        PatternCtor::UserStruct(ty) => v.visit_user_type_ref(ty),
        PatternCtor::Builtin(_) => {} // 通知不要
    }
}
```

`fold` 側も対称に `walk_pattern_ctor` 関数を追加し、`fold_user_type_ref` を経由。

#### 4. walker 完全 IrVisitor 化

`TypeRefCollector` を拡張:

```rust
impl<'a> IrVisitor for TypeRefCollector<'a> {
    fn visit_user_type_ref(&mut self, r: &UserTypeRef) {
        // user type 参照は無条件登録 (型レベル保証)
        self.refs.insert(r.as_str().to_string());
    }

    fn visit_expr(&mut self, expr: &Expr) {
        // StructInit::name の特殊扱い (Out of Scope の broken window)
        if let Expr::StructInit { name, .. } = expr {
            if name != "Self" {
                self.refs.insert(name.clone());
            }
        }
        crate::ir::visit::walk_expr(self, expr);
    }

    fn visit_rust_type(&mut self, ty: &RustType) {
        // Named 型名の登録 + Self 除外 + DynTrait 登録 を統合
        match ty {
            RustType::Named { name, .. } if name != "Self" => {
                self.refs.insert(name.clone());
            }
            RustType::DynTrait(name) => {
                self.refs.insert(name.clone());
            }
            _ => {}
        }
        crate::ir::visit::walk_rust_type(self, ty);
    }

    // visit_pattern / visit_item / visit_stmt 等はデフォルト walk に委ねる
    // (PatternCtor の user variant は walk_pattern_ctor → visit_user_type_ref 経由で発火)
}
```

すべての `collect_type_refs_from_*` スタンドアロン関数を削除し、エントリーポイントを `TypeRefCollector::visit_item` 経由に統一する。`collect_type_refs_from_item` は薄いラッパー (`let mut c = TypeRefCollector { refs }; c.visit_item(item);`) として残すか、呼び出し側を直接 `TypeRefCollector` に書き換える。

#### 5. Transformer 構築サイト書き換え

I-377 で構造化された `Pattern` 構築サイト (推定 ~30 箇所) を新形式に追従。`Pattern::UnitStruct { path: vec!["None"] }` → `Pattern::UnitStruct { ctor: PatternCtor::Builtin(BuiltinVariant::None) }` 等。

PatternCtor の判定ロジック: 既存の `path` 構築箇所では既に enum 名 / variant 名 / builtin 区別の情報を持っているため、構築時点で正しい variant を選択できる (transformer が registry を参照済み)。

### Design Integrity Review

- **Higher-level consistency**: I-378 の `CallTarget` 構造化と完全対称。同じ broken window class を Pattern 側で解消
- **DRY**: `BuiltinVariant` / `UserTypeRef` を再利用 (新型は `PatternCtor` 1 つのみ)。walker の手書き再帰群が単一 `TypeRefCollector` に集約
- **Orthogonality**: `PatternCtor` の 3 variant が単一意味論を担う。`is_none_unit` 等のヘルパーが構造的判定に集約
- **Coupling**: 依存方向は `walker → IrVisitor → ir/pattern.rs`。新規循環なし

### Impact Area

**変更ファイル** (推定):
- `src/ir/pattern.rs` — `PatternCtor` 追加 + `Pattern` variant 構造変更 + ヘルパー追従
- `src/ir/mod.rs` — `pub use pattern::PatternCtor;`
- `src/ir/visit.rs` — `walk_pattern` の ctor 呼び出し + `walk_pattern_ctor` 新設
- `src/ir/visit_tests.rs` — テスト追従 + 新フック発火テスト
- `src/ir/fold.rs` — 対称な fold 拡張
- `src/ir/test_fixtures.rs::all_patterns` — 新形式追従
- `src/ir/tests/expr_tests.rs` — テスト追従
- `src/generator/patterns.rs` — render_pattern 新形式追従
- `src/generator/patterns/tests` — テスト追従
- `src/pipeline/external_struct_generator/mod.rs` — `collect_type_refs_from_*` 8 関数削除 + `TypeRefCollector` 拡張
- `src/pipeline/external_struct_generator/tests/*.rs` — テスト追従
- Transformer 配下 ~30 サイトの Pattern 構築書き換え (`switch.rs`, `error_handling.rs`, `convert_var_pattern`, etc.)
- 全関連テスト

**規模見積もり**: I-377 同等 (~50 ファイル、行数 ~500-800 行の変更)

### Semantic Safety Analysis

| 変更 | 旧 | 新 | 意味論差 |
|---|---|---|---|
| `Pattern::UnitStruct { path: vec!["None"] }` → `UnitStruct { ctor: PatternCtor::Builtin(None) }` | render: `None` | render: `None` | **完全一致** (Safe) |
| `Pattern::TupleStruct { path: vec!["Color","Red"], .. }` → `ctor: UserEnumVariant { Color, Red }` | render: `Color::Red(_)` | render: `Color::Red(_)` | **完全一致** (Safe) |
| walker `PATTERN_LANG_BUILTINS` 削除 | string 比較で除外 | 型レベルで除外 | **完全一致** (Safe、より厳密) |
| 手書き `collect_type_refs_from_*` 削除 → IrVisitor デフォルト walk | 各関数が match arm を網羅 | walk_* が match arm を網羅 | **要慎重検証**: walk_* と手書きの意味論差を T0 で全 IR ノードに対して比較 |

**T0 における必須事前検証**: 既存 walker と新 `TypeRefCollector` の出力が任意の IR に対して一致することを property test で確認。具体的には Hono fixture 全 158 ファイルに対して両 walker を並走させ、refs 集合の差分が空であることを確認。

## Task List

### T0: baseline 取得 + 旧/新 walker 並走比較準備

- **Work**: `./scripts/hono-bench.sh` → baseline 退避。旧 walker と新 walker (中間状態) を並走して refs 差分を計測する property test を準備
- **Completion criteria**: baseline 退避完了、property test スケルトン作成

### T1: `PatternCtor` 型追加 + Pattern variant 構造変更

- **Work**: `src/ir/pattern.rs` に `PatternCtor` enum 追加。`Pattern::TupleStruct` / `Struct` / `UnitStruct` の `path` を `ctor: PatternCtor` に置換
- **Completion criteria**: `cargo check` 大量エラー (期待通り) + `PatternCtor` 単体テスト pass

### T2: `IrVisitor` / `IrFolder` 拡張

- **Work**: `walk_pattern_ctor` 新設、`walk_pattern` から呼び出し。fold 側も対称に
- **Completion criteria**: visit/fold 単体テスト pass、新 hook 発火テスト追加

### T3: Pattern ヘルパー追従

- **Work**: `Pattern::is_none_unit` / `Pattern::none()` / `Pattern::some_binding` 等を新形式に書き換え
- **Completion criteria**: 既存 helper の単体テスト pass

### T4: generator `render_pattern` 追従

- **Work**: `PatternCtor` 各 variant の rendering を実装 (`Builtin(_).as_rust_str()` / `UserEnumVariant` → `enum_ty::variant` / `UserStruct` → `ty`)
- **Completion criteria**: 各 variant の rendering 単体テスト pass

### T5: walker 完全 IrVisitor 化

- **Work**: `TypeRefCollector` 拡張 (`visit_rust_type` / `visit_pattern` を override)。8 個の `collect_type_refs_from_*` 関数削除。エントリーポイント書き換え。`PATTERN_LANG_BUILTINS` 削除
- **Completion criteria**: walker_tests 全 pass、`grep PATTERN_LANG_BUILTINS src/` 0 ヒット

### T6: Transformer Pattern 構築サイト全書き換え

- **Work**: `switch.rs` / `error_handling.rs` / `convert_var_pattern` / その他 Pattern 構築箇所を新形式に置換
- **Completion criteria**: `cargo check` pass、`grep -rn 'path:.*vec!\[".*"\]' src/transformer/.*pattern' src/` 0 ヒット (検索パターンは適宜調整)

### T7: テスト全面追従

- **Work**: 既存テストの Pattern fixture 書き換え
- **Completion criteria**: `cargo test --lib` 全 pass

### T8: walker property 比較テスト

- **Work**: T0 で準備した property test を実行 → 旧 walker と新 walker の出力が全 158 fixture で一致することを確認
- **Completion criteria**: 差分ゼロ。差分が出る場合は要 root cause 分析

### T9: Hono ベンチ + quality-check

- **Work**: `./scripts/hono-bench.sh` 後退ゼロ確認 + clippy + fmt + 行数 + llvm-cov
- **Completion criteria**: 全項目 pass

### T10: TODO + plan.md 更新

- **Work**: `[broken-window:PATTERN_LANG_BUILTINS]` / `[follow-up:walker-irvisitor]` を TODO から削除。`plan.md` 更新
- **Completion criteria**: ドキュメント更新完了

## Test Plan

### T1 単体 (新規)

- `PatternCtor::Builtin(BuiltinVariant::Some)` 等 4 builtin variant 構築
- `PatternCtor::UserEnumVariant { enum_ty, variant }` 構築
- `PatternCtor::UserStruct(ty)` 構築
- 各 variant の `Eq` / `Hash` round-trip

### T2 hook firing (新規)

- `walk_pattern_ctor` の `UserEnumVariant` / `UserStruct` で `visit_user_type_ref` 発火
- `Builtin` では非発火
- fold 側でも対称に `fold_user_type_ref` 経由を確認

### T5 walker (新規)

- `Pattern::TupleStruct { ctor: PatternCtor::UserEnumVariant { Color, Red }, .. }` を含む match で walker が `Color` を登録
- `Pattern::UnitStruct { ctor: PatternCtor::Builtin(None) }` を含む match で walker が何も登録しない (Some/Ok/Err も同様)
- 旧 walker との property 比較

### T8 統合 (新規)

- `tests/pattern_structuring_property_test.rs`: 158 Hono fixture を旧/新 walker で並走、refs HashSet が一致

## Completion Criteria

- [ ] `grep -rn "PATTERN_LANG_BUILTINS" src/` 0 ヒット
- [ ] `grep -rn 'collect_type_refs_from_pattern\|collect_type_refs_from_rust_type\|collect_type_refs_from_stmts\|collect_type_refs_from_stmt\|collect_type_refs_from_match_arm\|collect_type_refs_from_method\|collect_type_refs_from_type_params' src/` 0 ヒット (`collect_type_refs_from_item` のみエントリポイントとして残存可)
- [ ] `grep -rn 'path:.*vec!\["None"\]\|path:.*vec!\["Some"' src/` 0 ヒット (Pattern の path フィールド廃止)
- [ ] `Pattern::is_none_unit` の実装が `path[0] == "None"` 文字列比較を使わない
- [ ] `cargo test` 全 pass (新規テスト ≥ +30 件)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 警告 0
- [ ] `cargo fmt --all --check` pass
- [ ] `./scripts/check-file-lines.sh` pass
- [ ] `./scripts/hono-bench.sh` 後退なし (114/158 / 157/158 / err 54)
- [ ] T8 property test で旧/新 walker の出力が完全一致
- [ ] `TODO` から `[broken-window:PATTERN_LANG_BUILTINS]` および `[follow-up:walker-irvisitor]` セクションが削除されている

## References

- I-377 PRD: `Batch 11c-fix-2-b` の `Pattern` 構造化 + `IrVisitor` 導入 (本 PRD はその完全化)
- I-378 PRD: `Batch 11c-fix-2-d` の `CallTarget` / `Expr` 構造化 (本 PRD と対称な call/value 側の broken window 撲滅)
- I-378 T7: `collect_type_refs_from_expr` の `IrVisitor` 化 (本 PRD で残り 8 関数を完全化)
- `.claude/rules/pipeline-integrity.md`: IR に display-formatted 文字列を保存禁止
- `src/pipeline/external_struct_generator/mod.rs:526` の `PATTERN_LANG_BUILTINS` 暫定コメント
- `src/ir/pattern.rs:114` の `is_none_unit` 暫定実装
