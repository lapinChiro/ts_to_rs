# Batch 11c-fix-2-b (I-377): IrVisitor/IrFolder 導入 + Pattern 構造化

スコープ: PRD `backlog/I-377-ir-visitor-and-pattern-structuring.md` の実装。IR pattern 文字列の完全構造化、IrVisitor/IrFolder trait 導入、walker / substitute / 散発再帰の visitor 統合、`RUST_BUILTIN_TYPES` からの `Some/None/Ok/Err` 除去。

判断基準: 原理的な理想状態。文字列ベース pattern の完全撲滅、手書き IR 再帰の完全統合。コスト度外視。

進め方: `/large-scale-refactor` の Step 1〜5。Phase 単位で `cargo check` が通る状態を維持し、各 phase 完了時に `[WIP]` commit を提案する。

---

## Step 1: Analysis

### 1.1 IR pattern field 一覧（置換対象）

| # | 対象 | File:line | 現状型 | 新型 |
|---|------|-----------|--------|------|
| A1 | `MatchPattern` enum | `src/ir/mod.rs:292-309` | 独立 enum (`Literal`/`Wildcard`/`EnumVariant{path,bindings}`/`Verbatim`) | 削除。`Pattern` に統合 |
| A2 | `MatchArm::patterns` | `src/ir/mod.rs:315` | `Vec<MatchPattern>` | `Vec<Pattern>` |
| A3 | `Stmt::WhileLet::pattern` | `src/ir/mod.rs:525` | `String` | `Pattern` |
| A4 | `Stmt::IfLet::pattern` | `src/ir/mod.rs:570` | `String` | `Pattern` |
| A5 | `Expr::IfLet::pattern` | `src/ir/mod.rs:846` | `String` | `Pattern` |
| A6 | `Expr::Matches::pattern` | `src/ir/mod.rs:906` | `String` | `Pattern` |

→ `src/ir/mod.rs:924` の `pattern: String` は `Expr::Regex::pattern`（正規表現文字列）であり match pattern ではない。変更対象外。

### 1.2 Pattern 構築サイト（transformer）

| # | File:line | 現行コード | 新形 |
|---|-----------|-----------|------|
| B1 | `src/transformer/statements/control_flow.rs:279` | `MatchPattern::Verbatim(positive_pattern.clone())` | `Pattern`（`resolve_if_let_pattern` が返す構造化値） |
| B2 | `src/transformer/statements/control_flow.rs:284` | `MatchPattern::Verbatim(complement_pattern.clone())` | `Pattern`（`resolve_complement_pattern` から） |
| B3 | `src/transformer/statements/control_flow.rs:304` | `MatchPattern::Verbatim("None".to_string())` | `Pattern::UnitStruct { path: vec!["None".into()] }` |
| B4 | `src/transformer/statements/control_flow.rs:309` | `MatchPattern::Verbatim(format!("Some({})", var_name))` | `Pattern::TupleStruct { path: vec!["Some".into()], fields: vec![Pattern::Binding{name: var_name,..}] }` |
| B5 | `src/transformer/statements/control_flow.rs:327,332` | 同 B1/B2 | 同 |
| B6 | `src/transformer/statements/control_flow.rs:160-164` | `Stmt::IfLet { pattern: format!("Some({})", ca.var_name), ... }` | `Pattern::TupleStruct{Some, Binding}` |
| B7 | `src/transformer/statements/loops.rs:26-28` | `Stmt::WhileLet { pattern: format!("Some({})", ca.var_name), ... }` | 同 |
| B8 | `src/transformer/statements/error_handling.rs:119-120` | `Stmt::IfLet { pattern: format!("Err({catch_param})"), ... }` | `Pattern::TupleStruct{Err, Binding}` |
| B9 | `src/transformer/statements/control_flow.rs:358-374` (`generate_if_let`) | `(pattern: String, is_swap)` タプル使用 | `(Pattern, bool)` |
| B10 | `src/transformer/expressions/mod.rs:214-267` | `resolve_if_let_pattern` の戻り値を `Expr::IfLet { pattern: ..., ... }` に渡す | Pattern 経由 |
| B11 | `src/transformer/expressions/patterns.rs:127-131` | `Expr::Matches { pattern: format!("{enum_name}::{expected_variant}(_)"), ... }` | `Pattern::TupleStruct{vec![enum_name, expected_variant], vec![Wildcard]}` |
| B12 | `src/transformer/expressions/patterns.rs:287-290` | `Expr::Matches { pattern: format!("{name}::{class_name}(_)"), ... }` | 同 |
| B13 | `src/transformer/expressions/patterns.rs:402-494` `resolve_if_let_pattern` / `resolve_complement_pattern` / `resolve_other_variant` | 戻り値 `Option<(String, bool)>` / `Option<String>` | `Option<(Pattern, bool)>` / `Option<Pattern>` |
| B14 | `src/transformer/statements/switch.rs:273,285-296` | `MatchPattern::Literal(Expr::Ident("{ename}::{vname}({var_name})"))` — **既存 broken window**: Pattern を Expr::Ident に encode している | `Pattern::TupleStruct{vec![ename,vname], vec![Binding{var_name}]}` |
| B15 | `src/transformer/statements/switch.rs:437-440` | `MatchPattern::EnumVariant { path: format!("{enum_name}::{variant_name}"), bindings: vec![] }` | `Pattern::Struct{path: vec![enum_name, variant_name], fields: [], rest: true}` または `Pattern::UnitStruct` |
| B16 | `src/transformer/statements/switch.rs:461-474` | `if let MatchPattern::EnumVariant { bindings, path, .. } = pattern` — path から variant name を `rsplit("::")` で抽出 | `Pattern::Struct` の `path: Vec<String>` を直接使用 |
| B17 | `src/transformer/statements/switch.rs:553` | `MatchPattern::Literal(Expr::Ident(path))` path = `"EnumName::Variant"` — **既存 broken window** | `Pattern::UnitStruct{vec![enum_name, variant_name]}` |
| B18 | `src/transformer/statements/switch.rs:612,632,642,349,490,566` | `MatchPattern::Literal(expr)` / `MatchPattern::Wildcard` | そのまま `Pattern::Literal(expr)` / `Pattern::Wildcard` |
| B19 | `src/transformer/expressions/member_access.rs:387-403` | `MatchPattern::EnumVariant { path, bindings }` | `Pattern::Struct` |
| B20 | `src/transformer/context.rs:277-282` (test) | `MatchPattern::Literal(...)` / `MatchPattern::Wildcard` | `Pattern::Literal(...)` / `Pattern::Wildcard` |

