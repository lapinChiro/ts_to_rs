# Expected Type 伝搬の一本化

**調査レポート**: `report/expected-type-dual-propagation.md`
**依存**: Phase 2（ExprContext 削除）完了後
**前提**: Phase 3（Heuristic 削除）の**前**に実施

---

## 目的

TypeResolver の `propagate_expected` と Transformer の手動伝搬（`convert_expr_with_expected` 経由）の二重性を解消し、expected type の伝搬ロジックを TypeResolver に一本化する。

## 完了条件

1. Transformer のプロダクションコードに `convert_expr_with_expected` の呼び出しが存在しない（mod.rs 内の内部再帰呼び出しを除く）
2. `convert_expr_with_expected` が private 関数（Option unwrap 再帰専用）として存在。`pub(super)` / `pub(crate)` ではない
3. TypeResolver の `propagate_expected` が全パターンをカバーしている
4. unit test が TypeResolver 経由で expected type を設定している（テストヘルパー経由）
5. `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
6. `cargo test` 全 GREEN（unit + E2E + compile_test）

---

## Phase 2.5-A: TypeResolver のギャップ埋め ✅

### 目的

TypeResolver の `propagate_expected` に不足しているパターンを追加し、production pipeline で全てのケースの expected type が正しく設定されるようにする。

### 完了条件（全達成）

- [x] 下記 5 パターンが `propagate_expected` または関連メソッドに追加されている
- [x] 既存の TypeResolver テスト全 GREEN
- [x] 新パターンごとに TypeResolver のテストが 1 件以上追加されている

### 追加で実施した修正

- `visit_var_decl` 再構成: `resolve_expr` 3回→1回。expected type 設定を resolution の前に移動
- `resolve_arrow_expr` / `resolve_fn_expr` に expected type 読み取りを追加（`resolve_fn_type_info` ヘルパー）
- 関数型エイリアス注釈からの return type/param types 推論
- `test_var_type_alias_arrow` 修正（ネストされた object literal の struct name 推論）

### タスク

#### 2.5-A-1: DU object literal fields への expected type 伝搬

**ファイル**: `src/pipeline/type_resolver.rs` の `propagate_expected` メソッド内

**現状**: P-1（lines 601-638）は `TypeDef::Struct` のフィールドのみ lookup する。`TypeDef::Enum`（discriminated union）の variant fields は lookup していない。

**修正**: P-1 の `reg.get(name)` ブロックに `TypeDef::Enum { variant_fields, .. }` のケースを追加。discriminant field の値から variant を特定し、variant_fields から各フィールドの expected type を設定する。

**検証**: `propagate_expected` のテストで DU object literal を検証。

**依存**: なし

#### 2.5-A-2: HashMap value への expected type 伝搬

**ファイル**: `src/pipeline/type_resolver.rs` の `propagate_expected` メソッド内

**現状**: `propagate_expected` は `RustType::Named { name: "HashMap", type_args }` を処理しない。

**修正**: P-1 の前（または内部分岐として）、expected が `HashMap<K, V>` の場合、object literal の各 computed key-value の value に `V` を設定する。

**検証**: TypeResolver テストで `{ [key]: "value" }` パターンを検証。

**依存**: なし

#### 2.5-A-3: Arrow expression body への return type 伝搬

**ファイル**: `src/pipeline/type_resolver.rs`

**現状**: `resolve_expr` 内の arrow function 処理（`ast::Expr::Arrow`）で return type を解決しているが、expression body への `propagate_expected` 呼び出しがない。

**修正**: arrow function の expression body に対し、解決した return type で `propagate_expected` を呼ぶ。block body の場合は return statement 経由で既に設定されている（visit_stmt:358）ため、expression body のみ。

**検証**: `const f = (): string => "hello"` で `"hello"` に `String` の expected が設定されることを確認。

**依存**: なし

#### 2.5-A-4: Rest parameter args への element type 伝搬

**ファイル**: `src/pipeline/type_resolver.rs` の `set_call_arg_expected_types`

**現状**: `set_call_arg_expected_types` (lines 956-997) は通常パラメータのみ処理。rest parameter の element type 伝搬がない。

**修正**: パラメータ数 < 引数数の場合、最後のパラメータが `Vec<T>` なら、超過分の引数に `T` を設定する。

**検証**: `fn foo(a: f64, ...rest: Vec<String>)` の rest 引数に String が設定されることを確認。

**依存**: なし

#### 2.5-A-5: Optional chaining method call args への param type 伝搬

**ファイル**: `src/pipeline/type_resolver.rs` の `set_call_arg_expected_types`

**現状**: `set_call_arg_expected_types` は `ast::Expr::Call` と直接メソッドコールを処理するが、`ast::Expr::OptChain` 内のメソッドコールを処理していない可能性がある。

**修正**: `resolve_expr` 内の OptChain 処理で、内部が CallExpr の場合に `set_call_arg_expected_types` を呼ぶ。

**検証**: `obj?.method("hello")` で `"hello"` に method param type が設定されることを確認。

**依存**: なし

---

## Phase 2.5-B: テストヘルパー整備 ✅

### 目的

unit test で TypeResolver 経由の expected type 設定を容易にするヘルパーを整備する。

### 完了条件（全達成）

- [x] `TctxFixture` に TypeResolver 経由の構築メソッドが追加されている
- [x] 新メソッドを使用したテストが 3 件以上ある（既存テストの書き換え）

### タスク

#### 2.5-B-1: TctxFixture に TypeResolver 統合メソッドを追加

**ファイル**: `src/transformer/test_fixtures.rs`

**追加メソッド**:

```rust
/// TS ソースコードを解析し、TypeResolver を実行して TransformContext を構築する。
/// unit test で TypeResolver 経由の expected type 設定をテストする場合に使用。
pub fn from_source(source: &str) -> Self { ... }

