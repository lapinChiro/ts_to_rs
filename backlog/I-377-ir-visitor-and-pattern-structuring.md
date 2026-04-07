# I-377: walker / substitute の IrVisitor 化 + MatchPattern / Pattern 文字列の構造化 IR 化

## Background

Batch 11c-fix と I-375 の完了により、IR の構造化は大きく前進したが、**2 つの層で「IR に display-formatted 文字列を保存してはならない」という pipeline-integrity ルールへの違反**と、**手書き再帰の分散**が残存している。

### 問題 1: IR に pattern 文字列が残存している

IR 内の以下 5 箇所では、Rust の pattern が **`String`** として保存されている:

- `src/ir/mod.rs:308` `MatchPattern::Verbatim(String)`
- `src/ir/mod.rs:525` `Stmt::WhileLet { pattern: String, .. }`
- `src/ir/mod.rs:570` `Stmt::IfLet { pattern: String, .. }`
- `src/ir/mod.rs:846` `Expr::IfLet { pattern: String, .. }`
- `src/ir/mod.rs:906` `Expr::Matches { pattern: String, .. }`
- `src/ir/mod.rs:924` (Expr::IfLet の重複 — 要確認)

これらは pipeline-integrity ルール「IR を構造化データとして表現する。display-formatted 文字列を IR に保存禁止」への直接違反であり、次の broken window を生んでいる:

1. **walker が文字列パーサに依存**: `collect_type_refs_from_verbatim_pattern`（`src/pipeline/external_struct_generator/mod.rs:520-537`）は pattern 文字列先頭の uppercase identifier を正規表現的に抽出し refs に登録する **uppercase-head ヒューリスティック**。
2. **ビルトイン variant のハードコード除外**: uppercase ヒューリスティックは `"Some"` / `"None"` / `"Ok"` / `"Err"` を refs に登録するため、`RUST_BUILTIN_TYPES` に当該 4 エントリを**明示コメント付きで保持**している（`src/pipeline/external_struct_generator/mod.rs:31-36`）。lowercase クラス名（`class myClass`）の場合 false negative を起こす潜在バグ。
3. **generator 側は文字列をそのまま dump**: `src/generator/statements/mod.rs:200` / `src/generator/expressions/mod.rs:378` で `Verbatim(s) => s.clone()`。transformer で format 済みの文字列が IR を素通りして generator で再生される anti-pattern。

### 問題 2: 手書き再帰 walker が複数箇所に分散

IR の全要素再帰走査は現在、以下の **2 系統の独立実装**で存在する:

- `src/pipeline/external_struct_generator/mod.rs` の `collect_type_refs_from_item / _stmt / _expr / _rust_type / _type_params / _method / _match_arm / _verbatim_pattern`（約 400 行、本セッションで追加）
- `src/ir/substitute.rs` の `TypeParam::substitute / RustType::substitute / Stmt::substitute / Expr::substitute / Item::substitute`（595 行）
- 散発的な再帰: `src/transformer/mod.rs:756 expr_contains_runtime_typeof` / `stmts_contain_runtime_typeof` / `items_contain_regex` / `stmts_contain_regex` 等

これらはすべて「IR 全要素再帰」という同一パターンを独立に実装しており、新しい variant 追加時に **全箇所を個別更新する必要がある**。Rust の `match` 網羅性チェックで漏れは compile 時検出されるが、変更コストが線形に増加する。実際、本セッションで `collect_type_refs_from_item` に type_params constraint walking と verbatim pattern walking を追加した際、`substitute.rs` には反映されていない。

### なぜ同一 PRD で対処するか

問題 1（構造化）と問題 2（visitor 化）は**相互依存**する:

- 問題 2 の IrVisitor を導入した段階で、`visit_match_pattern` のシグネチャを `&MatchPattern` ではなく `&Pattern`（構造化後）にする必要がある。先に visitor を導入すると、problem 1 の解消時に visitor API を再設計する rework が発生。
- 逆に問題 1 を先に行っても、walker が依然として手書き再帰である限り「構造化した意味」が半減する（broken window 残存）。

したがって「構造化された Pattern に対する IrVisitor を一発で導入する」のが rework 最小の順序。

## Goal

以下を **同時に** 達成する:

1. **IR に pattern 文字列が一切存在しない**: `grep -rn 'pattern: String\|MatchPattern::Verbatim' src/ir/` が 0 件
2. **uppercase-head ヒューリスティックコードの全削除**: `collect_type_refs_from_verbatim_pattern` 関数および同等ロジックが存在しない
3. **`RUST_BUILTIN_TYPES` から `Some` / `None` / `Ok` / `Err` が除去されている**: `src/pipeline/external_struct_generator/mod.rs` の当該定数から当該 4 エントリが消え、コメントも更新されている
4. **`IrVisitor` trait が `src/ir/visit.rs` に存在する**: Item / Stmt / Expr / RustType / Pattern / MatchArm / TypeParam / Method を全カバーし、default 実装で walk 関数に委譲する形
5. **`collect_type_refs_*` が `TypeRefCollector: IrVisitor` として再実装されている**: 手書き再帰コードが削除されている
6. **`ir::substitute.rs` が `Substitute: IrVisitor`（あるいは同型の visitor）として再実装されている**: 手書き再帰コードが削除されている
7. **散発的再帰の visitor 化**: `expr_contains_runtime_typeof` / `stmts_contain_runtime_typeof` / `items_contain_regex` / `stmts_contain_regex` が `IrVisitor` ベースに統合されている
8. **後退ゼロ**: 既存全テスト pass、Hono ベンチ clean 率 / dir compile / error instances すべて現状維持または改善

## Scope

### In Scope

- `src/ir/mod.rs` の pattern field 型変更（String → 構造化 `Pattern` enum）
- 新設 `src/ir/pattern.rs`（または `mod.rs` 内）の `Pattern` enum 定義（Q2 A の範囲: Rust pattern grammar 相当を網羅）
- 新設 `src/ir/visit.rs` の `IrVisitor` trait + `walk_*` 関数群
- `src/transformer/` の全 pattern 構築サイトの書き換え（`MatchPattern::Verbatim(format!(...))` → 構造化 Pattern 構築）
- `src/generator/` の pattern rendering ロジック（構造化 Pattern → Rust 文字列）
- `src/pipeline/external_struct_generator/mod.rs::collect_type_refs_*` の IrVisitor ベース再実装 + uppercase ヒューリスティック削除 + RUST_BUILTIN_TYPES の Some/None/Ok/Err 除去
- `src/ir/substitute.rs` の IrVisitor ベース再実装
- `src/transformer/mod.rs::expr_contains_runtime_typeof` 等の散発再帰の IrVisitor 化
- 全関連テストの更新と追加

### Out of Scope

- 新しい Rust pattern 構文サポートの追加（例: Range、Box pattern など、現状 IR が表現していないもの）は必要に応じて Pattern enum に含めるが、**transformer 側で生成しない限り dead code**。YAGNI に従い、現状の構築サイトが使っている構文のみ構造化し、Q2 A の範囲（Or / Binding / Ref 等）は将来の拡張点として enum variant を用意するが generator/transformer での使用は未実装でも可
- I-376（per-file 外部型 stub 重複）の解消 — Batch 11c-fix-2-c に後置
- I-374（Rust 予約語 sanitize）の完全修正 — 独立 PRD
- Hono ベンチ clean 率の向上（本 PRD は構造リファクタリングであり、変換出力は現状維持）

## Design

### Technical Approach

#### Phase A: `Pattern` enum 設計と IR 定義

`src/ir/pattern.rs` に以下を新規作成:

```rust
/// Rust pattern grammar を構造化表現した IR ノード。
///
/// `MatchPattern` / `Stmt::IfLet` / `Stmt::WhileLet` / `Expr::IfLet` / `Expr::Matches`
/// の `pattern` フィールドで使用される。
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_`
    Wildcard,
    /// リテラル値（`1`, `"hello"`, `true`）
    Literal(Expr),
    /// 識別子束縛（`x`, `mut x`）— ただし Rust の pattern 文法では enum variant 名と
    /// 区別困難なため、transformer は原則 `Binding` か `EnumVariant` を明示生成する
    Binding {
        name: String,
        is_mut: bool,
        /// サブパターン（`x @ 1..=5`）
        subpat: Option<Box<Pattern>>,
    },
    /// タプル構造体 variant（`Some(x)`, `Color::Red(x, y)`, `Ok(v)`）
    TupleStruct {
        /// 完全修飾パス。セグメント列（例: `["Color", "Red"]`, `["Some"]`）
        path: Vec<String>,
        fields: Vec<Pattern>,
    },
    /// 構造体 variant（`Shape::Circle { radius, .. }`, `Foo { x, y }`）
    Struct {
        path: Vec<String>,
        fields: Vec<(String, Pattern)>,
        /// 末尾の `..` の有無
        rest: bool,
    },
    /// Unit variant（`None`, `Color::Green`）
    UnitStruct {
        path: Vec<String>,
    },
    /// Or パターン（`a | b | c`）
    Or(Vec<Pattern>),
    /// Range パターン（`1..=5`）
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    /// 参照パターン（`&x`）
    Ref {
        mutable: bool,
        inner: Box<Pattern>,
    },
    /// タプルパターン（`(a, b, c)`）
    Tuple(Vec<Pattern>),
}
```

**設計上の制約**:
- `path` は `Vec<String>`（文字列を `::` で結合しない。generator 段階で join）。これにより「enum 名」「variant 名」へのプログラム的アクセスが可能になり、walker の uppercase-head ヒューリスティックが不要になる
- `TupleStruct::path = vec!["Some"]` のように単一セグメントもサポート
- `UnitStruct` を `TupleStruct { fields: vec![] }` と統合せず分離することで generator のレンダリング分岐を明示化

`src/ir/mod.rs` の以下を変更:

- `MatchPattern` enum を削除（`Pattern` に統合）
- `Stmt::IfLet::pattern: String` → `Pattern`
- `Stmt::WhileLet::pattern: String` → `Pattern`
- `Expr::IfLet::pattern: String` → `Pattern`
- `Expr::Matches::pattern: String` → `Pattern`
- `MatchArm::patterns: Vec<MatchPattern>` → `Vec<Pattern>`

#### Phase B: `IrVisitor` trait

`src/ir/visit.rs` を新規作成:

```rust
use super::*;