### 1.3 Pattern 参照サイト（mutability.rs 等）

| # | File:line | 内容 |
|---|-----------|------|
| C1 | `src/transformer/statements/mutability.rs:97-131` | `Stmt::IfLet { ... }` / `Stmt::WhileLet { ... }` を pattern match（フィールド参照のみ） |
| C2 | `src/transformer/statements/mutability.rs:242-278` | `Expr::IfLet`, `Expr::Matches` を pattern match |
| C3 | `src/transformer/functions/helpers.rs:176-217` | `Stmt::WhileLet` / `Stmt::IfLet` の clone (`pattern: pattern.clone()`) |
| C4 | `src/transformer/statements/loops.rs:585-709` | `Stmt::IfLet` / `Stmt::WhileLet` の body walking |

→ これらは struct field 型が変わるだけで、field 名参照は変更不要。`pattern.clone()` は Pattern 型でも同様に動作。

### 1.4 Generator 側 rendering

| # | File:line | 現状 | 新 |
|---|-----------|------|----|
| D1 | `src/generator/statements/mod.rs:75-200` | `Stmt::WhileLet { pattern, .. }` `Stmt::IfLet { pattern, .. }` で `pattern` を `String` として扱い直接 emit。`MatchPattern::Verbatim(s) => s.clone()` | `render_pattern(&Pattern) -> String` を呼び出し |
| D2 | `src/generator/expressions/mod.rs:299-378` | `Expr::IfLet` / `Expr::Matches` / match arms 同様 | 同 |
| D3 | `src/generator/statements/mod.rs:177-200` | `MatchPattern::{Literal, Wildcard, EnumVariant{path,bindings}, Verbatim}` 各 arm 処理 | `Pattern` の全 variant 処理へ統合 |
| D4 | `src/generator/expressions/mod.rs:360-378` | 同 D3 | 同 |

### 1.5 Walker / Substitute / 散発再帰

| # | File:line | 現状 | 新 |
|---|-----------|------|----|
| E1 | `src/pipeline/external_struct_generator/mod.rs:305-712` `collect_type_refs_from_item/_stmt/_expr/_rust_type/_type_params/_method/_match_arm/_verbatim_pattern` 約 400 行 | 手書き再帰 + uppercase-head ヒューリスティック | `TypeRefCollector: IrVisitor` に統合 |
| E2 | `src/ir/substitute.rs` 全体 (595 行) `impl X { fn substitute }` | 手書き再帰で新 IR 生成 | `Substitute: IrFolder` に統合 |
| E3 | `src/transformer/mod.rs:756 expr_contains_runtime_typeof` + `stmts_contain_runtime_typeof` + `items_contain_runtime_typeof` | 手書き再帰 | `RuntimeTypeofDetector: IrVisitor` |
| E4 | `src/transformer/mod.rs:796 items_contain_regex` + `stmts_contain_regex` + body walking | 手書き再帰 | `RegexDetector: IrVisitor` |

### 1.6 uppercase-head ヒューリスティック / ハードコード除外

| # | File:line | 削除対象 |
|---|-----------|---------|
| F1 | `src/pipeline/external_struct_generator/mod.rs:31-36` | `RUST_BUILTIN_TYPES` から `"Some", "None", "Ok", "Err"` を削除 |
| F2 | `src/pipeline/external_struct_generator/mod.rs:15-36` | 定数上部の暫定コメント（I-377 まで必要な一時フィルタ）を正規コメントに置換 |
| F3 | `src/pipeline/external_struct_generator/mod.rs:511-537 collect_type_refs_from_verbatim_pattern` | 関数ごと削除 |
| F4 | `src/pipeline/external_struct_generator/mod.rs:539-562 collect_type_refs_from_match_arm` の uppercase-head ロジック | `TypeRefCollector` 再実装で消滅 |

