# Phase 2.5 設計レビュー修正 — 完全版

## Context

Phase 2.5-A〜D 完了後の設計レビューで、Phase 2.5 の完了条件を満たしていない箇所と妥協的な実装を発見した。
Phase 2.5 を完璧な状態にしてから Phase 3 に進む。

---

## 問題の全体像

### 問題 1: calls.rs が `convert_expr_with_expected` を再導入（Phase 2.5 完了条件違反）

**現状**: 設計レビュー修正 A で、calls.rs の Option wrapping を `convert_expr_with_expected` に一本化した。しかし、calls.rs が `param_ty` を `convert_expr_with_expected` に明示的に渡すことで、Phase 2.5 で排除した「Transformer からの手動 expected type 伝搬」が再導入された。

**違反している完了条件**（`tasks.expected-type-unification.md` L15-16）:
1. ~~Transformer のプロダクションコードに `convert_expr_with_expected` の呼び出しが存在しない~~ → calls.rs L586 に存在
2. ~~`convert_expr_with_expected` が削除されている（または `#[cfg(test)]` のみ）~~ → `pub(super)` で公開中

**根本原因**: テスト `test_call_with_option_arg_wraps_some` が `TctxFixture::with_reg(reg)` を使い TypeResolver を経由しないため、TypeResolver が call arg の expected type を設定しない。テストを通すために calls.rs から `param_ty` を明示的に渡す妥協が行われた。

**事実**: TypeResolver の `set_call_arg_expected_types` は TypeRegistry からの関数定義を正しく参照する（`self.registry.get(&fn_name)` → `TypeDef::Function { params, ... }`）。`TctxFixture::from_source_with_reg` を使えば TypeResolver が call arg の expected type を自動設定する。したがって calls.rs から `param_ty` を明示的に渡す必要はない。

### 問題 2: `convert_expr_with_expected` が `pub(super)` のまま

**現状**: 問題 1 の結果として `pub(super)` に変更されている。

**あるべき姿**: 完了条件に従い、private（Option unwrap 再帰専用）であるべき。

### 問題 3: ドキュメントの陳腐化

**tasks.md**:
- L24: Phase 2.5 完了済みなのに「Phase 2.5 で解消予定」のまま
- L45: `← 次の作業` → `← 完了` であるべき
- L74-78: Phase 2.5-B, C, D が未チェック `[ ]`。全て完了済み
- L79: `3-1〜3-5` → `3-1〜3-7`（3-5 追加、旧 3-5 → 3-6、旧 3-6 → 3-7 にリナンバ）

**tasks.expected-type-unification.md**:
- L15-16: 完了条件 1, 2 の記述が現状と乖離
- L225-226: 2.5-D 完了条件チェックが現状と不一致
- L330-331: 「`convert_expr_with_expected` は存在しない」→ 存在する

**tasks.type-resolution-unification.md**:
- L533: `expressions/mod.rs | 1` → 修正 A で `resolve_expr_type` が 1 箇所追加され現在 2 箇所

### 問題 4: Phase 3 タスク漏れ — `ast_produces_option` 削除

**現状**: `ast_produces_option` は `resolve_expr_type` が Cond/OptChain の型を正確に返さないための AST レベルのヒューリスティック。Phase 3 で `resolve_expr_type` が `tctx.type_resolution.expr_type()` に置換された後、TypeResolver が Cond/OptChain の expr_type を正しく返せば不要になる。

**必要なタスク**:
1. TypeResolver が Cond 式の expr_type として `Option<T>` を返すよう強化（一方のブランチが null/undefined の場合）
2. `ast_produces_option` と `is_null_or_undefined` ヘルパーを削除
3. Option wrapping 判定を `tctx.type_resolution.expr_type()` のみに依存

---

## 修正計画

### 実装順序

```
タスク 1: calls.rs — convert_expr に戻す ✅
    ↓
タスク 2: テスト修正（from_source_with_reg 移行） ✅
    ↓
タスク 3: convert_expr_with_expected を private に戻す ✅
    ↓
タスク 4: ドキュメント更新（4 ファイル） ✅
```

全タスク完了。

---

### タスク 1: calls.rs — `convert_expr_with_expected` → `convert_expr` に戻す

**ファイル**: `src/transformer/expressions/calls.rs`

**変更箇所**: L584-588

```rust
// Before (現状):
let mut expr = super::convert_expr_with_expected(
    &arg.expr, tctx, reg, param_ty, type_env, synthetic,
)?;

// After:
let mut expr = convert_expr(&arg.expr, tctx, reg, type_env, synthetic)?;
```