pub trait IrVisitor {
    fn visit_item(&mut self, item: &Item) { walk_item(self, item); }
    fn visit_stmt(&mut self, stmt: &Stmt) { walk_stmt(self, stmt); }
    fn visit_expr(&mut self, expr: &Expr) { walk_expr(self, expr); }
    fn visit_rust_type(&mut self, ty: &RustType) { walk_rust_type(self, ty); }
    fn visit_pattern(&mut self, pat: &Pattern) { walk_pattern(self, pat); }
    fn visit_match_arm(&mut self, arm: &MatchArm) { walk_match_arm(self, arm); }
    fn visit_type_param(&mut self, tp: &TypeParam) { walk_type_param(self, tp); }
    fn visit_method(&mut self, m: &Method) { walk_method(self, m); }
}

pub fn walk_item<V: IrVisitor + ?Sized>(v: &mut V, item: &Item) { /* 全 variant */ }
pub fn walk_stmt<V: IrVisitor + ?Sized>(v: &mut V, stmt: &Stmt) { /* 全 variant */ }
pub fn walk_expr<V: IrVisitor + ?Sized>(v: &mut V, expr: &Expr) { /* 全 variant */ }
pub fn walk_rust_type<V: IrVisitor + ?Sized>(v: &mut V, ty: &RustType) { /* 全 variant */ }
pub fn walk_pattern<V: IrVisitor + ?Sized>(v: &mut V, pat: &Pattern) { /* 全 variant */ }
pub fn walk_match_arm<V: IrVisitor + ?Sized>(v: &mut V, arm: &MatchArm) { /* ... */ }
pub fn walk_type_param<V: IrVisitor + ?Sized>(v: &mut V, tp: &TypeParam) { /* ... */ }
pub fn walk_method<V: IrVisitor + ?Sized>(v: &mut V, m: &Method) { /* ... */ }
```

SWC の `swc_ecma_visit::Visit` と同型の trait-based visitor pattern。各 visitor 実装は必要なノードだけ override する。

#### Phase C: `TypeRefCollector` 再実装

`src/pipeline/external_struct_generator/type_ref_collector.rs`（新規）に:

```rust
pub(crate) struct TypeRefCollector<'a> {
    pub refs: &'a mut HashSet<String>,
}

impl<'a> IrVisitor for TypeRefCollector<'a> {
    fn visit_rust_type(&mut self, ty: &RustType) {
        if let RustType::Named { name, .. } = ty {
            if name != "Self" {
                self.refs.insert(name.clone());
            }
        }
        walk_rust_type(self, ty); // 子も走査
    }

    fn visit_expr(&mut self, expr: &Expr) {
        if let Expr::FnCall { target: CallTarget::Path { type_ref: Some(t), .. }, .. } = expr {
            self.refs.insert(t.clone());
        }
        if let Expr::StructInit { name, .. } = expr {
            if name != "Self" { self.refs.insert(name.clone()); }
        }
        walk_expr(self, expr);
    }

