# D-2-2 監査指摘対応 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** D-2 完了後の監査で検出された 5 課題（free function メソッド化、NarrowingGuard リファクタリング、フィールド private 化、`let reg` 除去、entry point 簡素化）を全て修正する。

**Architecture:** 5 課題のうち A, B, C, E は相互に独立。D は A/B/C/E 完了後に一括実施（新メソッド分も含めて一括処理するため）。各課題はそれぞれ 1 コミットとし、段階的にコミットする。

**Tech Stack:** Rust, cargo check/test/clippy/fmt

---

## 依存関係

```
A (2関数メソッド化) ──────────┐
B (NarrowingGuard リファクタ) ┼─→ D (let reg 除去: 全メソッド一括)
C (フィールド private 化) ────┤
E (entry point 簡素化) ───────┘
```

A, B, C, E は任意の順序で実施可能。D は最後。

---

### Task 1: Phase A — `resolve_enum_type_name` と `needs_trait_box_coercion` のメソッド化

**Files:**
- Modify: `src/transformer/expressions/patterns.rs:340-351` — `resolve_enum_type_name` をメソッド化
- Modify: `src/transformer/expressions/patterns.rs:62,77` — 呼び出し元を `self.resolve_enum_type_name()` に更新
- Modify: `src/transformer/expressions/mod.rs:215-263` — `needs_trait_box_coercion` をメソッド化
- Modify: `src/transformer/expressions/mod.rs:134` — 呼び出し元を `self.needs_trait_box_coercion()` に更新

#### A-1: `resolve_enum_type_name` メソッド化

- [ ] **Step 1: free function をメソッドに変換**

`src/transformer/expressions/patterns.rs:340-351` の `resolve_enum_type_name` を変更:
- シグネチャ: `fn resolve_enum_type_name(expr, tctx)` → `fn resolve_enum_type_name(&self, expr: &ast::Expr) -> Option<String>`
- `impl Transformer` ブロック内に配置（同ファイル内の既存 `impl Transformer` に追加）
- 本体: `tctx` → `self.tctx`、`let reg = tctx.type_registry` → `let reg = self.reg()`（後で D で除去）

```rust
/// 式の型が string literal union enum の場合、その enum 名を返す。
fn resolve_enum_type_name(&self, expr: &ast::Expr) -> Option<String> {
    let reg = self.reg();
    let ty = get_expr_type(self.tctx, expr)?;
    if let RustType::Named { name, .. } = ty {
        if let Some(TypeDef::Enum { string_values, .. }) = reg.get(name) {
            if !string_values.is_empty() {
                return Some(name.clone());
            }
        }
    }
    None
}
```

- [ ] **Step 2: 呼び出し元を更新**

`src/transformer/expressions/patterns.rs` の 2 箇所:
- L62: `resolve_enum_type_name(&bin.left, self.tctx)` → `self.resolve_enum_type_name(&bin.left)`
- L77: `resolve_enum_type_name(&bin.right, self.tctx)` → `self.resolve_enum_type_name(&bin.right)`

- [ ] **Step 3: `cargo check` 通過確認**

Run: `cargo check 2>&1 | tail -5`
Expected: `Finished` (エラーなし)

#### A-2: `needs_trait_box_coercion` メソッド化

- [ ] **Step 4: free function をメソッドに変換**

`src/transformer/expressions/mod.rs:215-263` の `needs_trait_box_coercion` を変更:
- シグネチャ: `fn needs_trait_box_coercion(expected, src_expr, tctx)` → `fn needs_trait_box_coercion(&self, expected: &RustType, src_expr: &ast::Expr) -> bool`
- `impl Transformer` ブロック内に配置
- 本体: `tctx` → `self.tctx`、`let reg = tctx.type_registry` → `let reg = self.reg()`

