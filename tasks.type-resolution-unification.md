# D-TR: 型解決の統一 — 詳細実施計画

## 設計分析: 根本的な問題

### 現状の問題構造

TypeResolver（Pass 4, pre-pass）は Transformer の runtime 型解決を置き換えるために設計されたが、移行が未完了のまま **2つの並行システム** が共存している:

| 機能 | TypeResolver (pre-pass) | Runtime fallback | 重複 |
|---|---|---|---|
| 式の型解決 | `resolve_expr` → `expr_types` | `resolve_expr_type_heuristic` | **完全重複**: 同一ロジックが2箇所に実装 |
| 期待型の伝搬 | `expected_types`（3パターンのみ） | `ExprContext`（26箇所で設定） | **部分重複**: TypeResolver が不完全 |
| 型ナローイング | `narrowing_events`（typeof/null/instanceof） | `TypeEnv.push_scope/pop_scope` | **完全重複**: テストで実証済み |

### DRY 違反

1. **`TypeResolver.resolve_expr`** (`type_resolver.rs:568-685`) と **`resolve_expr_type_heuristic`** (`type_resolution.rs:46-99`) が同一の AST パターン（identifier lookup, literal type, binary expr, member access, call return type, etc.）を実装
2. TypeResolver の方が完全なスーパーセット（assign, cond, unary, await, TsNonNull 等を追加カバー）であるにもかかわらず、heuristic が残存

### 直交性の欠如

期待型の伝搬が **TypeResolver** と **ExprContext** に分散:
- TypeResolver: 変数宣言 → 初期化式, return 文 → 返り値, 関数引数（registry 関数のみ） — **3 パターン**
- ExprContext: object literal → struct 名, array → 要素型, method 引数, switch case, assignment RHS, nullish coalescing RHS, ternary, etc. — **26 箇所**

これにより:
- テストは ExprContext + TypeEnv を手動構築するか、TypeResolver を経由するかを選ぶ必要がある
- 同じ期待型伝搬ロジックを理解するのに 2 つのコードパスを読む必要がある
- ExprContext を全 convert_* 関数に threading する必要はないが、convert_expr に渡された expected が内部で convert_lit, convert_object_lit, convert_array_lit 等に伝搬する構造のため、中間関数が expected を中継している

### 結合度の問題

`convert_expr` のシグネチャ: `(expr, tctx, reg, ctx, type_env, synthetic)` — 6 パラメータ

- `reg` は `tctx.type_registry` と同一（D5 の課題）
- `ctx: &ExprContext` は expected type を 1 つ持つだけの wrapper
- `type_env` は narrowing が不要になれば大幅に簡素化可能

**理想**: `(expr, tctx, type_env, synthetic)` — 4 パラメータ（reg 統合 + ExprContext 削除）

## 目標アーキテクチャ

```
TypeResolver (pre-pass) → FileTypeResolution (完全・不変)
    ├── expr_types: 全式の型（heuristic を完全に置換）
    ├── expected_types: 全期待型（ExprContext を完全に置換）
    ├── narrowing_events: 全ナローイング（TypeEnv narrowing を完全に置換）
    └── var_mutability: 全変数の可変性

Transformer (post-pass) ← FileTypeResolution を読むのみ
    ├── ExprContext なし（expected は FileTypeResolution から直接読む）
    ├── heuristic なし（expr type は FileTypeResolution から直接読む）
    ├── TypeEnv narrowing なし（narrowing は FileTypeResolution から直接読む）
    ├── TypeEnv は変数宣言追跡のみ残存（将来的に削除可能だが今回はスコープ外）
    └── tctx.type_registry で reg パラメータを統合
```

### 核心となる設計変更

TypeResolver に `propagate_expected` メソッドを追加し、expected_type を複合式（object literal, array literal, function call args 等）の子式に再帰的に伝搬する:

```rust
/// 期待型を子式に再帰的に伝搬する。
///
/// 親が expected type を持つとき、子式（object literal のフィールド値、
/// array literal の要素、function call の引数 等）にも適切な expected type を設定する。
fn propagate_expected(&mut self, expr: &ast::Expr, expected: &RustType) {
    match expr {
        ast::Expr::Object(obj) => {
            // Named(struct) → struct fields の型を各フィールド値に設定
            // Named(enum with tag) → variant fields の型を設定
        }
        ast::Expr::Array(arr) => {
            // Vec<T> → T を各要素に設定
            // Tuple(T1, T2, ...) → Ti を各要素に設定
        }
        ast::Expr::Paren(paren) => {
            // 内側の式に同じ expected を伝搬
        }
        ast::Expr::Cond(cond) => {
            // 両分岐に同じ expected を伝搬
        }
        _ => {} // leaf 式は伝搬不要
    }
}
```

これにより ExprContext の全 26 箇所が TypeResolver の 1 箇所に集約される。

### ExprContext 削除後の Option<T> unwrap 処理

ExprContext が必要だった唯一の技術的理由: Option<T> 期待でリテラルを Some() で wrap する際の無限ループ防止。

解決策: `convert_expr` の公開 API から expected パラメータを除去し、内部実装で Option unwrap の場合のみ override する:

```rust
// 公開 API: expected パラメータなし
pub fn convert_expr(expr, tctx, type_env, synthetic) -> Result<Expr> {
    convert_expr_inner(expr, tctx, None, type_env, synthetic)
}

// 内部実装: Option unwrap 時のみ expected_override を使用
fn convert_expr_inner(
    expr, tctx,
    expected_override: Option<&RustType>,  // Option unwrap 専用
    type_env, synthetic,
) -> Result<Expr> {
    let expected = expected_override
        .or_else(|| tctx.type_resolution.expected_type(span));

    // Option<T> handling
    if let Some(RustType::Option(inner)) = expected {
        if is_literal(expr) {
            // inner type で再帰（FileTypeResolution を再読みしない）
            let inner_result = convert_expr_inner(
                expr, tctx, Some(inner), type_env, synthetic
            )?;
            return Ok(Expr::FnCall { name: "Some", args: vec![inner_result] });
        }
    }

    // 各子式は FileTypeResolution から自分の expected を読む
    match expr {
        Lit(lit) => convert_lit(lit, expected, tctx),
        Object(obj) => convert_object_lit(obj, tctx, type_env, synthetic),
        // ...
    }
}
```

---

## フェーズ構成と依存関係

本ファイルのスコープは Phase 1〜4（型解決の統一）のみ。D5, D1, D6, Phase E は `tasks.md` で管理。

```
Phase 1: TypeResolver expected_types 完全化
    │
    ├─→ Phase 2: ExprContext 削除（Phase 1 完了が前提）
    │       │
    │       └─→ Phase 3: Heuristic 削除（Phase 2 完了が前提）
    │               │
    │               └─→ Phase 4: TypeEnv 簡素化（Phase 3 完了が前提）
    │
    └─→ (tasks.md に戻る: D5 → Phase E)
```

---

## Phase 1: TypeResolver expected_types 完全化

### 目的

TypeResolver が ExprContext の全 26 箇所と同等の expected_type を `FileTypeResolution.expected_types` に設定する。

### 完了条件

- ExprContext を無効化（`with_expected` → `none()`）しても、TypeResolver 経由のテスト（compile_test, snapshot test）が全て GREEN
- 新規テスト: 各伝搬パターンに対して TypeResolver が正しい expected_type を設定することを検証

### タスク

#### 1-1: `propagate_expected` メソッド追加

**ファイル**: `src/pipeline/type_resolver.rs`

`propagate_expected(&mut self, expr: &ast::Expr, expected: &RustType)` メソッドを実装する。以下のパターンを再帰的に処理:

| # | パターン | 対応する ExprContext 設定箇所 | propagate_expected の実装 |
|---|---|---|---|
| P-1 | Object literal + Named(struct) | `data_literals.rs:266,279` | struct 名で registry lookup → 各フィールド値 span に field type を設定 |
| P-2 | Object literal + Named(DU enum) | `data_literals.rs:76,92` | tag field から variant 特定 → variant fields を設定 |
| P-3 | Array literal + Vec<T> | `data_literals.rs:451` | 各要素 span に T を設定 |
| P-4 | Array literal + Tuple(T1,...) | `data_literals.rs:418` | 各要素 span に Ti を設定 |
| P-5 | Paren expr | N/A（convert_expr が再帰） | 内側 expr に同じ expected を設定 |
| P-6 | Cond (ternary) expr | `mod.rs:314`（暗黙的に ctx を再帰） | 両分岐に同じ expected を設定 |
| P-7 | HashMap literal | `data_literals.rs:169` | Named("HashMap") の場合、value type を各値 span に設定 |