/// TS ソースコードを解析し、カスタム TypeRegistry と TypeResolver を使用して構築する。
pub fn from_source_with_reg(source: &str, reg: TypeRegistry) -> Self { ... }
```

**実装要件**:
- `parse_typescript(source)` で AST を取得
- `build_registry(&module)` で TypeRegistry を構築（from_source の場合）
- `TypeResolver::new(&reg, &mut synthetic, &mg).resolve_file(&parsed)` で FileTypeResolution を取得
- 取得した FileTypeResolution と TypeRegistry を TctxFixture に格納

**検証**: 既存の `context.rs` テスト 3 件を新メソッドを使用して書き換え。

**依存**: なし

#### 2.5-B-2: Span 取得ヘルパーの検討

**ファイル**: `src/transformer/test_fixtures.rs` または `src/transformer/expressions/tests.rs`

**課題**: `FileTypeResolution.expected_types` に手動でエントリを追加するには span が必要だが、unit test では span の値を知るのが困難。TypeResolver 経由で設定すれば span は自動的に正しい値になる。

**検討事項**: TctxFixture::from_source を使えば span の手動管理は不要になるため、別途 span ヘルパーは不要かもしれない。

**依存**: 2.5-B-1

---

## Phase 2.5-C: unit test の TypeResolver 経由移行 ✅

### 目的

`convert_expr_with_expected` の `expected_override` に直接 expected を渡す既存テストを、TypeResolver 経由の expected type 設定に移行する。

### 完了条件（全達成）

- [x] `convert_expr_with_expected` を `Some(&expected)` で呼ぶテストが存在しない
- [x] 全テストが TypeResolver 経由の expected type、または FileTypeResolution への手動挿入で動作する
- [x] テストの検証内容（assert）は変更しない

### タスク

#### 2.5-C-1: expressions/tests.rs の移行（約 50 テスト）

**ファイル**: `src/transformer/expressions/tests.rs`

**対象**: `super::convert_expr_with_expected` を `Some(&expected)` で呼ぶ全テスト。

**移行パターン**:

```rust
// Before: 手動 expected override
let expected = RustType::Vec(Box::new(RustType::String));
let result = super::convert_expr_with_expected(
    &expr, &tctx, f.reg(), Some(&expected), &TypeEnv::new(), &mut SyntheticTypeRegistry::new(),
).unwrap();