    fn visit_pattern(&mut self, pat: &Pattern) {
        match pat {
            Pattern::TupleStruct { path, .. }
            | Pattern::Struct { path, .. }
            | Pattern::UnitStruct { path } => {
                // 先頭セグメントを refs に登録。Some/None/Ok/Err は path = vec!["Some"] 等
                // で構造的に識別されるため、walker が呼び出し側で分離判断可能。
                // → TypeRegistry の is_external チェックで builtin は後段フィルタ。
                if let Some(first) = path.first() {
                    self.refs.insert(first.clone());
                }
            }
            _ => {}
        }
        walk_pattern(self, pat);
    }
}
```

**重要**: `TypeRefCollector::visit_pattern` では `Some / None / Ok / Err` も登録される。しかしこれらは `TypeRegistry::is_external` で `Some` / `Option` / ... に正規化済みの external types としてフィルタリングされるべき。ここが I-377 の核心:

**Some/None/Ok/Err を `RUST_BUILTIN_TYPES` から削除するため、後段フィルタを構造的なものに置き換える**:
- 現在: `RUST_BUILTIN_TYPES` に文字列マッチ + `TypeRegistry::is_external`
- 変更後: `TypeRegistry` に `Some` / `None` / `Ok` / `Err` を「`Option` / `Result` のコンストラクタ名」として登録するか、あるいは walker が `Pattern::TupleStruct { path: vec!["Some"] }` の段階で `Some`/`None` を登録しないよう明示ガードする

**設計判断**: 後者を採用。理由は `Some`/`None`/`Ok`/`Err` は Rust の言語レベル組み込みであり、`TypeRegistry` に「型」として登録すると抽象レベルが混乱する。代わりに `TypeRefCollector::visit_pattern` で以下の明示ガードを置く:

```rust
// Option / Result のコンストラクタは Rust 言語レベル組み込みであり、
// external type として struct 生成する対象ではない。pattern 上の constructor
// 名が `Some`/`None`/`Ok`/`Err` の場合は登録しない。
const PATTERN_LANG_BUILTINS: &[&str] = &["Some", "None", "Ok", "Err"];
if let Some(first) = path.first() {
    if !PATTERN_LANG_BUILTINS.contains(&first.as_str()) {
        self.refs.insert(first.clone());
    }
}
```

これは **uppercase-head ヒューリスティックではない**ことに注意: 文字列パースではなく構造化された `path: Vec<String>` の最初のセグメントを直接比較する。lowercase クラス名の場合も `path = vec!["myClass"]` → `"myClass"` を正しく refs に登録する。`RUST_BUILTIN_TYPES` からは `Some/None/Ok/Err` を除去し、このガードが唯一の除外ポイントになる。

> **DRY 検討**: `PATTERN_LANG_BUILTINS` と `RUST_BUILTIN_TYPES` は重複ではない。前者は「Option/Result の variant コンストラクタ」、後者は「型名」であり、抽象レベルが異なる。Type 名としての `Option` / `Result` は `RUST_BUILTIN_TYPES` に残す。

#### Phase D: `Substitute` 再実装

`src/ir/substitute.rs` を `IrVisitor` ベースに書き換え…たいところだが、substitute は **変換 visitor**（in-place 変更ではなく新 IR を生成）であり、read-only な `IrVisitor` のシグネチャには収まらない。

**対処**: `IrVisitorMut` trait を別途定義し、`fn visit_*(&mut self, node: &mut T)` シグネチャで in-place mutation visitor を提供するか、あるいは `Folder` trait として `fn fold_*(&mut self, node: T) -> T` を提供する。現在の `substitute.rs` は `&self` + `-> Self` の純関数形式なので `Folder` が自然。

```rust
pub trait IrFolder {
    fn fold_item(&mut self, item: Item) -> Item { walk_item(self, item) }
    fn fold_stmt(&mut self, stmt: Stmt) -> Stmt { walk_stmt(self, stmt) }
    fn fold_expr(&mut self, expr: Expr) -> Expr { walk_expr(self, expr) }
    fn fold_rust_type(&mut self, ty: RustType) -> RustType { walk_rust_type(self, ty) }
    fn fold_pattern(&mut self, pat: Pattern) -> Pattern { walk_pattern(self, pat) }
    // ...
}
```

`Substitute { bindings: HashMap<String, RustType> }` が `IrFolder` を実装し、`fold_rust_type` で `Named { name } if bindings.contains_key(name) => bindings[name].clone()` を行う。その他のノードは default `walk_*` に委譲。既存の `*.substitute(&bindings)` の呼び出しは `Substitute::new(&bindings).fold_*(node)` に置き換える。

> `IrVisitor` と `IrFolder` の 2 trait に分ける理由: read-only 走査と in-place 変換は本質的に異なる責務であり、1 trait に統合すると実装者は不要な default を埋める負担が生じる。SWC も `Visit` と `Fold` を分けている。

#### Phase E: 散発再帰の統合

`src/transformer/mod.rs::expr_contains_runtime_typeof` → `RuntimeTypeofDetector: IrVisitor`。`visit_expr` で `Expr::RuntimeTypeof` を検出したら `self.found = true`。同様に `RegexDetector`。

#### Phase F: uppercase ヒューリスティック削除と builtin 除外整理

- `collect_type_refs_from_verbatim_pattern` 関数を削除
- `collect_type_refs_from_match_arm` の uppercase-head 判定コードを削除
- `RUST_BUILTIN_TYPES` から `"Some", "None", "Ok", "Err"` を削除し、コメントを「I-377 で構造化 Pattern に移行済み」に更新
- `TypeRefCollector::visit_pattern` の `PATTERN_LANG_BUILTINS` 明示ガードが唯一の除外ポイント

### Design Integrity Review

**1. 凝集度 (Cohesion)**:
- `Pattern` enum: 「Rust pattern grammar の IR 表現」という単一責務
- `IrVisitor` trait: 「IR の read-only 走査骨格」という単一責務
- `IrFolder` trait: 「IR の変換走査骨格」という単一責務
- `TypeRefCollector`: 「IR から型参照名を収集する」という単一責務
- `Substitute`: 「型パラメータを具体型に置換する」という単一責務

**2. 責務の分離 (Separation of Concerns)**:
- **走査と判定の分離**: `walk_*` 関数が走査を担い、visitor 実装が判定（refs 登録、型置換）を担う。Phase C 以前は `collect_type_refs_*` で両責務が混在
- **文字列化の所在**: Pattern の文字列化は generator が担う。transformer と IR は構造化データのみ扱う
- **builtin 判定の所在**: `PATTERN_LANG_BUILTINS` は `TypeRefCollector` 内部（walker サブシステム）にクローズ。`RUST_BUILTIN_TYPES` は type resolution サブシステムに残る。抽象レベル分離

**3. DRY**:
- **解消**: IR 全要素再帰パターンが 2 系統（external_struct_generator + substitute）+ 散発 4 箇所に重複していたのが、`walk_*` 関数群 1 箇所に集約される
- **残存する意図的重複**: `RUST_BUILTIN_TYPES`（type name フィルタ）と `PATTERN_LANG_BUILTINS`（pattern constructor フィルタ）は異なる知識のため、統合せず分離維持

**4. Broken window**:
- 本 PRD の発端そのものが broken window 群（pattern 文字列、uppercase ヒューリスティック、Some/None/Ok/Err ハードコード）の解消
- 新規発見: `Expr::StructInit::name != "Self"` / `collect_type_refs_from_rust_type::Named::name != "Self"` に同じ `Self` 除外ロジックが 2 箇所に存在（DRY 違反）。本 PRD で `walk_rust_type` に集約する際に解消
- `src/ir/mod.rs:924` に重複した `pattern: String` がある可能性 → Phase A で grep 再確認

### Impact Area

- `src/ir/mod.rs`: `MatchPattern` 削除、`Stmt::IfLet/WhileLet`・`Expr::IfLet/Matches` の pattern field 型変更
- `src/ir/pattern.rs`: 新規（`Pattern` enum）
- `src/ir/visit.rs`: 新規（`IrVisitor` trait + `walk_*`）
- `src/ir/fold.rs`: 新規（`IrFolder` trait + `walk_*` mut 版）
- `src/ir/substitute.rs`: `IrFolder` ベースに全面再実装
- `src/transformer/statements/control_flow.rs`: `MatchPattern::Verbatim(format!(...))` 全箇所を構造化
- `src/transformer/expressions/literals.rs`: pattern 構築サイトの構造化
- `src/transformer/expressions/patterns.rs`: `Expr::Matches` 構築サイト
- `src/transformer/expressions/mod.rs`: `Expr::IfLet` 構築サイト
- `src/transformer/statements/error_handling.rs`: `Stmt::IfLet` 構築サイト
- `src/transformer/statements/loops.rs`: `Stmt::IfLet/WhileLet` 構築サイト
- `src/transformer/functions/helpers.rs`: pattern clone サイト
- `src/transformer/statements/mutability.rs`: pattern 参照サイト
- `src/transformer/mod.rs`: `expr_contains_runtime_typeof` / `stmts_contain_runtime_typeof` / `items_contain_regex` / `stmts_contain_regex` の IrVisitor 化
- `src/generator/statements/mod.rs`: Pattern rendering 実装
- `src/generator/expressions/mod.rs`: Pattern rendering 実装
- `src/pipeline/external_struct_generator/mod.rs`: `collect_type_refs_*` 削除、`TypeRefCollector` 新設、`RUST_BUILTIN_TYPES` から Some/None/Ok/Err 削除
- `src/pipeline/external_struct_generator/type_ref_collector.rs`: 新規
- `src/pipeline/placement.rs`: `collect_type_refs_from_*` 呼び出し側を `TypeRefCollector` に移行
- `src/pipeline/external_struct_generator/tests.rs`: テスト更新
- 各種 transformer tests: pattern 参照の期待値を構造化 Pattern に更新

### Semantic Safety Analysis

**Not applicable — no type fallback changes.** 本 PRD は純粋な内部表現リファクタリングであり、型解決ロジック・型 fallback は一切変更しない。transformer が生成する構造化 Pattern は generator で同一の Rust 文字列にレンダリングされる。変換出力は全テスト/ベンチで後退ゼロであることが完了条件。

検証方針:
1. Pattern 構造化前後で生成された Rust ソースの snapshot diff が空であること
2. Hono ベンチの `clean` ファイルセットが現状維持
3. 全単体テストの assertion が変更されない（pattern 構造検証を追加するテストを除く）

## Task List

### T1: `Pattern` enum 定義 + `IrVisitor` / `IrFolder` trait 骨格

- **Work**:
  - `src/ir/pattern.rs` 新規作成。`Pattern` enum を上記 Design の通り定義
  - `src/ir/visit.rs` 新規作成。`IrVisitor` trait + 全 `walk_*` 関数（Item/Stmt/Expr/RustType/Pattern/MatchArm/TypeParam/Method）を実装
  - `src/ir/fold.rs` 新規作成。`IrFolder` trait + 全 `walk_*` fold 版関数を実装
  - `src/ir/mod.rs` から `pattern` / `visit` / `fold` モジュールを pub 再エクスポート
  - まだ既存 `MatchPattern` / `pattern: String` は削除しない（次タスクまで共存）
- **完了基準**: `cargo check` pass。`IrVisitor` / `IrFolder` の単体テスト（カウンタ visitor で全 variant が訪問されることを確認）が pass
- **Depends on**: なし
- **Prerequisites**: なし

### T2: IR の pattern field 型変更

- **Work**:
  - `src/ir/mod.rs`:
    - `MatchPattern` enum を削除
    - `MatchArm::patterns: Vec<MatchPattern>` → `Vec<Pattern>`
    - `Stmt::IfLet::pattern: String` → `Pattern`
    - `Stmt::WhileLet::pattern: String` → `Pattern`
    - `Expr::IfLet::pattern: String` → `Pattern`
    - `Expr::Matches::pattern: String` → `Pattern`
  - この段階で compile が全面的に壊れる。次タスクで修復
- **完了基準**: `cargo check` は失敗するが、IR 定義の型変更が一貫している（pattern 関連が全て `Pattern` 型）
- **Depends on**: T1
- **Prerequisites**: T1 で `Pattern` enum が定義済み

### T3: Transformer の pattern 構築サイト書き換え

- **Work**:
  - `src/transformer/statements/control_flow.rs:279-332` の `MatchPattern::Verbatim(format!(...))` を `Pattern::TupleStruct` / `Pattern::UnitStruct` 構築に置き換え:
    - `"None"` → `Pattern::UnitStruct { path: vec!["None".into()] }`
    - `format!("Some({})", var_name)` → `Pattern::TupleStruct { path: vec!["Some".into()], fields: vec![Pattern::Binding { name: var_name, is_mut: false, subpat: None }] }`
    - `positive_pattern` / `complement_pattern` の既存文字列生成コードを構造化 Pattern 生成に書き換え
  - `src/transformer/expressions/literals.rs:63` の `pattern: full_pattern` を Pattern に
  - `src/transformer/expressions/patterns.rs:128,287` の `Expr::Matches { pattern: ... }` 構築サイト
  - `src/transformer/expressions/mod.rs:223,255,262` の `Expr::IfLet { pattern: ... }` 構築サイト
  - `src/transformer/statements/error_handling.rs:119` の `Stmt::IfLet { pattern: ... }` 構築サイト
  - `src/transformer/statements/loops.rs:26` の `Stmt::WhileLet { pattern: ... }` 構築サイト
  - `src/transformer/statements/loops.rs:160,361,368,391` の `Stmt::IfLet` 構築サイト
  - `src/transformer/functions/helpers.rs:176-217` の pattern clone サイト（型変更のみ）
  - `src/transformer/statements/mutability.rs` の pattern 参照サイト（pattern match のフィールドアクセスを構造化に合わせる）
- **完了基準**: `cargo check` の transformer 関連エラーが 0
- **Depends on**: T2
- **Prerequisites**: なし

### T4: Generator の Pattern rendering 実装

- **Work**:
  - `src/generator/statements/mod.rs:200` の `MatchPattern::Verbatim(s) => s.clone()` を削除し、`Pattern` を Rust 文字列にレンダリングする `render_pattern` 関数を実装
  - `src/generator/expressions/mod.rs:378` 同様
  - `Pattern::TupleStruct { path, fields }` → `Color::Red(x, y)` 形式
  - `Pattern::Struct { path, fields, rest }` → `Foo { a, b, .. }` 形式
  - `Pattern::UnitStruct { path }` → `None` / `Color::Green`
  - `Pattern::Binding { name, is_mut, subpat }` → `mut x @ ...`
  - `Pattern::Wildcard` → `_`
  - `Pattern::Literal(expr)` → expr の generator 出力
  - `Pattern::Or(pats)` → `a | b | c`
  - `Pattern::Range { start, end, inclusive }` → `1..=5`
  - `Pattern::Ref { mutable, inner }` → `&x` / `&mut x`
  - `Pattern::Tuple(pats)` → `(a, b, c)`
- **完了基準**: `cargo check` の generator 関連エラーが 0。`cargo build` 成功
- **Depends on**: T3
- **Prerequisites**: T3 で IR ↔ 構築サイトが整合

### T5: `collect_type_refs_*` の `TypeRefCollector` 化

- **Work**:
  - `src/pipeline/external_struct_generator/type_ref_collector.rs` 新規作成
  - `TypeRefCollector: IrVisitor` を Design に従い実装
  - `PATTERN_LANG_BUILTINS = &["Some", "None", "Ok", "Err"]` を `visit_pattern` 内に配置
  - `visit_rust_type` で `Named::name != "Self"` ガード付き registration
  - `visit_expr` で `FnCall::CallTarget::Path::type_ref` と `StructInit::name` を処理
  - `src/pipeline/external_struct_generator/mod.rs`:
    - `collect_type_refs_from_item / _stmt / _expr / _rust_type / _type_params / _method / _match_arm / _verbatim_pattern` を削除
    - 呼び出し側（`src/pipeline/placement.rs`, `src/transformer/mod.rs` 等）を `TypeRefCollector::new(&mut refs).visit_*(node)` に書き換え
    - `RUST_BUILTIN_TYPES` から `"Some", "None", "Ok", "Err"` を削除
    - 定数上部コメントを「I-377 で構造化 Pattern に移行済み。builtin variant 除外は `TypeRefCollector::visit_pattern` の `PATTERN_LANG_BUILTINS` で処理」に更新
- **完了基準**: `cargo test` が pass（少なくとも従来と同じ件数）。`grep "collect_type_refs_from_" src/` が定義・呼び出しとも 0 件。`grep '"Some", "None", "Ok", "Err"' src/pipeline/external_struct_generator/mod.rs` が 0 件
- **Depends on**: T4
- **Prerequisites**: T4 で変換出力が復元済み

### T6: `substitute.rs` の `IrFolder` 化

- **Work**:
  - `src/ir/substitute.rs` の各 `impl X { fn substitute(&self, ...) -> X }` を `Substitute: IrFolder` 実装に書き換え
  - `Substitute::new(&bindings).fold_*(node)` 呼び出しに全サイト置き換え
  - 既存の `substitute` メソッドは薄いラッパー（`self.clone().fold_by(Substitute::new(bindings))`）として残すか、呼び出し側を全部書き換えて削除
- **完了基準**: `cargo test` pass。`substitute.rs` の手書き再帰コードが削除されている（`walk_*` への委譲のみ）
- **Depends on**: T5
- **Prerequisites**: T5 で `IrVisitor` が実動作確認済み

### T7: 散発再帰の IrVisitor 化

- **Work**:
  - `src/transformer/mod.rs::expr_contains_runtime_typeof` + `stmts_contain_runtime_typeof` + `items_contain_runtime_typeof` を `RuntimeTypeofDetector: IrVisitor { found: bool }` に統合
  - `items_contain_regex` + `stmts_contain_regex` + regex 検出再帰を `RegexDetector: IrVisitor` に統合
  - 呼び出し側を detector ベースに置き換え
- **完了基準**: `cargo test` pass。`expr_contains_runtime_typeof` / `stmts_contain_runtime_typeof` / `items_contain_regex` / `stmts_contain_regex` 関数が削除されている
- **Depends on**: T6
- **Prerequisites**: なし

### T8: テスト追加と検証

- **Work**:
  - `IrVisitor` / `IrFolder` trait の単体テスト（全 variant を訪問するカウンタ visitor）
  - `TypeRefCollector` の単体テスト:
    - `Pattern::TupleStruct { path: vec!["myClass".into()], .. }` → refs に `"myClass"` 登録（lowercase class 回帰防止）
    - `Pattern::TupleStruct { path: vec!["Some".into()], .. }` → refs に `"Some"` **未**登録
    - `Pattern::UnitStruct { path: vec!["None".into()] }` → refs に `"None"` **未**登録
    - `Pattern::Struct { path: vec!["Color".into(), "Red".into()], .. }` → refs に `"Color"` 登録
  - `Substitute` の既存 substitute テストが全 pass
  - 回帰テスト: `tests/lowercase_class_reference_test.rs` が pass
  - 回帰テスト: `tests/integration_test.rs` の `test_type_narrowing` / `test_async_await` / `test_error_handling` / `test_narrowing_truthy_instanceof` が pass（I-375 申し送りの 4 件）
  - snapshot テストが全 pass（generator 出力の文字列が現状と一致）
  - `./scripts/hono-bench.sh` 実行: clean / dir compile / error instances すべて現状維持
- **完了基準**: 上記全テスト pass。`grep 'pattern: String\|MatchPattern::Verbatim\|collect_type_refs_from_verbatim\|uppercase' src/` の該当検索が 0 件（一時的コメントも含めて削除確認）
- **Depends on**: T7
- **Prerequisites**: T1–T7 完了

### T9: Quality Check

- **Work**: `cargo fix` → `cargo fmt` → `cargo clippy --all-targets --all-features -- -D warnings` → `cargo test` → `./scripts/check-file-lines.sh`
- **完了基準**: 全て 0 error 0 warning。`external_struct_generator/mod.rs` が 1000 行超過していないこと（現状 777 行 + TypeRefCollector 分離で縮小見込み）
- **Depends on**: T8
- **Prerequisites**: なし

## Test Plan

### 新規テスト

1. **IrVisitor 骨格テスト** (`src/ir/visit.rs` 内 `#[cfg(test)]`):
   - 全 Item / Stmt / Expr / RustType / Pattern variant を含むサンプル IR を構築し、訪問カウンタで全 variant が 1 度ずつ訪問されることを検証