### 1.7 テストファイルへの影響

| # | File | 影響 |
|---|------|------|
| T1 | `src/pipeline/external_struct_generator/tests.rs:1910-2084` | `MatchPattern` / `Stmt::IfLet{pattern:"..."}` / `Stmt::WhileLet{pattern:"..."}` / `Expr::Matches{pattern:"..."}` 構築 |
| T2 | `src/transformer/statements/tests/switch.rs` 多数 | `MatchPattern::` での pattern matching |
| T3 | `src/transformer/statements/tests/control_flow.rs:452,532` | `pattern == "Some(x)"` 文字列比較 |
| T4 | `src/transformer/statements/tests/error_handling.rs:125` | `Stmt::IfLet { pattern, .. }` |
| T5 | `src/transformer/expressions/tests/type_guards.rs:352-380` | `resolve_if_let_pattern` の結果型変更 |
| T6 | `src/transformer/expressions/tests/enums.rs:331` | `MatchPattern::EnumVariant` |
| T7 | `src/generator/statements/tests.rs:165,464,494` | `Stmt::IfLet/WhileLet { pattern: "..." }` |
| T8 | `src/generator/expressions/tests.rs:706-718` | `MatchPattern::EnumVariant` / `Wildcard` |
| T9 | `src/transformer/context.rs:277-282` (inline test) | `MatchPattern::Literal` / `Wildcard` |
| T10 | `src/transformer/statements/tests/mod.rs:13` / `expressions/tests/mod.rs:23` | `use crate::ir::MatchPattern` |
| T11 | `tests/lowercase_class_reference_test.rs` | 回帰確認 |
| T12 | `tests/integration_test.rs::test_type_narrowing/test_async_await/test_error_handling/test_narrowing_truthy_instanceof` | I-375 申し送り 4 件の回帰確認（Some/None/Ok/Err 除去後も pass） |

### 1.8 変更ファイル総数

新規作成 4 + 変更 ~30 + テスト ~10 = **合計 ~45 ファイル**。

- 新規: `src/ir/pattern.rs`, `src/ir/visit.rs`, `src/ir/fold.rs`, `src/pipeline/external_struct_generator/type_ref_collector.rs`
- 変更（非テスト）: `src/ir/mod.rs`, `src/ir/substitute.rs`, `src/transformer/statements/{control_flow.rs, loops.rs, error_handling.rs, switch.rs, mutability.rs}`, `src/transformer/expressions/{mod.rs, patterns.rs, member_access.rs, literals.rs (確認のみ)}`, `src/transformer/functions/helpers.rs`, `src/transformer/context.rs`, `src/transformer/mod.rs`, `src/generator/statements/mod.rs`, `src/generator/expressions/mod.rs`, `src/pipeline/external_struct_generator/mod.rs`, `src/pipeline/placement.rs`（使用箇所があれば）, 他の呼び出し側

---

## Step 2: Design

### 2.1 `Pattern` enum 定義

`src/ir/pattern.rs` 新規:

```rust
use super::Expr;

/// Rust pattern grammar を構造化表現した IR ノード。
///
/// `MatchArm::patterns` および `Stmt::IfLet` / `Stmt::WhileLet` /
/// `Expr::IfLet` / `Expr::Matches` の `pattern` フィールドで使用される。
/// 文字列化は generator の `render_pattern` が担当し、IR 層は
/// 常に構造化データを保持する。
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_`
    Wildcard,
    /// リテラル値（`1`, `"hello"`, `true`, `Direction::Up` 相当の path expression）。
    /// match arm で discriminant と比較される値そのもの。
    Literal(Expr),
    /// 変数束縛（`x`, `mut x`, `x @ 1..=5`）
    Binding {
        name: String,
        is_mut: bool,
        subpat: Option<Box<Pattern>>,
    },
    /// タプル構造体 variant（`Some(x)`, `Color::Red(x, y)`, `Ok(v)`, `Err(e)`）
    ///
    /// `path` は `::` 結合前のセグメント列。`fields` はタプル要素のサブパターン。
    TupleStruct {
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
    /// Unit variant / unit struct（`None`, `Color::Green`）
    UnitStruct {
        path: Vec<String>,
    },
    /// Or パターン（`a | b | c`）
    Or(Vec<Pattern>),
    /// Range パターン（`1..=5`, `..10`）
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    /// 参照パターン（`&x`, `&mut x`）
    Ref {
        mutable: bool,
        inner: Box<Pattern>,
    },
    /// タプルパターン（`(a, b, c)`）
    Tuple(Vec<Pattern>),
}
```

**設計判断**:
- `path: Vec<String>` によりセグメント単位でアクセス可能。walker は `path[0]` を直接見る
- `UnitStruct` と `TupleStruct { fields: vec![] }` を分離: 前者 → `None`、後者 → `None()` とレンダリング差が明確
- `Binding::subpat` は `x @ 1..=5` パターン用。現状 transformer では生成しないが enum variant は用意（Q2 B 範囲）
- `Literal` の `Expr` には `Expr::StringLit` / `Expr::IntLit` / `Expr::Ident("Direction::Up")` 等が入る。`Ident` に path string を入れるのは既存 switch.rs のパターンだが、新コードでは `UnitStruct` に置き換える（B17）

### 2.2 `IrVisitor` trait

`src/ir/visit.rs` 新規:

```rust
use super::*;