// After: TypeResolver 経由
let f = TctxFixture::from_source(r#"const a: string[] = ["a", "b"];"#);
let tctx = f.tctx();
let swc_expr = extract_var_init(&f.module());  // ヘルパーで initializer を取得
let result = convert_expr(
    &swc_expr, &tctx, f.reg(), &TypeEnv::new(), &mut SyntheticTypeRegistry::new(),
).unwrap();
```

**注意**: テストによっては TypeResolver がカバーしていないパターンを検証している場合がある。その場合は 2.5-A のギャップ埋めが先に必要。

**依存**: 2.5-A（全タスク）, 2.5-B-1

#### 2.5-C-2: statements/tests.rs の移行

**ファイル**: `src/transformer/statements/tests.rs`

**対象**: `convert_expr_with_expected` を使用するテスト。statements テストの多くは `convert_stmts` ヘルパー経由で、expected は TypeResolver が設定する。Phase 2 の修正で statements/mod.rs に追加された手動伝搬（return stmt, var decl）を経由するテストを TypeResolver 経由に切り替える。

**依存**: 2.5-A, 2.5-B-1

#### 2.5-C-3: classes.rs, functions/tests.rs, context.rs の移行

**ファイル**: 各テストファイル

**対象**: `convert_expr_with_expected` を使用する残りのテスト。

**依存**: 2.5-A, 2.5-B-1

---

## Phase 2.5-D: Transformer の手動伝搬削除 ✅

### 目的

Transformer プロダクションコードから `convert_expr_with_expected` の呼び出しを全て除去し、`convert_expr` に統一する。

### 完了条件（全達成）

- [x] Transformer プロダクションコードに `convert_expr_with_expected` の呼び出しが存在しない（mod.rs 内の内部再帰を除く）
- [x] `convert_expr_with_expected` が private 関数（Option unwrap 再帰専用）に変更。`pub(super)` / `pub(crate)` ではない
- [x] 全テスト GREEN（unit 1115 + CLI 3 + compile 2 + E2E 60 + integration 69）

### タスク

#### 2.5-D-1: Transformer プロダクションコードの `convert_expr_with_expected` → `convert_expr` 置換

**対象ファイルと箇所数**:

| ファイル | 箇所数 |
|---|---|
| data_literals.rs | 8 |
| calls.rs | 3 |
| member_access.rs | 1 |
| assignments.rs | 1 |
| binary.rs | 1 |
| functions.rs | 2 |
| statements/mod.rs | 2 |
| classes.rs | 1 |

**合計**: 19 箇所

**方法**: 各 `super::convert_expr_with_expected(expr, tctx, reg, expected, type_env, synthetic)` を `convert_expr(expr, tctx, reg, type_env, synthetic)` に置換。expected type の計算コード（field_expected, element_type, rest_element_type 等）も不要になるため削除。

**依存**: 2.5-A（TypeResolver が全パターンをカバーしていること）, 2.5-C（テストが TypeResolver 経由に移行していること）

#### 2.5-D-2: `convert_expr_with_expected` の private 化

**ファイル**: `src/transformer/expressions/mod.rs`

**結果**: `convert_expr_with_expected` は削除せず private 関数として残した。Option\<T\> の unwrap + Some 付与ロジック（mod.rs:84-122）が `expected_override` パラメータによる再帰を使うため、`convert_expr` への統合では無限再帰を回避できない。`convert_expr` → `convert_expr_with_expected(None)` → Option 検出時に `convert_expr_with_expected(Some(inner))` と再帰する構造が必要。

**依存**: 2.5-D-1

#### 2.5-D-3: Option\<Option\<T\>\> 二重ラップバグの検証

**現状の問題**: `calls.rs` の `convert_call_args_with_types` と `mod.rs` の `convert_expr_with_expected` の間で、`param_ty` が `Option<Option<T>>` の場合に非リテラル引数の `Some()` ラップが不整合になる。

**メカニズム**:
- `calls.rs:588`: `Option<Option<T>>` から inner を抽出 → `expected = Some(&Option<T>)`
- `mod.rs:85-100`: expected が `Option<T>` → リテラルなら `Some(lit)` に包むが、非リテラル（変数等）は包まない
- `calls.rs:600-607`: `param_ty` が `Option<_>` → 結果をさらに `Some(...)` で包む
- 結果: リテラル `42` → `Some(Some(42.0))`（正しい）、変数 `x` → `Some(x)`（`Some(Some(x))` が正しい）

**影響**: TS→Rust の変換で `Option<Option<T>>` が生成されるケースは実質的に存在しないため、現時点で実害はない。

**解消**: 2.5-D-1 で `convert_expr_with_expected` をプロダクションコードから削除すれば、この二重ラップ問題は消滅する。TypeResolver 経由なら各引数の span に正確な expected type（`Option<Option<T>>` そのもの）が設定され、`convert_expr` 内の Option ハンドリングが正しく動作する。

**検証**: 2.5-D-1 完了後、`Option<Option<f64>>` パラメータへの非リテラル引数が正しく `Some(Some(x))` に変換されることをテストで確認する。

**依存**: 2.5-D-1

---

## 設計レビュー修正（Phase 2.5 完了後）

### 修正 A: Option wrapping の責務一本化 ✅

Option\<T\> ラッピングが `mod.rs`, `calls.rs`, `statements/mod.rs` の 3 箇所に分散していた問題を修正。`convert_expr_with_expected` 内の Option ハンドリングに一本化し、`resolve_expr_type` + `ast_produces_option` で二重ラップを防止。

- `calls.rs` と `statements/mod.rs` の Option wrapping ロジックを削除
- `ast_produces_option` ヘルパーで AST レベルの Option 判定（optional chaining, ternary with null — 再帰的にネストされた三項演算子も検出）。Phase 3 タスク 3-7 で型解決ベースに置換予定
- calls.rs は `convert_expr`（TypeResolver 経由の expected type）で call arg を変換。`convert_expr_with_expected` は private のまま

### 修正 B: `_tctx` パラメータ削除 ✅

`resolve_member_access` の未使用 `_tctx: &TransformContext` パラメータを削除。

### 修正 C: `set_expected_types_in_nested_calls` 文書化 ✅

アーキテクチャ制約の doc comment を追加。根本解決は Phase 3 タスク 3-5 として追記。

---

## 作業量の見積もり（参考）

| Phase | 変更ファイル数 | 主な作業内容 | リスク |
|---|---|---|---|
| 2.5-A | 1 (type_resolver.rs) | propagate_expected に 5 パターン追加 + テスト | propagate_expected の再帰ロジック正しさ |
| 2.5-B | 1 (test_fixtures.rs) | TctxFixture 拡張 | なし（additive） |
| 2.5-C | 5 (テストファイル) | 50+ テスト書き換え | テストの意図変更リスク |
| 2.5-D | 9 (プロダクション + mod.rs) | 19 箇所置換 + 関数削除 | 伝搬漏れ |

## リスクと対策

### リスク 1: TypeResolver の propagate_expected と Transformer の手動伝搬で結果が異なるケース

**対策**: 2.5-A で各パターンを追加する際、既存の手動伝搬テストを TypeResolver 経由に書き換え、出力が同一であることを確認する。差異があれば TypeResolver 側を修正。

### リスク 2: テスト書き換え時に span の不一致

**対策**: `TctxFixture::from_source` を使えば TypeResolver が正しい span を設定する。手動 span 管理は不要。

### リスク 3: Phase 3 との作業順序

**対策**: Phase 2.5 を Phase 3 の前に完了する。Phase 2.5-D で `convert_expr_with_expected` が削除されれば、Phase 3 の `resolve_expr_type` 削除がよりクリーンになる。

---

## Phase 3 以降への影響

Phase 2.5 完了後:
- Transformer は expected type を `tctx.type_resolution.expected_type(span)` からのみ読む
- `convert_expr_with_expected` は private（Option unwrap 再帰専用）。プロダクションコードからは `convert_expr` のみ使用
- Phase 3 の `resolve_expr_type` → `tctx.type_resolution.expr_type(span)` 置換がシンプルになる
- Phase 4 の TypeEnv 簡素化も、TypeResolver がより多くの情報を持つため容易になる