```rust
/// Returns true when the expected type is a trait type (`Box<dyn Trait>`)
/// and the source expression produces a concrete (non-Box) value that needs wrapping.
fn needs_trait_box_coercion(&self, expected: &RustType, src_expr: &ast::Expr) -> bool {
    let reg = self.reg();
    let trait_name = match expected {
        RustType::Named { name, type_args }
            if name == "Box"
                && type_args.len() == 1
                && matches!(&type_args[0], RustType::DynTrait(_)) =>
        {
            if let RustType::DynTrait(t) = &type_args[0] {
                t.as_str()
            } else {
                return false;
            }
        }
        RustType::Named { name, .. } if reg.is_trait_type(name) => name.as_str(),
        _ => return false,
    };

    let Some(expr_type) = type_resolution::get_expr_type(self.tctx, src_expr) else {
        return false;
    };
    if matches!(expr_type, RustType::Any) {
        return false;
    }

    if matches!(
        expr_type,
        RustType::Named { name, type_args }
            if name == "Box" && type_args.first().is_some_and(|a| matches!(a, RustType::DynTrait(t) if t == trait_name))
    ) {
        return false;
    }

    if let RustType::Named {
        name: expr_name,
        type_args: expr_args,
    } = expr_type
    {
        if expr_args.is_empty() && expr_name == trait_name && reg.is_trait_type(expr_name) {
            return false;
        }
    }

    true
}
```

- [ ] **Step 5: 呼び出し元を更新**

`src/transformer/expressions/mod.rs:134`:
`needs_trait_box_coercion(expected, expr, self.tctx)` → `self.needs_trait_box_coercion(expected, expr)`

- [ ] **Step 6: `cargo check` 通過確認**

Run: `cargo check 2>&1 | tail -5`
Expected: `Finished`

- [ ] **Step 7: コミット**

`tasks.d-2-2.md` の Phase A タスクにチェックを入れ、`plan.md` を最新化してからコミット提案。
メッセージ: `[WIP] P8: D-2-2-A — resolve_enum_type_name, needs_trait_box_coercion メソッド化`

---

### Task 2: Phase B — NarrowingGuard リファクタリング

**Files:**
- Modify: `src/transformer/expressions/patterns.rs:728-779` — `resolve_typeof_to_enum_variant`, `resolve_instanceof_to_enum_variant` メソッド化
- Modify: `src/transformer/expressions/patterns.rs:532-572` — `if_let_pattern` のロジックを Transformer に移動、元メソッド削除
- Modify: `src/transformer/expressions/mod.rs:168` — 呼び出し元更新
- Modify: `src/transformer/statements/mod.rs:650,659,1802,1864` — 呼び出し元更新

#### B-1: `resolve_typeof_to_enum_variant` メソッド化

- [ ] **Step 1: free function をメソッドに変換**

`src/transformer/expressions/patterns.rs:728-756` を `impl Transformer` ブロックに移動:
- シグネチャ: `pub(crate) fn resolve_typeof_to_enum_variant(var_type, typeof_str, tctx)` → `fn resolve_typeof_to_enum_variant(&self, var_type: &RustType, typeof_str: &str) -> Option<(String, String)>`
- 本体: `let reg = tctx.type_registry` → `let reg = self.reg()`

#### B-2: `resolve_instanceof_to_enum_variant` メソッド化

- [ ] **Step 3: free function をメソッドに変換**

`src/transformer/expressions/patterns.rs:759-779` を同様にメソッド化:
- シグネチャ: `pub(crate) fn resolve_instanceof_to_enum_variant(var_type, class_name, tctx)` → `fn resolve_instanceof_to_enum_variant(&self, var_type: &RustType, class_name: &str) -> Option<(String, String)>`

#### B-3: `resolve_if_let_pattern` メソッド作成

- [ ] **Step 4: Transformer メソッドとして新規作成**

`src/transformer/expressions/patterns.rs` の `impl Transformer` ブロックに追加:

```rust
/// NarrowingGuard から if-let パターン文字列を解決する。
///
/// Returns `Some((pattern, is_swap))` where `is_swap` is true for `!==`/`!=` guards
/// (meaning then/else branches should be swapped).
fn resolve_if_let_pattern(
    &self,
    guard: &NarrowingGuard,
) -> Option<(String, bool)> {
    let var_type = self.type_env.get(guard.var_name())?;
    match guard {
        NarrowingGuard::NonNullish { is_neq, .. } => {
            if matches!(var_type, RustType::Option(_)) {
                Some((format!("Some({})", guard.var_name()), !is_neq))
            } else {
                None
            }
        }
        NarrowingGuard::Truthy { .. } => {
            if matches!(var_type, RustType::Option(_)) {
                Some((format!("Some({})", guard.var_name()), false))
            } else {
                None
            }
        }
        NarrowingGuard::Typeof {
            type_name, is_eq, ..
        } => {
            let (enum_name, variant) =
                self.resolve_typeof_to_enum_variant(var_type, type_name)?;
            Some((
                format!("{enum_name}::{variant}({})", guard.var_name()),
                !is_eq,
            ))
        }
        NarrowingGuard::InstanceOf { class_name, .. } => {
            let (enum_name, variant) =
                self.resolve_instanceof_to_enum_variant(var_type, class_name)?;
            Some((
                format!("{enum_name}::{variant}({})", guard.var_name()),
                false,
            ))
        }
    }
}
```