**完了条件**: `propagate_expected` が上記全パターンを処理し、expected_types に正しいエントリを挿入する

**依存**: なし

#### 1-2: 変数宣言 → 初期化式の伝搬強化

**ファイル**: `src/pipeline/type_resolver.rs` — `visit_var_decl` メソッド

**現状**: `expected_types.insert(init_span, ann_ty.clone())` のみ（`type_resolver.rs:213`）。初期化式が object literal や array literal の場合、子式への伝搬がない。

**変更**: expected_types 挿入後に `self.propagate_expected(init, &ann_ty)` を呼び出す。

**完了条件**: `const x: Point = { x: 1, y: 2 }` で、`{ x: 1, y: 2 }` の span に `Named("Point")`、`1` の span に `F64`（Point.x の型）、`2` の span に `F64`（Point.y の型）が設定される

**依存**: 1-1

#### 1-3: return 文 → 返り値の伝搬強化

**ファイル**: `src/pipeline/type_resolver.rs` — `visit_stmt` の `Stmt::Return` 分岐

**現状**: `expected_types.insert(span, return_ty.clone())` のみ（`type_resolver.rs:347`）。返り値が compound expr の場合、子式への伝搬がない。

**変更**: expected_types 挿入後に `self.propagate_expected(arg, return_ty)` を呼び出す。

**完了条件**: `function f(): Point { return { x: 1, y: 2 }; }` で、object literal 内の各フィールド値にも expected_type が設定される

**依存**: 1-1

#### 1-4: 関数呼び出し引数の伝搬拡張

**ファイル**: `src/pipeline/type_resolver.rs` — `set_call_arg_expected_types` メソッド

**現状**:
- `ast::Expr::Ident` calleeのみ対応（`type_resolver.rs:817`）
- TypeRegistry の `TypeDef::Function` のみ参照
- scope 内の Fn 型変数は未対応
- method call は未対応

**変更**:

a. **scope 内 Fn 型変数の対応**: callee が Ident の場合、registry に加えて `self.lookup_var()` で Fn 型を探す:
```rust
ast::Expr::Ident(ident) => {
    let fn_name = ident.sym.to_string();
    // 1. Registry lookup (existing)
    if let Some(TypeDef::Function { params, .. }) = self.registry.get(&fn_name) {
        return Some(params.iter().map(|(_, ty)| ty.clone()).collect());
    }
    // 2. NEW: Scope lookup for Fn type variables
    if let ResolvedType::Known(RustType::Fn { params, .. }) = self.lookup_var(&fn_name) {
        return Some(params);
    }
    None
}
```

b. **method call の対応**: callee が `Member(obj.method)` の場合、obj の型から method signature を取得:
```rust
ast::Expr::Member(member) => {
    let obj_type = self.resolve_expr(&member.obj);
    let method_name = extract_ident_name(&member.prop)?;
    let method_sig = self.lookup_method_params(&obj_type, &method_name);
    method_sig
}
```

c. **引数への propagate_expected**: 各引数 span に expected を設定した後、`self.propagate_expected(&arg.expr, &param_ty)` を呼び出す。

**完了条件**:
- `handler(new Request())` で handler が scope 内の Fn 型変数 → Request が引数の expected に設定
- `server.configure({ host: "localhost" })` で configure の param type が引数の expected に設定

**依存**: 1-1

#### 1-5: switch 文の discriminant 型 → case 値への伝搬

**ファイル**: `src/pipeline/type_resolver.rs` — `visit_stmt` の `Stmt::Switch` 分岐

**現状**: discriminant の expr_type を記録するのみ（`type_resolver.rs:398-401`）。case 値への expected 伝搬なし。