/// Read-only IR visitor trait. Each `visit_*` method default-delegates to
/// the corresponding `walk_*` function, which recursively visits children.
/// Implementors override `visit_*` methods they need and optionally call
/// `walk_*` to continue recursion.
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

pub fn walk_item<V: IrVisitor + ?Sized>(v: &mut V, item: &Item) { /* 全 variant 再帰 */ }
// ... walk_stmt, walk_expr, walk_rust_type, walk_pattern, walk_match_arm, walk_type_param, walk_method
```

**設計判断**:
- `?Sized` bound により trait object での使用も可能
- default delegation により実装者は必要な visit_* のみ override
- `walk_pattern` 内で `Pattern::Literal(expr)` は `v.visit_expr(expr)` を呼ぶ（expr 側の Ident/StructInit 経由で型参照を収集可能にする）

### 2.3 `IrFolder` trait

`src/ir/fold.rs` 新規:

```rust
pub trait IrFolder {
    fn fold_item(&mut self, item: Item) -> Item { walk_item(self, item) }
    fn fold_stmt(&mut self, stmt: Stmt) -> Stmt { walk_stmt(self, stmt) }
    fn fold_expr(&mut self, expr: Expr) -> Expr { walk_expr(self, expr) }
    fn fold_rust_type(&mut self, ty: RustType) -> RustType { walk_rust_type(self, ty) }
    fn fold_pattern(&mut self, pat: Pattern) -> Pattern { walk_pattern(self, pat) }
    fn fold_match_arm(&mut self, arm: MatchArm) -> MatchArm { walk_match_arm(self, arm) }
    fn fold_type_param(&mut self, tp: TypeParam) -> TypeParam { walk_type_param(self, tp) }
    fn fold_method(&mut self, m: Method) -> Method { walk_method(self, m) }
}
```

所有権ベースの変換 visitor（SWC の `Fold` と同型）。各 `walk_*` 関数は `by move` で新 IR を構築。

### 2.4 `TypeRefCollector` 設計

```rust
pub(crate) struct TypeRefCollector<'a> {
    pub refs: &'a mut HashSet<String>,
}

/// Option / Result のコンストラクタは言語レベル組み込みで type_ref 登録対象外。
/// uppercase-head ヒューリスティックではなく構造化 path[0] 比較で除外する。
const PATTERN_LANG_BUILTINS: &[&str] = &["Some", "None", "Ok", "Err"];

impl<'a> IrVisitor for TypeRefCollector<'a> {
    fn visit_rust_type(&mut self, ty: &RustType) {
        if let RustType::Named { name, .. } = ty {
            if name != "Self" { self.refs.insert(name.clone()); }
        }
        if let RustType::QSelf { trait_ref, .. } = ty {
            self.refs.insert(trait_ref.name.clone());
        }
        walk_rust_type(self, ty);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::StructInit { name, .. } if name != "Self" => {
                self.refs.insert(name.clone());
            }
            Expr::FnCall { target: CallTarget::Path { type_ref: Some(t), .. }, .. } => {
                self.refs.insert(t.clone());
            }
            _ => {}
        }
        walk_expr(self, expr);
    }

    fn visit_pattern(&mut self, pat: &Pattern) {
        let head = match pat {
            Pattern::TupleStruct { path, .. }
            | Pattern::Struct { path, .. }
            | Pattern::UnitStruct { path } => path.first(),
            _ => None,
        };
        if let Some(first) = head {
            if !PATTERN_LANG_BUILTINS.contains(&first.as_str()) {
                self.refs.insert(first.clone());
            }
        }
        walk_pattern(self, pat);
    }
}
```

`RUST_BUILTIN_TYPES` から `Some/None/Ok/Err` を削除。Type 名 filter と pattern constructor filter は抽象レベルが異なるため分離維持（DRY 分析済）。

### 2.5 `Substitute` 設計

```rust
pub struct Substitute<'a> {
    pub bindings: &'a HashMap<String, RustType>,
}

impl<'a> IrFolder for Substitute<'a> {
    fn fold_rust_type(&mut self, ty: RustType) -> RustType {
        if let RustType::Named { ref name, ref type_args } = ty {
            if type_args.is_empty() {
                if let Some(concrete) = self.bindings.get(name.as_str()) {
                    return concrete.clone();
                }
            }
        }
        walk_rust_type(self, ty) // 子を再帰 fold
    }
}
```

その他 `fold_item/_stmt/_expr/_pattern` は全て default（`walk_*` 委譲）。既存の `*.substitute(&bindings)` 呼び出しは薄いラッパーで互換維持するか、全サイトを書き換える。

**判断**: **全サイト書き換え**（ラッパー残存は新旧併存で DRY 違反）。既存の `X.substitute(&bindings)` → `Substitute { bindings: &bindings }.fold_x(X)`。

### 2.6 `RuntimeTypeofDetector` / `RegexDetector`

```rust
#[derive(Default)]
struct RuntimeTypeofDetector { found: bool }