#### B-4: `NarrowingGuard::if_let_pattern` 削除 + 呼び出し元更新

- [ ] **Step 5: `NarrowingGuard::if_let_pattern` メソッドを削除**

`src/transformer/expressions/patterns.rs:527-572` の `if_let_pattern` メソッドを丸ごと削除。

- [ ] **Step 6: 呼び出し元 3 箇所を更新**

| ファイル | 行 | 変更前 | 変更後 |
|---------|-----|--------|--------|
| `expressions/mod.rs` | 168 | `guard.if_let_pattern(&self.type_env, self.tctx)` | `self.resolve_if_let_pattern(&guard)` |
| `statements/mod.rs` | 650 | `guard.if_let_pattern(&self.type_env, self.tctx).is_some()` | `self.resolve_if_let_pattern(guard).is_some()` |
| `statements/mod.rs` | 659 | `guard.if_let_pattern(&self.type_env, self.tctx).unwrap()` | `self.resolve_if_let_pattern(guard).unwrap()` |

**注意**: L650, L659 では `guard` は参照 `&NarrowingGuard` で渡されている。L168 では `guard` はローカル変数（所有）なので `&guard` が必要。

- [ ] **Step 7: `try_convert_typeof_switch` 内の呼び出し更新**

`src/transformer/statements/mod.rs:1802,1864`:
- L1802: `use crate::transformer::expressions::patterns::resolve_typeof_to_enum_variant;` を削除
- L1864: `resolve_typeof_to_enum_variant(&var_type, &typeof_str, self.tctx)` → `self.resolve_typeof_to_enum_variant(&var_type, &typeof_str)`

- [ ] **Step 8: 不要な `pub(crate)` 可視性を削除**

`resolve_typeof_to_enum_variant` と `resolve_instanceof_to_enum_variant` は Transformer メソッド内からのみ呼ばれるため、`pub(crate)` は不要。メソッド化時に可視性を `fn`（private）にする。

- [ ] **Step 9: `cargo check` 通過確認**

Run: `cargo check 2>&1 | tail -5`
Expected: `Finished`

- [ ] **Step 10: コミット**

`tasks.d-2-2.md` の Phase B タスクにチェックを入れ、`plan.md` を最新化してからコミット提案。
メッセージ: `[WIP] P8: D-2-2-B — NarrowingGuard if_let_pattern を Transformer メソッドに移動`

---

### Task 3: Phase C — Transformer フィールド private 化

**Files:**
- Modify: `src/transformer/mod.rs:36-43` — `pub(crate)` を削除

- [ ] **Step 1: 3 フィールドから `pub(crate)` を削除**

`src/transformer/mod.rs:36-43`:

```rust
pub(crate) struct Transformer<'a> {
    /// 不変コンテキスト（TypeRegistry, ModuleGraph, TypeResolution, file path）
    tctx: &'a TransformContext<'a>,
    /// ローカル変数の型追跡（可変 — ブロックスコープで push_scope / pop_scope）
    type_env: TypeEnv,
    /// 合成型レジストリ（可変 — 変換中に型が追加される）
    synthetic: &'a mut SyntheticTypeRegistry,
}
```

- [ ] **Step 2: `cargo check` 通過確認**

Run: `cargo check 2>&1 | tail -5`
Expected: `Finished` — 全アクセスが `transformer/` サブモジュールの `impl Transformer` 内なので private でもアクセス可能。

- [ ] **Step 3: エラー発生時の対処**

万が一 `transformer/` 外からのアクセスがある場合、アクセサメソッドを追加して対処。ただし事前調査で外部アクセスは確認されていない。

- [ ] **Step 4: コミット**

メッセージ: `[WIP] P8: D-2-2-C — Transformer フィールド private 化`

---

### Task 4: Phase E — entry point 簡素化