2. **IrFolder 骨格テスト** (`src/ir/fold.rs` 内):
   - 恒等 folder が IR を変更しないこと
   - `RustType::Named { name: "T" }` を `RustType::Named { name: "i32" }` に置換する folder が全変換箇所で動作すること
3. **TypeRefCollector 単体テスト** (`src/pipeline/external_struct_generator/type_ref_collector.rs` 内):
   - lowercase class 名 (`myClass`) の Pattern から refs 登録
   - `Some` / `None` / `Ok` / `Err` の Pattern は refs 未登録
   - `Color::Red` の Pattern から `Color` を登録
   - Pattern 内 nested pattern（`Some(Color::Red)`）から両方の型名を登録
4. **Substitute 回帰**: 既存 substitute テストが全 pass

### 既存テスト修正

- transformer 側 pattern 構築テスト (`tests/control_flow.rs` 等) の assertion を構造化 Pattern に更新
- `matches!(s, Stmt::IfLet { pattern, .. } if pattern == "Some(x)")` → `matches!(s, Stmt::IfLet { pattern: Pattern::TupleStruct { path, .. }, .. } if path == &vec!["Some"])`

### 回帰テスト（必須 pass）

- `tests/lowercase_class_reference_test.rs`
- `tests/integration_test.rs::test_type_narrowing`
- `tests/integration_test.rs::test_async_await`
- `tests/integration_test.rs::test_error_handling`
- `tests/integration_test.rs::test_narrowing_truthy_instanceof`
- 全 snapshot テスト