**変更**: discriminant の型が Named（enum）の場合、各 case の test 式に expected_type を設定:
```rust
ast::Stmt::Switch(switch_stmt) => {
    let span = Span::from_swc(switch_stmt.discriminant.span());
    let ty = self.resolve_expr(&switch_stmt.discriminant);
    self.result.expr_types.insert(span, ty.clone());

    // NEW: discriminant 型を case values に伝搬
    if let ResolvedType::Known(ref rust_ty) = ty {
        for case in &switch_stmt.cases {
            if let Some(test) = &case.test {
                let test_span = Span::from_swc(test.span());
                self.result.expected_types.insert(test_span, rust_ty.clone());
            }
        }
    }

    for case in &switch_stmt.cases {
        // visit case body (existing)
    }
}
```

**完了条件**: `switch (direction) { case "up": ... }` で `"up"` の span に Direction enum 型が expected として設定

**依存**: なし（propagate_expected は不要）

#### 1-6: assignment RHS への伝搬

**ファイル**: `src/pipeline/type_resolver.rs` — `resolve_expr` の `Expr::Assign` 分岐

**現状**: LHS の mutability tracking + RHS の resolve のみ（`type_resolver.rs:591-597`）。LHS の型を RHS の expected に設定していない。

**変更**: LHS が identifier の場合、その変数の型を RHS の expected_type に設定し、propagate_expected を呼び出す:
```rust
ast::Expr::Assign(assign) => {
    if let Some(ast::SimpleAssignTarget::Ident(ident)) = assign.left.as_simple() {
        self.mark_var_mutable(ident.id.sym.as_ref());
        // NEW: LHS 型を RHS expected に設定
        let lhs_type = self.lookup_var(ident.id.sym.as_ref());
        if let ResolvedType::Known(ref ty) = lhs_type {
            let rhs_span = Span::from_swc(assign.right.span());
            self.result.expected_types.insert(rhs_span, ty.clone());
            self.propagate_expected(&assign.right, ty);
        }
    }
    self.resolve_expr(&assign.right)
}
```

**完了条件**: `x = { name: "Alice" }` で RHS の object literal に x の型が expected として設定

**依存**: 1-1

#### 1-7: nullish coalescing RHS への伝搬

**ファイル**: `src/pipeline/type_resolver.rs` — `resolve_bin_expr` の `NullishCoalescing` 分岐

**現状**: 右辺の型を解決して返すのみ（`type_resolver.rs:706-713`）。左辺が Option<T> の場合、右辺に T を expected として設定していない。

**変更**: 左辺の型が Option<T> の場合、右辺の span に T を expected として設定:
```rust
NullishCoalescing => {
    let left = self.resolve_expr(&bin.left);
    let right_span = Span::from_swc(bin.right.span());

    // NEW: Option<T> の場合、右辺に inner T を expected として設定
    if let ResolvedType::Known(RustType::Option(inner)) = &left {
        self.result.expected_types.insert(right_span, inner.as_ref().clone());
        self.propagate_expected(&bin.right, inner);
    }

    let right = self.resolve_expr(&bin.right);
    if !matches!(right, ResolvedType::Unknown) { right } else { left }
}
```

**完了条件**: `x ?? "default"` で x が Option<String> の場合、`"default"` に String が expected として設定

**依存**: 1-1

#### 1-8: class property 初期化式への伝搬

**ファイル**: `src/pipeline/type_resolver.rs` — `visit_class_decl` の `ClassProp` 分岐

**現状**: 初期化式の expr_type を記録するのみ（`type_resolver.rs:311-315`）。型注釈からの expected 伝搬なし。

**変更**: 型注釈がある場合、初期化式に expected_type を設定:
```rust
ast::ClassMember::ClassProp(prop) => {
    if let Some(init) = &prop.value {
        let span = Span::from_swc(init.span());
        let ty = self.resolve_expr(init);
        self.result.expr_types.insert(span, ty);

        // NEW: 型注釈 → 初期化式 expected
        if let Some(type_ann) = &prop.type_ann {
            if let Ok(ann_ty) = convert_ts_type(&type_ann.type_ann, self.synthetic, self.registry) {
                self.result.expected_types.insert(span, ann_ty.clone());
                self.propagate_expected(init, &ann_ty);
            }
        }
    }
}
```

**完了条件**: `static config: Config = { ... }` で初期化式に Config が expected として設定

**依存**: 1-1

#### 1-9: Optional chain の expr_types 改善

**ファイル**: `src/pipeline/type_resolver.rs` — `resolve_expr` の `OptChain` 分岐