**Files:**
- Modify: `src/transformer/mod.rs:45-71` — `for_single_module` ファクトリメソッド追加
- Modify: `src/transformer/mod.rs:135-184` — `transform_module`, `transform_module_collecting` を新ファクトリ経由に書き換え
- Modify: `src/transformer/mod.rs:150-157` — `transform_module_with_context` の必要性検討

#### E-1: `for_single_module` ファクトリメソッド

- [ ] **Step 1: ファクトリメソッドを追加**

`src/transformer/mod.rs` の `impl Transformer` ブロック内に追加:

```rust
/// 単一モジュール変換用の Transformer を構築する。
///
/// テスト用および `transform_module` / `transform_module_collecting` entry point 用。
/// 空の ModuleGraph, FileTypeResolution, file_path で TransformContext を構築するため、
/// マルチモジュール機能（import 解決、型伝搬）は利用できない。
///
/// # Lifetime
///
/// 返り値の `TransformContext` は内部で構築されるため、ライフタイムの都合で
/// Transformer を直接返すことはできない。代わりにクロージャで処理を渡す。
fn with_single_module<R>(
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
    f: impl FnOnce(&mut Transformer<'_>) -> R,
) -> R {
    let mg = crate::pipeline::ModuleGraph::empty();
    let resolution = crate::pipeline::type_resolution::FileTypeResolution::empty();
    let tctx = context::TransformContext::new(&mg, reg, &resolution, std::path::Path::new(""));
    let mut t = Transformer::for_module(&tctx, synthetic);
    f(&mut t)
}
```

**設計判断**:
- `TransformContext` のライフタイムが `mg`, `resolution` に依存するため、Transformer を直接返すファクトリは不可。クロージャパターンで `mg`, `resolution` のライフタイムを関数スコープに閉じ込める。
- `synthetic` はパラメータとして残す（内部作成不可）。理由: `transform_module` は `synthetic.into_items()` をクロージャ完了後に呼び出す必要があり、クロージャ内で作成・消費すると呼び出し元に返せない。

- [ ] **Step 2: `transform_module` を書き換え**

```rust
pub fn transform_module(module: &Module, reg: &TypeRegistry) -> Result<Vec<Item>> {
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut items = Transformer::with_single_module(reg, &mut synthetic, |t| {
        t.transform_module(module)
    })?;
    let mut all = synthetic.into_items();
    all.append(&mut items);
    Ok(all)
}
```

- [ ] **Step 3: `transform_module_collecting` を書き換え**

```rust
pub fn transform_module_collecting(
    module: &Module,
    reg: &TypeRegistry,
) -> Result<(Vec<Item>, Vec<UnsupportedSyntaxError>)> {
    let mut synthetic = SyntheticTypeRegistry::new();
    let (mut items, unsupported) = Transformer::with_single_module(reg, &mut synthetic, |t| {
        t.transform_module_collecting(module)
    })?;
    let mut all = synthetic.into_items();
    all.append(&mut items);
    Ok((all, unsupported))
}
```

- [ ] **Step 4: `transform_module_with_context` の必要性検討**

`transform_module_with_context` は `Transformer::for_module()` の薄いラッパー。呼び出し元を確認:
- テスト 3 箇所 + `test_fixtures.rs` — これらは `Transformer::for_module()` を直接使えるが、テストの移行コストと破壊リスクを考慮し、**このフェーズでは残す**（テストヘルパーとしての役割があり、削除のメリットが小さい）。

- [ ] **Step 5: `cargo check` 通過確認**

Run: `cargo check 2>&1 | tail -5`
Expected: `Finished`

- [ ] **Step 6: `cargo test` 全 GREEN 確認**

Run: `cargo test > /tmp/test-result.txt 2>&1 && grep -E "test result:" /tmp/test-result.txt`
Expected: `test result: ok`

- [ ] **Step 7: コミット**

メッセージ: `[WIP] P8: D-2-2-E — entry point 簡素化 (with_single_module ファクトリ導入)`

---

### Task 5: Phase D — `let reg = self.reg()` / `let reg = tctx.type_registry` 全除去

**Files:**
- Modify: 全 `src/transformer/**/*.rs` ファイル（`let reg = self.reg()` がある全メソッド）

**前提**: Task 1-4 が全て完了していること。

#### D-1: 全箇所の洗い出し

- [ ] **Step 1: `let reg = self.reg();` の全箇所を grep で列挙**

Run: `grep -rn "let reg = self\.reg()" src/transformer/`
Expected: 全箇所のリストが得られる。Task 1-4 で新しく追加されたメソッド内の `let reg = self.reg()` も含まれる。