### ベンチマーク検証

- `./scripts/hono-bench.sh`: clean 114/158、dir compile 157/158、error instances 54 の現状維持を確認

## Completion Criteria

1. ✅ `cargo test` 全 pass（テスト数は既存 2156 件 + 本 PRD で追加する IrVisitor/Folder/TypeRefCollector 単体テスト）
2. ✅ `cargo clippy --all-targets --all-features -- -D warnings` 通過
3. ✅ `cargo fmt --all --check` 通過
4. ✅ `./scripts/check-file-lines.sh` 通過（1000 行超過ファイルなし）
5. ✅ `grep -rn 'pattern: String' src/ir/` が 0 件
6. ✅ `grep -rn 'MatchPattern::Verbatim\|MatchPattern' src/ir/ src/transformer/ src/generator/` が 0 件（`Pattern` enum への統合完了）
7. ✅ `grep -n 'collect_type_refs_from_verbatim_pattern' src/` が 0 件
8. ✅ `grep -n '"Some", "None", "Ok", "Err"' src/pipeline/external_struct_generator/mod.rs` が 0 件
9. ✅ `grep -n 'fn collect_type_refs_from_' src/` が 0 件（`TypeRefCollector` 統合完了）
10. ✅ `grep -n 'fn expr_contains_runtime_typeof\|fn stmts_contain_runtime_typeof\|fn items_contain_regex\|fn stmts_contain_regex' src/` が 0 件
11. ✅ Hono ベンチ: clean 114/158 以上、dir compile 157/158 以上、error instances 54 以下
12. ✅ `tests/lowercase_class_reference_test.rs` pass
13. ✅ I-375 申し送りの 4 件の integration test pass
14. ✅ plan.md の Batch 11c-fix-2-b セクションが「完了」に更新されている
15. ✅ TODO から I-377 が削除されている
16. ✅ 本 PRD ファイルが `backlog/` から削除されている（`/backlog-management` フローに従い）

### 影響範囲の実コード確認

本 PRD は純粋な構造リファクタリングのため、「error count reduction を 3 instances 以上 trace」要件は適用されない。代わりに以下の変更前/後一致検証を課す:

1. **Snapshot 全件一致**: `cargo insta test` で全 snapshot が変更なしで pass
2. **Hono ベンチ output diff**: リファクタリング前後で `hono-output/` の diff が空であること（生成 Rust ソースが一致）
3. **walker 捕捉セット一致**: `TypeRefCollector` 統合前後で、代表的な 3 ファイル（Hono の複雑な generic を含むファイル 3 つを選定）について `refs: HashSet<String>` が完全一致することを確認