impl IrVisitor for RuntimeTypeofDetector {
    fn visit_expr(&mut self, expr: &Expr) {
        if matches!(expr, Expr::RuntimeTypeof { .. }) {
            self.found = true;
            return; // 早期リターンで残りの走査回避
        }
        walk_expr(self, expr);
    }
}

fn items_contain_runtime_typeof(items: &[Item]) -> bool {
    let mut d = RuntimeTypeofDetector::default();
    for item in items {
        d.visit_item(item);
        if d.found { return true; }
    }
    false
}
```

`RegexDetector` は `Expr::Regex { .. }` / `MethodCall::method == "regex" new` 検出ロジックを移植（現状コードを Read して確認のうえ実装）。

### 2.7 Generator pattern rendering 設計

`src/generator/mod.rs` または新規 `src/generator/patterns.rs`:

```rust
pub(crate) fn render_pattern(pat: &Pattern) -> String {
    match pat {
        Pattern::Wildcard => "_".to_string(),
        Pattern::Literal(e) => generate_expr(e),
        Pattern::Binding { name, is_mut, subpat } => {
            let prefix = if *is_mut { "mut " } else { "" };
            match subpat {
                Some(sub) => format!("{prefix}{name} @ {}", render_pattern(sub)),
                None => format!("{prefix}{name}"),
            }
        }
        Pattern::TupleStruct { path, fields } => {
            let path_str = path.join("::");
            let field_strs: Vec<String> = fields.iter().map(render_pattern).collect();
            format!("{path_str}({})", field_strs.join(", "))
        }
        Pattern::Struct { path, fields, rest } => {
            let path_str = path.join("::");
            let mut parts: Vec<String> = fields
                .iter()
                .map(|(n, p)| match p {
                    Pattern::Binding { name, is_mut: false, subpat: None } if name == n => n.clone(),
                    _ => format!("{n}: {}", render_pattern(p)),
                })
                .collect();
            if *rest { parts.push("..".to_string()); }
            if parts.is_empty() {
                format!("{path_str} {{}}")
            } else {
                format!("{path_str} {{ {} }}", parts.join(", "))
            }
        }
        Pattern::UnitStruct { path } => path.join("::"),
        Pattern::Or(pats) => pats.iter().map(render_pattern).collect::<Vec<_>>().join(" | "),
        Pattern::Range { start, end, inclusive } => {
            let s = start.as_deref().map(generate_expr).unwrap_or_default();
            let e = end.as_deref().map(generate_expr).unwrap_or_default();
            let op = if *inclusive { "..=" } else { ".." };
            format!("{s}{op}{e}")
        }
        Pattern::Ref { mutable, inner } => {
            let prefix = if *mutable { "&mut " } else { "&" };
            format!("{prefix}{}", render_pattern(inner))
        }
        Pattern::Tuple(pats) => {
            let strs: Vec<String> = pats.iter().map(render_pattern).collect();
            format!("({})", strs.join(", "))
        }
    }
}
```

配置先: `src/generator/patterns.rs` 新規（凝集度重視）。

### 2.8 `resolve_if_let_pattern` / `resolve_complement_pattern` 戻り値変更

```rust
// Before:
pub(crate) fn resolve_if_let_pattern(&self, guard: &NarrowingGuard) -> Option<(String, bool)>
pub(crate) fn resolve_complement_pattern(&self, guard: &NarrowingGuard) -> Option<String>
fn resolve_other_variant(...) -> Option<String>