**現状**: `ResolvedType::Unknown` を返す（`type_resolver.rs:655-663`）。

**変更**: base の型を解決し、member の場合は field type を返す。call の場合は method return type を返す:
```rust
ast::Expr::OptChain(opt) => {
    match &*opt.base {
        ast::OptChainBase::Member(member) => {
            let obj_type = self.resolve_expr(&member.obj);
            // Option<T> の場合は inner T を使う
            let inner_type = match &obj_type {
                ResolvedType::Known(RustType::Option(inner)) => {
                    ResolvedType::Known(inner.as_ref().clone())
                }
                other => other.clone(),
            };
            // field type lookup (既存の resolve_member_expr ロジックを再利用)
            // 結果を Option<field_type> で wrap
            // ...
        }
        ast::OptChainBase::Call(call) => {
            // method call の場合: resolve_call_expr ロジックを再利用
            // ...
        }
    }
}
```

**完了条件**: `x?.name` で x が Option<User> の場合、式の型が Option<String>（User.name の型が String の場合）になる

**依存**: なし

#### 1-10: ternary 分岐への expected 伝搬

**ファイル**: `src/pipeline/type_resolver.rs` — `resolve_expr` の `Cond` 分岐

**現状**: 両分岐を resolve して非 Unknown を返すのみ（`type_resolver.rs:598-606`）。

**変更**: 自身の expected_type がある場合（例: 変数宣言の初期化式がテンパー）、両分岐にも同じ expected を設定。ただし、ternary 自体には visit_var_decl や visit_stmt(Return) で既に expected が設定されているため、ここでは propagate_expected 内で処理する（1-1 で対応済み）。

resolve_expr 内では、visit 前に自身の expected を確認する仕組みが必要。方法:
- `propagate_expected` が Cond の cons/alt に expected を設定する
- resolve_expr は変更不要

**完了条件**: `const x: string = cond ? "a" : "b"` で `"a"` と `"b"` の両方に String が expected として設定

**依存**: 1-1, 1-2

#### 1-11: type assertion 内側式への伝搬

**ファイル**: `src/pipeline/type_resolver.rs` — `resolve_expr` の `TsAs` 分岐

**現状**: `convert_ts_type` でターゲット型を解決して返すのみ（`type_resolver.rs:583-587`）。内側式への expected 設定なし。

**変更**: assertion のターゲット型を内側式の expected として設定:
```rust
ast::Expr::TsAs(ts_as) => {
    let target = convert_ts_type(&ts_as.type_ann, self.synthetic, self.registry);
    if let Ok(ref ty) = target {
        // NEW: 内側式に expected 設定
        let inner_span = Span::from_swc(ts_as.expr.span());
        self.result.expected_types.insert(inner_span, ty.clone());
        self.propagate_expected(&ts_as.expr, ty);
    }
    target.map(ResolvedType::Known).unwrap_or(ResolvedType::Unknown)
}
```

**完了条件**: `expr as string` で `expr` の span に String が expected として設定

**依存**: 1-1

#### 1-12: 検証 — ExprContext 無効化で全テスト GREEN

**方法**: `ExprContext::with_expected` を一時的に `none()` に変更し `cargo test` を実行。

ただし、この時点では Transformer のテストは ExprContext 経由で expected を設定しているため、TypeResolver を経由しないテスト（unit test）は失敗する。成功基準は **compile_test + snapshot test** が全 GREEN であること。

unit test の修正は Phase 2（ExprContext 削除）で行う。

**完了条件**: compile_test, snapshot test が ExprContext 無効化でも GREEN

**依存**: 1-1 〜 1-11 全て

---

## Phase 2: ExprContext 削除

### 目的

`ExprContext` struct と `ctx: &ExprContext` パラメータを全関数から除去する。expected type は FileTypeResolution から直接読む。

### 完了条件

- `ExprContext` struct が存在しない
- `convert_expr` のシグネチャから `ctx` パラメータが消えている
- 全テスト GREEN

### タスク

#### 2-1: `convert_expr` のシグネチャ変更

**ファイル**: `src/transformer/expressions/mod.rs`