**理由**: TypeResolver の `set_call_arg_expected_types` が call arg の span に expected type を設定する。`convert_expr` は `tctx.type_resolution.expected_type(span)` を読むので、明示的な `param_ty` 伝搬は不要。

**残す処理**:
- `Box::new` wrapping（Fn パラメータ用）: L606-611 — これは expected type とは別の責務。型に基づく後処理であり、TypeResolver の expected type では制御できない
- Trait coercion: L614-604 — 同上
- Missing Option パラメータの `None` 付加: L663-668 — call 引数数と param 数の差分処理。TypeResolver は影響しない

**リスク**: なし。production pipeline では TypeResolver が必ず実行されるため、call arg の expected type は設定済み。

### タスク 2: テスト修正

**ファイル**: `src/transformer/expressions/tests.rs`

#### 2-1: `test_call_with_option_arg_wraps_some` (L4522-4566)

**Before**:
```rust
let f = TctxFixture::with_reg(reg);
let tctx = f.tctx();
let swc_expr = parse_expr("greet(\"World\", \"Hi\")");
let result = convert_expr(&swc_expr, &tctx, f.reg(), ...);
```

**After**:
```rust
let f = TctxFixture::from_source_with_reg("greet(\"World\", \"Hi\");", reg);
let tctx = f.tctx();
let swc_expr = extract_expr_stmt(f.module(), 0);
let result = convert_expr(&swc_expr, &tctx, f.reg(), ...);
```

**変更理由**: TypeResolver が `set_call_arg_expected_types` で `"Hi"` のspan に `Option<String>` を設定する。`convert_expr` がそれを読み、Option wrapping が自動的に行われる。

**assert は変更なし**: `args[1]` が `Some(...)` であることを引き続き検証。

#### 2-2: `test_call_with_missing_default_arg_appends_none` (L4475-4519)

**Before**:
```rust
let f = TctxFixture::with_reg(reg);
let tctx = f.tctx();
let swc_expr = parse_expr("greet(\"World\")");
let result = convert_expr(&swc_expr, &tctx, f.reg(), ...);
```

**After**:
```rust
let f = TctxFixture::from_source_with_reg("greet(\"World\");", reg);
let tctx = f.tctx();
let swc_expr = extract_expr_stmt(f.module(), 0);
let result = convert_expr(&swc_expr, &tctx, f.reg(), ...);
```

**変更理由**: Phase 2.5 の原則に従い、全テストで TypeResolver を経由する。`None` 付加は calls.rs 内の `param_types` 比較ロジック（L663-668）が処理するが、テストの構造は TypeResolver 経由に統一する。

**注意**: `None` 付加ロジックは `convert_call_args_with_types` 内で `param_types`（TypeRegistry から直接取得）を参照する。TypeResolver 経由にしてもこのロジックは `param_types` に依存するため、動作は変わらない。

**assert は変更なし**: `args[1]` が `Ident("None")` であることを引き続き検証。

### タスク 3: `convert_expr_with_expected` を private に戻す

**ファイル**: `src/transformer/expressions/mod.rs`

**変更**:
```rust
// Before (現状):
pub(super) fn convert_expr_with_expected(

// After:
fn convert_expr_with_expected(
```

**前提**: タスク 1 完了後、calls.rs からの参照がなくなっていること。

**検証**: `cargo check` でコンパイルエラーがないこと。calls.rs からの参照が残っている場合はコンパイルエラーになるため、安全。

### タスク 4: ドキュメント更新

#### 4-1: `tasks.md`

| 行 | 変更内容 |
|---|---|
| L24 | 「Phase 2.5 で解消予定」→ 「Phase 2.5 で解消済み」に更新。ただし `convert_expr_with_expected` は private（Option unwrap 再帰専用）として残存する旨を記載 |
| L45 | `← 次の作業` → `← 完了` |
| L74 | `- [ ] **Phase 2.5**:` → `- [x] **Phase 2.5**:` |
| L76 | `- [ ] **Phase 2.5-B**:` → `- [x] **Phase 2.5-B**:` |
| L77 | `- [ ] **Phase 2.5-C**:` → `- [x] **Phase 2.5-C**:` |
| L78 | `- [ ] **Phase 2.5-D**:` → `- [x] **Phase 2.5-D**:` |
| L79 | `3-1〜3-5` → `3-1〜3-7` |

#### 4-2: `tasks.expected-type-unification.md`