// After:
pub(crate) fn resolve_if_let_pattern(&self, guard: &NarrowingGuard) -> Option<(Pattern, bool)>
pub(crate) fn resolve_complement_pattern(&self, guard: &NarrowingGuard) -> Option<Pattern>
fn resolve_other_variant(...) -> Option<Pattern>
```

変換ロジック内の `format!("Some({})", guard.var_name())` → `Pattern::TupleStruct { path: vec!["Some".into()], fields: vec![Pattern::Binding { name: guard.var_name().into(), is_mut: false, subpat: None }] }`。

特殊: `complement_pattern == "None"` / `complement_pattern != "None"` のような文字列比較が `control_flow.rs:276, 299, 324` にある。これらは `matches!(complement_pattern, Pattern::UnitStruct { path } if path == &["None"])` に置換。ヘルパー関数 `is_none_pattern(&Pattern) -> bool` を `src/transformer/expressions/patterns.rs` に追加。

### 2.9 Design Integrity Review

- **凝集度**: `Pattern` / `IrVisitor` / `IrFolder` / `TypeRefCollector` / `Substitute` / `render_pattern` は各々単一責務。OK
- **責務分離**: walker 走査と判定を trait の walk_* / visit_* で分離。Pattern の文字列化は generator が担う。builtin 判定は `TypeRefCollector::visit_pattern` 内部にクローズ。OK
- **DRY**: walker/substitute/detector の 4+ 系統の手書き再帰を 1 セットの `walk_*` に集約。`PATTERN_LANG_BUILTINS` と `RUST_BUILTIN_TYPES` は抽象レベル分離のため意図的重複
- **Broken windows 検出**:
  - **BW1**: `switch.rs:287` `MatchPattern::Literal(Expr::Ident("Shape::Circle(x)"))` — Pattern を Expr::Ident に encode する重大な抽象レベル破壊。本 PRD で `Pattern::TupleStruct` に修正
  - **BW2**: `switch.rs:553` `MatchPattern::Literal(Expr::Ident("Direction::Up"))` — 同じパターン、`Pattern::UnitStruct` に修正
  - **BW3**: `switch.rs:465` `path.rsplit("::").next()` — `path: String` から variant 名を抽出する文字列操作。`path: Vec<String>` 化で `.last()` に置換
  - **BW4**: `MatchPattern::EnumVariant { path: "Shape::Circle", bindings: ["radius"] }` は Struct vs Tuple の区別を持たない。`Pattern::Struct { path, fields, rest }` に構造化することで named-field vs positional が明示される
  - **BW5**: `generator/statements/mod.rs:192` と `generator/expressions/mod.rs:370` の `MatchPattern::EnumVariant` rendering が 2 箇所にコピー。`render_pattern` 統合で DRY 解消

### 2.10 Semantic Safety Analysis

本 PRD は型解決を変更しない。Pattern の構築サイトでは既存の文字列フォーマットと **等価** の Pattern を生成し、generator でまた同じ文字列にレンダリングする。

**検証**: Hono ベンチ出力 diff が空であること（T-final）。

---

## Step 3: Implementation Tasks

各タスクは phase 単位で `cargo check` が通る状態を維持する。**phase 完了時に `[WIP]` commit を提案する**。

### Phase 1: IR 基盤（Pattern / IrVisitor / IrFolder 骨格）

- [x] **P1-T1**: `src/ir/pattern.rs` 新規作成。`Pattern` enum 定義（§2.1）
- [x] **P1-T2**: `src/ir/visit.rs` 新規作成。`IrVisitor` trait + `walk_item/_stmt/_expr/_rust_type/_pattern/_match_arm/_type_param/_method` 関数（§2.2）。**この段階では `walk_pattern` は `Pattern` enum に対して全 variant を処理**。`walk_stmt`/`walk_expr` 内の `IfLet/WhileLet/Matches` は **まだ `pattern: String` を参照**（次 phase で構造化するまで）→ `visit_pattern` は呼ばない
- [x] **P1-T3**: `src/ir/fold.rs` 新規作成。`IrFolder` trait + `walk_*` 関数（§2.3）。同じく `IfLet/WhileLet/Matches` の pattern は文字列のまま pass-through
- [x] **P1-T4**: `src/ir/mod.rs` に `pub mod pattern; pub mod visit; pub mod fold;` 追加 + `pub use pattern::Pattern;` 再エクスポート
- [x] **P1-T5**: `src/ir/visit.rs` / `src/ir/fold.rs` 内に `#[cfg(test)]` でカウンタ visitor/folder の単体テスト
- [x] **P1-T6**: `cargo check`, `cargo test --lib ir::` で `Pattern` enum と trait 骨格が通ることを確認
- [x] **P1-Commit**: `[WIP] Batch 11c-fix-2-b Phase 1: Pattern enum + IrVisitor/IrFolder 骨格`（ユーザーによるコミット待ち）

### Phase 2: IR 型変更と構築サイト一括書き換え

大規模型変更。cargo check がエラーだらけになる大手術。**1 phase で一気に通す**。

- [ ] **P2-T1**: `src/ir/mod.rs` から `MatchPattern` enum を削除。`MatchArm::patterns: Vec<Pattern>` に変更
- [ ] **P2-T2**: `src/ir/mod.rs` の `Stmt::WhileLet::pattern` / `Stmt::IfLet::pattern` / `Expr::IfLet::pattern` / `Expr::Matches::pattern` を全て `Pattern` に変更
- [ ] **P2-T3**: `src/ir/visit.rs` / `src/ir/fold.rs` の `walk_stmt` / `walk_expr` 内で `IfLet/WhileLet/Matches` の pattern を `visit_pattern` / `fold_pattern` で処理するよう追記
- [ ] **P2-T4**: `src/ir/substitute.rs` を `Substitute: IrFolder` に全面書き換え（§2.5）。既存の `impl X { fn substitute }` は削除
- [ ] **P2-T5**: substitute 呼び出し側（`grep -rn '\.substitute(' src/`）を `Substitute { bindings: &b }.fold_*(node)` に書き換え
- [ ] **P2-T6**: `src/transformer/expressions/patterns.rs` の `resolve_if_let_pattern` / `resolve_complement_pattern` / `resolve_other_variant` の戻り値を `Pattern` ベースに変更（§2.8）
- [ ] **P2-T7**: `src/transformer/statements/control_flow.rs` の `try_generate_narrowing_match` / `generate_if_let`: Pattern 取得ロジックと `is_none_pattern` 利用。`MatchPattern::Verbatim(...)` を `Pattern::TupleStruct/UnitStruct` に置換
- [ ] **P2-T8**: `src/transformer/statements/loops.rs:26-28` の `Stmt::WhileLet { pattern: format!(...) }` を `Pattern::TupleStruct{Some, Binding}` に
- [ ] **P2-T9**: `src/transformer/statements/control_flow.rs:160-164` の `Stmt::IfLet { pattern: format!(...) }` を同上
- [ ] **P2-T10**: `src/transformer/statements/error_handling.rs:119-120` の `Stmt::IfLet { pattern: format!("Err({catch_param})"), .. }` を `Pattern::TupleStruct{Err, Binding}` に
- [ ] **P2-T11**: `src/transformer/expressions/patterns.rs:128,289` の `Expr::Matches { pattern: format!(...) }` を `Pattern::TupleStruct{ [enum_name, variant], [Wildcard] }` に
- [ ] **P2-T12**: `src/transformer/expressions/mod.rs:214-267` の `Expr::IfLet` 構築サイト（`resolve_if_let_pattern` 呼び出し結果の展開）を Pattern ベースに
- [ ] **P2-T13**: `src/transformer/statements/switch.rs` の全 pattern 構築サイト書き換え:
  - `MatchPattern::Literal` → `Pattern::Literal`
  - `MatchPattern::Wildcard` → `Pattern::Wildcard`
  - `MatchPattern::EnumVariant { path, bindings }` → `Pattern::Struct { path: path.split("::"), fields: bindings.map(|n| (n, Pattern::Binding{name:n, ..})), rest: true }`
  - **BW1 修正**: `switch.rs:287` `MatchPattern::Literal(Expr::Ident(format!("{ename}::{vname}({var_name})")))` → `Pattern::TupleStruct { path: vec![ename, vname], fields: vec![Pattern::Binding{name: var_name, ..}] }`
  - **BW2 修正**: `switch.rs:553` `MatchPattern::Literal(Expr::Ident(path))` → `Pattern::UnitStruct { path: vec![enum_name, variant_name] }`
  - `switch.rs:465` `path.rsplit("::").next()` → `path.last()`