**変更**:
1. `convert_expr` から `ctx: &ExprContext` パラメータを除去
2. 内部に `convert_expr_inner` を追加し、`expected_override: Option<&RustType>` を持つ
3. `convert_expr` は `convert_expr_inner(expr, tctx, None, type_env, synthetic)` を呼ぶ
4. expected の解決を `expected_override.or_else(|| tctx.type_resolution.expected_type(span))` に変更
5. Option<T> unwrap の再帰呼び出しは `convert_expr_inner(expr, tctx, Some(inner), ...)` を使う

**影響範囲**: `convert_expr` を呼ぶ全箇所。ExprContext を構築して渡していた箇所は、ExprContext の構築を削除するだけ。

**完了条件**: `convert_expr` が `ctx` を取らない。Option unwrap が内部の `expected_override` で動作する

**依存**: Phase 1 全完了

#### 2-2: ExprContext::with_expected 全呼び出しの除去

**対象ファイル** と **対応方針**:

| ファイル | 箇所数 | 対応 |
|---|---|---|
| `statements/mod.rs` | 5 | return type, var decl, array elem, switch case — 全て TypeResolver が設定済みなので ExprContext 構築を削除し `convert_expr` を呼ぶだけ |
| `expressions/data_literals.rs` | 8 | struct field, DU field, HashMap value, tuple/vec element — TypeResolver が propagate_expected で設定済み |
| `expressions/calls.rs` | 4 | function arg, rest param — TypeResolver の set_call_arg_expected_types が設定済み |
| `expressions/functions.rs` | 2 | arrow body return — TypeResolver の current_fn_return_type が設定済み |
| `expressions/binary.rs` | 1 | nullish coalescing RHS — TypeResolver が設定済み（1-7） |
| `expressions/assignments.rs` | 1 | assignment RHS — TypeResolver が設定済み（1-6） |
| `expressions/member_access.rs` | 1 | opt chain method arg — TypeResolver の method call 対応（1-4b）が設定済み |
| `expressions/type_resolution.rs` | 3 | TsAs inner — TypeResolver が設定済み（1-11）|
| `expressions/mod.rs` | 1 | Option inner — convert_expr_inner の expected_override で処理 |
| `classes.rs` | 1 | static prop init — TypeResolver が設定済み（1-8） |

**完了条件**: `ExprContext::with_expected` の呼び出しがゼロ

**依存**: 2-1

#### 2-3: ExprContext::none 全呼び出しの除去

**対象**: 全ファイルで `ExprContext::none()` を渡している箇所（約 100 箇所）。`convert_expr` から `ctx` パラメータが消えたため、これらは全て単純に削除する（引数リストから `&ExprContext::none()` を消す）。

**方法**: `/large-scale-refactor` スキルに従う。sed/grep による一括置換。

**完了条件**: `ExprContext::none()` の呼び出しがゼロ

**依存**: 2-1

#### 2-4: ExprContext struct 削除

**ファイル**: `src/transformer/expressions/mod.rs`

`ExprContext` struct と `impl ExprContext` を削除。`pub use ExprContext` も削除。

**完了条件**: `ExprContext` が codebase 内に存在しない

**依存**: 2-2, 2-3

#### 2-5: テスト更新

ExprContext を使っていたユニットテストを更新:
- TypeResolver 経由でテストする場合: `FileTypeResolution` に expected_types を設定
- Direct テストの場合: `tctx.type_resolution.expected_types` にエントリを追加

**完了条件**: `cargo test` 全 GREEN

**依存**: 2-4

---

## Phase 3: Heuristic 削除

### 目的

`resolve_expr_type` と `resolve_expr_type_heuristic` を削除し、Transformer が FileTypeResolution.expr_type のみを使うようにする。

### 完了条件

- `resolve_expr_type` 関数が存在しない
- `resolve_expr_type_heuristic` 関数が存在しない
- Transformer 内の全 `resolve_expr_type` 呼び出しが `tctx.type_resolution.expr_type(span)` に置換

### タスク

#### 3-1: `resolve_expr_type` 呼び出し箇所の置換

**対象**: Transformer 内の全 `resolve_expr_type(expr, type_env, tctx, reg)` 呼び出し（約 30 箇所）。

**置換先**: `tctx.type_resolution.expr_type(Span::from_swc(expr.span()))` を使い、`ResolvedType::Known(ty)` を `Some(ty)` に変換するヘルパーを用意:

```rust
/// FileTypeResolution から式の型を取得する。Unknown なら None。
fn get_expr_type(tctx: &TransformContext<'_>, expr: &ast::Expr) -> Option<&RustType> {
    match tctx.type_resolution.expr_type(Span::from_swc(expr.span())) {
        ResolvedType::Known(ty) => Some(ty),
        ResolvedType::Unknown => None,
    }
}
```

**対象ファイルと箇所数**:

| ファイル | 呼び出し数 | 用途 |
|---|---|---|
| `expressions/binary.rs` | 6 | string concat 判定, typeof operand 型, unary plus operand 型 |
| `expressions/calls.rs` | 3 | format arg 型, method signature lookup, trait coercion |
| `expressions/member_access.rs` | 5 | opt chain base 型, DU field access, tuple index |
| `expressions/patterns.rs` | 3 | typeof guard, in operator object 型 |
| `expressions/mod.rs` | 1 | trait coercion wrapping |
| `statements/mod.rs` | 6 | cond assign, switch discriminant, destructuring source, DU tag field |
| `expressions/type_resolution.rs` | 6 | 内部再帰呼び出し（全削除対象） |

**方法**: `/large-scale-refactor` スキルに従う。

**完了条件**: `resolve_expr_type` の呼び出しがゼロ

**依存**: Phase 2 完了

#### 3-2: `resolve_expr_type` 関連関数の削除

**ファイル**: `src/transformer/expressions/type_resolution.rs`

以下の関数を削除:
- `resolve_expr_type`（公開関数、エントリポイント）
- `resolve_expr_type_heuristic`（内部関数）
- `resolve_bin_expr_type`（heuristic 内部の二項演算子型解決）
- `resolve_call_return_type`（heuristic 内部の呼び出し戻り型解決）
- `resolve_new_expr_type`（heuristic 内部の new 式型解決）
- `resolve_field_type`（heuristic 内部のフィールド型解決）

残す関数:
- `resolve_method_return_type`（pub(super)、member_access.rs が使用。ただし Phase 3 で FileTypeResolution に置換できれば削除）
- `convert_ts_as_expr`（型アサーションの IR 変換。型解決ではなくコード生成）

**完了条件**: 上記関数が削除され、`cargo test` が GREEN

**依存**: 3-1

#### 3-3: `resolve_method_return_type` の置換検討

**ファイル**: `src/transformer/expressions/type_resolution.rs:182`, `src/transformer/expressions/member_access.rs`

`resolve_method_return_type` は method call の返り型を TypeRegistry から取得する。TypeResolver の `resolve_method_return_type`（`type_resolver.rs:839`）が同等の機能を持つ。

Transformer 側の `resolve_method_return_type` を呼んでいる箇所が、代わりに `tctx.type_resolution.expr_type(call_span)` で method call 全体の型を取得できれば、この関数も削除可能。

**完了条件**: `resolve_method_return_type`（Transformer 側）が削除されるか、残す正当な理由が文書化される

**依存**: 3-2

#### 3-4: heuristic fallback テストの削除・書き換え

以下のテストは heuristic 自体のテストであり、heuristic 削除後は不要:
- `test_resolve_expr_type_falls_back_when_resolution_unknown`
- `test_resolve_expr_type_falls_back_when_span_not_in_resolution`
- `test_resolve_expr_type_narrowing_from_file_resolution_overrides_type_env`

resolve_expr_type 経由で型解決をテストしていたテスト（16 件: `test_resolve_expr_type_*`）は、TypeResolver のテストに書き換えるか、get_expr_type ヘルパーのテストに変更する。

**完了条件**: 全テスト GREEN。heuristic に依存するテストが存在しない

**依存**: 3-2

#### 3-5: `type_env` パラメータの部分的除去

Heuristic 削除により、`type_env` を `resolve_expr_type` に渡していた箇所が消える。以下のファイルで `type_env` パラメータが不要になる可能性を検討:

- `expressions/type_resolution.rs` の全関数から `type_env` を除去（heuristic が消えるため）
- ただし、Transformer の他の箇所（statements/mod.rs の narrowing guard, calls.rs の fn type lookup 等）では引き続き必要