- [ ] **Step 2: 各箇所を分類**

各箇所を以下の 3 グループに分類:
- **GROUP A（未使用）**: `let reg = self.reg();` の後で `reg` が一度も使われない → 行を削除するのみ
- **GROUP B（通常使用）**: クロージャ外で `reg.xxx()` を使用 → `self.reg().xxx()` に置換
- **GROUP C（クロージャ内使用）**: クロージャ内で `reg` を使用 → `self.tctx.type_registry.xxx()` に置換（`self` 全体のキャプチャ回避）

#### D-2: GROUP A（未使用 binding）除去

- [ ] **Step 3: 未使用 binding を削除**

各ファイルで `let reg = self.reg();` 行を削除。clippy の `unused_variables` 警告が出ていないか（`_` prefix なし）を確認。

- [ ] **Step 4: `cargo check` 通過確認**

#### D-3: GROUP B + C（使用箇所あり）置換

- [ ] **Step 5: 各メソッドで `reg` → `self.reg()` または `self.tctx.type_registry` に置換**

**置換ルール**:
1. クロージャ（`|...|` / `move |...|`）の**外**で `reg.xxx()` → `self.reg().xxx()`
2. クロージャの**中**で `reg.xxx()` → `self.tctx.type_registry.xxx()`
3. `reg` を関数の引数として渡している場合（`some_fn(reg, ...)`） → `self.reg()` を渡す

各メソッドは手動で確認し、クロージャ有無を判定してから置換する。

- [ ] **Step 6: `cargo check` 通過確認**

Run: `cargo check 2>&1 | tail -5`
Expected: `Finished`

#### D-4: 品質確認

- [ ] **Step 7: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告**
- [ ] **Step 8: `cargo fmt --all --check` 通過**
- [ ] **Step 9: `cargo test` 全 GREEN**

Run: `cargo test > /tmp/test-result.txt 2>&1 && grep -E "test result:" /tmp/test-result.txt`
Expected: `test result: ok`

- [ ] **Step 10: `let reg = self.reg()` が完全に除去されていることを確認**

Run: `grep -rn "let reg = self\.reg()" src/transformer/`
Expected: 0 件

- [ ] **Step 11: コミット**

メッセージ: `[WIP] P8: D-2-2-D — let reg = self.reg() 全箇所除去`

---

### Task 6: 最終検証 + ドキュメント更新

- [ ] **Step 1: 全完了条件の検証**

| # | 条件 | 検証コマンド |
|---|------|-------------|
| 1 | 4 関数がメソッドになっている | `grep -n "fn resolve_enum_type_name\|fn needs_trait_box_coercion\|fn resolve_typeof_to_enum_variant\|fn resolve_instanceof_to_enum_variant" src/transformer/expressions/` で `&self` が含まれること |
| 2 | `NarrowingGuard::if_let_pattern` が削除 | `grep -n "fn if_let_pattern" src/transformer/` が 0 件 |
| 3 | NarrowingGuard に tctx/type_env パラメータなし | `grep -n "type_env\|tctx" src/transformer/expressions/patterns.rs` の NarrowingGuard impl ブロック内を確認 |
| 4 | フィールドが private | `grep "pub(crate)" src/transformer/mod.rs` で `tctx`, `type_env`, `synthetic` が含まれないこと |
| 5a | `let reg = self.reg()` が存在しない | `grep -rn "let reg = self\.reg()" src/transformer/` が 0 件 |
| 5b | `let reg = tctx.type_registry` が存在しない | `grep -rn "let reg = tctx\.type_registry" src/transformer/` が 0 件 |
| 6 | entry point ボイラープレートが集約 | `grep -A3 "fn transform_module\b" src/transformer/mod.rs` で `ModuleGraph::empty()` が entry point 関数本体にないこと |
| 7 | `cargo test` 全 GREEN | `cargo test` |
| 8 | `cargo clippy` 0 エラー・0 警告 | `cargo clippy --all-targets --all-features -- -D warnings` |

- [ ] **Step 2: `tasks.d-2-2.md` の全タスクにチェックを入れる**
- [ ] **Step 3: `plan.md` を更新 — D-2-2 完了を記載**
- [ ] **Step 4: コミット**

メッセージ: `P8: D-2-2 完了 — 監査指摘 5 課題全修正`