- [ ] **P2-T14**: `src/transformer/expressions/member_access.rs:387-403` の `MatchPattern::EnumVariant` → `Pattern::Struct` または `TupleStruct`
- [ ] **P2-T15**: `src/transformer/context.rs:277-282` inline test の `MatchPattern` → `Pattern`
- [ ] **P2-T16**: `src/generator/statements/mod.rs` / `src/generator/expressions/mod.rs` の pattern rendering を新規 `src/generator/patterns.rs::render_pattern` に差し替え（§2.7）。`MatchPattern::*` の全 arm 処理を削除し `render_pattern(&Pattern)` 1 呼び出しに統合
- [ ] **P2-T17**: `cargo check` が全ファイルで pass することを確認。エラーがあれば順次修正
- [ ] **P2-T18**: `cargo test` — pattern 文字列比較している既存テスト（`tests/control_flow.rs:452,532`, `tests/error_handling.rs:125` 等）を Pattern 構造比較に更新
- [ ] **P2-Commit**: `[WIP] Batch 11c-fix-2-b Phase 2: IR pattern 構造化と全構築サイト書き換え`

### Phase 3: Walker 統合（TypeRefCollector）

- [ ] **P3-T1**: `src/pipeline/external_struct_generator/type_ref_collector.rs` 新規作成。`TypeRefCollector: IrVisitor` 実装（§2.4）
- [ ] **P3-T2**: `src/pipeline/external_struct_generator/mod.rs` から `collect_type_refs_from_item/_stmt/_expr/_rust_type/_type_params/_method/_match_arm/_verbatim_pattern` を **全て削除**
- [ ] **P3-T3**: `src/pipeline/external_struct_generator/mod.rs` の `RUST_BUILTIN_TYPES` から `"Some", "None", "Ok", "Err"` を削除。定数上部コメントを「構造化 Pattern 移行済み」に更新
- [ ] **P3-T4**: `collect_type_refs_from_*` の呼び出し側を `TypeRefCollector::new(&mut refs).visit_*(...)` に書き換え（grep で全箇所特定）
- [ ] **P3-T5**: `src/pipeline/external_struct_generator/tests.rs` の既存 walker テストを `TypeRefCollector` API で書き直し
- [ ] **P3-T6**: 新規テスト追加:
  - lowercase class (`myClass`) の Pattern から refs 登録されることを確認
  - `Pattern::TupleStruct { path: vec!["Some"] }` は refs 未登録
  - `Pattern::UnitStruct { path: vec!["None"] }` は refs 未登録
  - `Pattern::TupleStruct { path: vec!["Ok"/"Err"] }` も未登録
  - `Pattern::Struct { path: vec!["Color", "Red"] }` から `Color` 登録
- [ ] **P3-T7**: `cargo test` 全 pass 確認。Hono ベンチ実行（`./scripts/hono-bench.sh`）で clean 率 / dir compile / error instances の後退ゼロ確認
- [ ] **P3-T8**: `tests/integration_test.rs::{test_type_narrowing, test_async_await, test_error_handling, test_narrowing_truthy_instanceof}` が pass することを個別実行で確認（I-375 申し送り）
- [ ] **P3-Commit**: `[WIP] Batch 11c-fix-2-b Phase 3: TypeRefCollector 統合 + Some/None/Ok/Err 除去`

### Phase 4: 散発再帰の visitor 化