**完了条件**: type_resolution.rs から type_env 依存が消える

**依存**: 3-2

---

## Phase 4: TypeEnv 簡素化

### 目的

TypeEnv から narrowing 関連の用途を除去する。TypeEnv は変数宣言の型追跡のみに使用する。

### 完了条件

- TypeEnv の `push_scope` / `pop_scope` が narrowing 目的で使われていない
- `push_scope` / `pop_scope` が block scope の変数追跡にのみ使われる（or 完全削除）
- 全テスト GREEN

### タスク

#### 4-1: narrowing 用 push_scope/pop_scope の削除

**ファイル**: `src/transformer/statements/mod.rs`

以下の箇所で、narrowing 目的の push_scope → insert(narrowed_type) → pop_scope パターンを削除:

| 行範囲 | パターン | 対応 |
|---|---|---|
| 759-769 | if-then narrowing scope | 削除。FileTypeResolution.narrowed_type が代替 |
| 777-786 | if-else narrowing scope | 削除 |
| 825-839 | compound condition narrowing | 削除 |
| 2458-2471 | switch case narrowing scope | 削除 |
| 2659-2674 | switch case variant narrowing | 削除 |

**検証**: D-TR-1 で TypeEnv narrowing 無効化時に変換テストが全 GREEN だったため、削除しても安全。

**完了条件**: narrowing 目的の push_scope/pop_scope が存在しない

**依存**: Phase 3 完了

#### 4-2: TypeEnv.update() の削除

`type_env.update()` は codebase 内で一度も呼ばれていない（探索で確認済み）。メソッド自体を削除する。

**完了条件**: `TypeEnv::update` メソッドが存在しない

**依存**: なし

#### 4-3: TypeEnv ユニットテストの更新

TypeEnv の narrowing 関連テスト 4 件を更新:
- `test_type_env_nested_scopes_three_levels`
- `test_type_env_pop_scope_removes_child_variables`
- `test_type_env_shadow_in_child_scope_hides_parent`
- `test_type_env_update_nonexistent_inserts_in_current_scope`

push_scope/pop_scope が残る場合（変数スコープ管理用途）は変更不要。削除される場合はテストも削除。

**完了条件**: `cargo test` 全 GREEN

**依存**: 4-1

---

## 作業量の見積もり（参考）

| Phase | 変更ファイル数 | 変更関数数 | 新規テスト | 主なリスク |
|---|---|---|---|---|
| 1 | 1 (type_resolver.rs) | ~15 | ~20 | propagate_expected の再帰ロジックの正しさ |
| 2 | ~10 | ~30 | ~15 | Option unwrap の expected_override が正しく動作するか |
| 3 | ~8 | ~30 | テスト書き換え ~20 | resolve_expr_type 依存の見落とし |
| 4 | 2 | ~5 | テスト修正 ~4 | narrowing 削除の影響漏れ（D-TR-1 で検証済み） |

---

## リスクと対策

### リスク 1: propagate_expected の再帰が不完全

**対策**: Phase 1-12 で ExprContext 無効化テストを実行し、全パターンがカバーされていることを検証する。失敗するテストがあれば、そのパターンを追加してから Phase 2 に進む。

### リスク 2: TypeResolver が Transformer と異なる型を解決する

**対策**: Phase 1 では既存コードを変更しない（TypeResolver に追加するのみ）。Phase 2 の ExprContext 削除時に、各テストの出力が変わらないことを確認する。

### リスク 3: span の衝突（同一 span に複数の expected_type）

**対策**: 例えば `1` というリテラルが variable initializer かつ array element の場合、TypeResolver の propagate_expected が深い方（array element → element type）を後から設定するため、後勝ちになる。これは ExprContext の動作と同じ（子の ExprContext が親を override する）ため、問題ない。

### リスク 4: テスト構造の問題（unit test が TypeResolver を経由しない）

**対策**: Phase 2-3 のテスト更新で、unit test を以下の2パターンに分類:
1. **TypeResolver 経由テスト**: `TransformContext` に実際の `FileTypeResolution` を設定
2. **直接テスト**: `FileTypeResolution.expected_types` にエントリを手動追加

既存テストの多くは pattern 2 で対応可能（TypeEnv の代わりに expected_types にエントリを追加するだけ）。