| 行 | 変更内容 |
|---|---|
| L15 | 完了条件を現状に合わせる: `convert_expr_with_expected` は private（Option unwrap 再帰専用）。プロダクションコードの呼び出しが存在しない |
| L16 | `削除されている（または #[cfg(test)] のみ）` → `private 関数（Option unwrap 再帰専用）として存在。pub(super) / pub(crate) ではない` |
| L225 | 完了条件を正確に: `Transformer プロダクションコードに convert_expr_with_expected の呼び出しが存在しない（mod.rs 内の内部再帰呼び出しを除く）` |
| L226 | `private 関数（Option unwrap 再帰専用）に変更` のまま（修正後は正確） |
| L330-331 | `convert_expr_with_expected は存在しない` → `convert_expr_with_expected は private（Option unwrap 再帰専用）。Transformer のプロダクションコードから呼ばれない` |

#### 4-3: `tasks.type-resolution-unification.md`

| 箇所 | 変更内容 |
|---|---|
| L533 | `expressions/mod.rs \| 1 \| trait coercion wrapping` → `expressions/mod.rs \| 2 \| Option wrapping 判定, trait coercion wrapping` |
| Phase 3 タスクに追記 | 3-7（新規）: `ast_produces_option` 削除タスクを追加（後述） |

**Phase 3 タスク 3-7 の内容**:

```markdown
#### 3-7: `ast_produces_option` ヘルパー削除

**ファイル**: `src/transformer/expressions/mod.rs`

**現状**: `ast_produces_option` は `resolve_expr_type` が Cond/OptChain 式の型を `Option<T>` として返さないための AST レベルのワークアラウンド。以下のパターンを検出する:
- `OptChain(_)` — optional chaining は常に `Option` を生成
- `Cond(cond)` — 一方のブランチが null/undefined → `Option` を生成
- `Paren(p)` — 再帰的に内部式を検査

**Phase 3 での解消方法**:

3-1 で `resolve_expr_type` を `tctx.type_resolution.expr_type(span)` に置換する際、以下を確認する:

1. TypeResolver の `resolve_expr` が Cond 式に対し、一方のブランチが null/undefined の場合に `Option<T>` を expr_type として返すこと。現状の TypeResolver (`type_resolver.rs:768-775`) は Cond の両ブランチを resolve し non-Unknown を返すが、null ブランチの存在から `Option<T>` を推論するロジックがない。**TypeResolver 強化が必要**
2. TypeResolver の `resolve_expr` が OptChain 式に対し `Option<T>` を expr_type として返すこと。現状の OptChain 処理で unwrap 後の型が返されているなら、`Option<unwrapped_type>` を返すよう修正が必要

**タスク**:
1. TypeResolver の `resolve_expr` Cond 処理を強化: null/undefined ブランチ検出 → expr_type を `Option<T>` に設定
2. TypeResolver の `resolve_expr` OptChain 処理を確認: expr_type が `Option<T>` であることを保証
3. `ast_produces_option` と `is_null_or_undefined` ヘルパーを `mod.rs` から削除
4. Option wrapping 判定を `tctx.type_resolution.expr_type()` のみに依存するよう変更

**完了条件**:
- `ast_produces_option` 関数が存在しない
- `is_null_or_undefined` 関数が存在しない
- Option wrapping の二重ラップ防止が型解決のみで動作する
- `cargo test` 全 GREEN

**依存**: 3-1（resolve_expr_type 置換完了後）
```

#### 4-4: `plan.md`

引継ぎ事項の設計レビュー修正セクションを更新。Phase 2.5 完了条件が全て満たされた旨を記載。

---

## 検証計画

各タスク完了後、以下を順次実行:

1. **タスク 1 完了後**: `cargo check` — calls.rs のコンパイル確認
2. **タスク 2 完了後**: `cargo test -- test_call_with_option_arg_wraps_some test_call_with_missing_default_arg_appends_none` — 対象テストの GREEN 確認
3. **タスク 3 完了後**: `cargo check` — private 化後のコンパイル確認
4. **全タスク完了後**:
   - `cargo test` — 全テスト GREEN（unit 1115 + CLI 3 + compile 2 + E2E 60 + integration 69）
   - `cargo clippy --all-targets --all-features -- -D warnings` — 0 warnings
   - `cargo fmt --all --check` — 通過

## Phase 2.5 完了条件の最終検証

全タスク完了後、以下の完了条件を再検証する:

1. ✅ Transformer のプロダクションコードに `convert_expr_with_expected` の呼び出しが存在しない（mod.rs 内の内部再帰を除く）
2. ✅ `convert_expr_with_expected` が private 関数（Option unwrap 再帰専用）
3. ✅ TypeResolver の `propagate_expected` が全パターンをカバーしている
4. ✅ unit test が TypeResolver 経由で expected type を設定している
5. ✅ `cargo clippy` 0 警告
6. ✅ `cargo test` 全 GREEN