- [ ] **P4-T1**: `src/transformer/mod.rs::expr_contains_runtime_typeof` / `stmts_contain_runtime_typeof` / `items_contain_runtime_typeof` を `RuntimeTypeofDetector: IrVisitor` に統合
- [ ] **P4-T2**: `src/transformer/mod.rs::items_contain_regex` / `stmts_contain_regex` / `expr_contains_regex` を `RegexDetector: IrVisitor` に統合
- [ ] **P4-T3**: 呼び出し側の置換
- [ ] **P4-T4**: `cargo test` pass 確認
- [ ] **P4-Commit**: `[WIP] Batch 11c-fix-2-b Phase 4: 散発再帰の IrVisitor 化`

### Phase 5: 最終検証 + クリーンアップ

- [ ] **P5-T1**: `grep -rn 'pattern: String' src/ir/` が 0 件
- [ ] **P5-T2**: `grep -rn 'MatchPattern' src/` が 0 件
- [ ] **P5-T3**: `grep -rn 'collect_type_refs_from_verbatim_pattern\|collect_type_refs_from_match_arm' src/` が 0 件
- [ ] **P5-T4**: `grep -n '"Some", "None", "Ok", "Err"' src/pipeline/external_struct_generator/mod.rs` が 0 件
- [ ] **P5-T5**: `grep -n 'fn collect_type_refs_from_\|fn expr_contains_runtime_typeof\|fn stmts_contain_runtime_typeof\|fn items_contain_regex\|fn stmts_contain_regex' src/` が 0 件
- [ ] **P5-T6**: `cargo fix --allow-dirty --allow-staged`
- [ ] **P5-T7**: `cargo fmt --all`
- [ ] **P5-T8**: `cargo clippy --all-targets --all-features -- -D warnings` 通過
- [ ] **P5-T9**: `cargo test` 全 pass
- [ ] **P5-T10**: `./scripts/check-file-lines.sh` 通過
- [ ] **P5-T11**: `./scripts/hono-bench.sh` 実行: clean 114/158 以上、dir compile 157/158 以上、error instances 54 以下
- [ ] **P5-T12**: `plan.md` の Batch 11c-fix-2-b を完了に更新、TODO から I-377 削除
- [ ] **P5-T13**: `backlog/I-377-ir-visitor-and-pattern-structuring.md` 削除
- [ ] **P5-Commit**: `Batch 11c-fix-2-b: I-377 完了 — IrVisitor 化 + Pattern 構造化 + Some/None/Ok/Err 除去`

---

## Step 4: Review Results

### 完全性

- §1.1–1.8 の全対象が §3 タスクに落とし込まれている
  - A1–A7: P2-T1, P2-T2
  - B1–B20: P2-T6 〜 P2-T15
  - C1–C4: 型変更により自動追随（field 参照のみのため）
  - D1–D4: P2-T16
  - E1: P3-T1, P3-T2
  - E2: P2-T4, P2-T5
  - E3: P4-T1
  - E4: P4-T2
  - F1–F4: P3-T3
  - T1–T12: P2-T18, P3-T5, P3-T6, P3-T8, P5-T11

### 依存関係整合性

1. Phase 1（IR 基盤）→ Phase 2（構築サイト）: `Pattern` 型と `IrFolder` が必要
2. Phase 2 → Phase 3（TypeRefCollector）: `Pattern` が構造化済みである必要
3. Phase 3 → Phase 4（散発再帰）: `IrVisitor` 基盤が動作確認済みであることを示す
4. Phase 4 → Phase 5（最終検証）

### コンパイル可能性

- Phase 1 完了時: `Pattern` は独立定義。`MatchPattern` / `pattern: String` と併存するため `cargo check` pass
- Phase 2 完了時: 型変更は全サイト同時書き換えのため 1 phase 内で一括解消
- Phase 3 以降: 追加変更のみで維持

### エッジケース対応

- **BW1/BW2**: `switch.rs` の `Expr::Ident` に pattern を encode している既存 broken window を P2-T13 で構造化
- **BW3**: `path.rsplit("::").next()` → `path.last()` を P2-T13 で
- **`is_none_pattern` ヘルパー**: §2.8 で明示。`complement_pattern == "None"` の文字列比較（control_flow.rs:276, 299）を構造比較に置換
- **`Expr::IfLet` の重複**: ir/mod.rs:846 と 924 の 2 箇所要確認 → Phase 2 で現物確認のうえ対応
- **`collect_type_refs_from_stmts` 呼び出し側**: `external_struct_generator/mod.rs` 外にも `placement.rs` や他で呼ばれている可能性 → P3-T4 で grep 実測
- **`Substitute` の所有権**: `walk_*` が `by-move` 取るため、呼び出し側で `.clone()` が必要な場合あり → P2-T5 で個別確認

### テスト影響分離

- P2-T18: 既存テストの pattern 文字列比較を Pattern 構造比較に更新（transformer 側テスト）
- P3-T5: external_struct_generator/tests.rs のセットアップを Pattern ベースに
- P3-T6: TypeRefCollector 新規単体テスト
- P5-T11: Hono ベンチで回帰検出

### 問題ゼロ確認

上記レビューで**修正不要**。本タスク分解で Step 5 に進む。

---

## Step 5: Implementation

Phase 1 から順次実施する。各 phase 完了時に本 tasks.md の checkbox を `[x]` に更新する。
