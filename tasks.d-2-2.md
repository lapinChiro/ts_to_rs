# D-2-2: Transformer struct 導入 — 監査指摘対応

## 背景

D-2 完了後の監査で以下の 5 課題が検出された。いずれも D-2 の設計意図または完了条件に対する乖離。

## 課題一覧

### 課題 A: entry point のボイラープレート重複

3 つの public entry point が存在し、うち 2 つ（`transform_module`, `transform_module_collecting`）がダミー tctx 構築のボイラープレートを抱えている:

```rust
// transform_module, transform_module_collecting で重複
let mut synthetic = SyntheticTypeRegistry::new();
let mg = crate::pipeline::ModuleGraph::empty();
let resolution = crate::pipeline::type_resolution::FileTypeResolution::empty();
let tctx = context::TransformContext::new(&mg, reg, &resolution, std::path::Path::new(""));
```

現状:
- `transform_module(module, reg)` — 空 tctx 構築 + synthetic 内部管理。テスト 37 箇所から呼び出し
- `transform_module_with_context(module, ctx, synthetic)` — tctx 外部注入。テスト 3 箇所 + test_fixtures.rs から呼び出し
- `transform_module_collecting(module, reg)` — 空 tctx 構築 + collecting モード。テスト 1 箇所から呼び出し
- `pipeline/mod.rs` は `Transformer::for_module()` を直接使用（entry point を経由しない）

**問題**: ダミーコンテキスト構築の知識が entry point に漏れている。`Transformer::for_module()` でファクトリメソッドに集約した設計判断と同じ問題。

**今後の影響**: 不変だが、認識した時点で対処すべき（理想的でない状態を放置する理由がない）。

**対処方針**: ファクトリメソッドを追加し、entry point のボイラープレートを集約する。テスト用の簡易ファクトリ `Transformer::for_single_module(reg)` を導入し、entry point を簡素化する。`transform_module_with_context` は `Transformer::for_module()` の薄いラッパーのため、テストを直接 `Transformer::for_module()` 呼び出しに移行し、不要であれば削除を検討する。

### 課題 B: 4 つの free function がメソッド化されていない

設計セクション（line 90）で「tctx のみを取る変換関数 11 個」をメソッド化対象としているが、以下の 4 関数が free function のまま残存。

| # | 関数 | ファイル | 呼び出し元 |
|---|------|---------|-----------|
| 1 | `resolve_enum_type_name` | `patterns.rs:340` | Transformer メソッドのみ |
| 2 | `needs_trait_box_coercion` | `expressions/mod.rs:215` | Transformer メソッドのみ |
| 3 | `resolve_typeof_to_enum_variant` | `patterns.rs:728` | `NarrowingGuard::if_let_pattern` + Transformer メソッド (`try_convert_typeof_switch`) |
| 4 | `resolve_instanceof_to_enum_variant` | `patterns.rs:759` | `NarrowingGuard::if_let_pattern` のみ |

**判定**:
- #1, #2: 呼び出し元が全て Transformer メソッド → 単純にメソッド化すべき
- #3, #4: `NarrowingGuard::if_let_pattern` から呼ばれている → 課題 C の対処が前提

**今後の影響**: 不変。新たな呼び出し元は増えない。

### 課題 C: `NarrowingGuard::if_let_pattern` が `type_env` と `tctx` をパラメータで受け取っている

`NarrowingGuard` は Transformer とは別の型だが、`if_let_pattern` メソッドだけが `type_env: &TypeEnv` と `tctx: &TransformContext` をパラメータとして受け取っている。呼び出し元 3 箇所は全て Transformer メソッド内で `guard.if_let_pattern(&self.type_env, self.tctx)` としている。

他の NarrowingGuard メソッド（`var_name`, `narrowed_type_for_then`, `narrowed_type_for_else`）は tctx/type_env を取らない。

**分析**: `if_let_pattern` の本体は TypeRegistry を通じた enum バリアント解決（`resolve_typeof_to_enum_variant`, `resolve_instanceof_to_enum_variant`）を行っており、これは Transformer の責務。ロジックを Transformer メソッドに移動し、NarrowingGuard は純粋なデータ型として維持するのが理想。

**今後の影響**: **拡大**。今後の narrowing 拡張（Phase B-2〜B-4: 複合条件、三項演算子、switch typeof）で NarrowingGuard に新しいバリアントやメソッドが追加される場合、tctx/type_env パラメータ渡しパターンが伝播する。拡大する前に対処すべき。

**対処方針**: `if_let_pattern` のロジックを Transformer メソッド `resolve_if_let_pattern(&self, guard: &NarrowingGuard)` に移動。これにより:
1. `NarrowingGuard` から tctx/type_env パラメータが完全消滅
2. `resolve_typeof_to_enum_variant` と `resolve_instanceof_to_enum_variant` も Transformer メソッド化可能に（呼び出し元が全て Transformer になるため）
3. 今後の narrowing 拡張は `resolve_if_let_pattern` に新しい match arm を追加する形になり、パラメータ増殖を防止

### 課題 D: `let reg = self.reg()` 39 箇所が設計意図に反して残存

設計セクション（line 79）は明確に「`let reg = self.reg();` パターンは使わず、直接 `self.reg()` を呼ぶ」と規定。I-2 で「borrow checker 対策のため除去不可」と判断したが、これは誤り。

`self.reg()` は `&'a TypeRegistry` を返し、lifetime `'a` は `TransformContext` 由来であり `&self` 借用とは独立。NLL により `self.reg()` 呼び出し後の `&mut self` メソッド呼び出しと衝突しない。

**内訳**（調査結果）:

| グループ | 箇所数 | 説明 |
|---------|--------|------|
| A: 未使用 binding | 18 | `let reg = self.reg();` の後 `reg` が一度も使われない。削除のみ |
| B: 直接置換可能 | ~17 | `reg.xxx()` を `self.reg().xxx()` に置換。ただしクロージャ内で使用されるケースは `self.tctx.type_registry` に書き換え（`self` 全体のキャプチャを避けるため） |
| C: free function 内 | 4 | `let reg = tctx.type_registry;`。課題 B/C でメソッド化された後、`self.reg()` に置換 |

**今後の影響**: 不変〜縮小。新コードは設計規約に従い `self.reg()` を直接使用するため、今後増えない。

### 課題 E: Transformer フィールドが `pub(crate)` — カプセル化の不足

完了条件 6（「`pipeline/mod.rs` が Transformer のフィールド構造に直接依存していない」）は達成されているが、3 フィールドが `pub(crate)` のため crate 内どこからでも直接アクセス可能。型レベルでカプセル化が強制されていない。

**検証結果**: private 化（visibility 修飾子なし）は実現可能。
- `impl Transformer` ブロックは全て `transformer/` の子モジュール内にあり、private フィールドにアクセス可能
- テストファイル（`statements/tests.rs` 等）も `transformer/` の子モジュール内
- `pipeline/mod.rs` は `Transformer::for_module()` のみ使用、フィールド直接アクセスなし
- struct リテラル構築（サブ Transformer パターン）は全て `impl Transformer` ブロック内

**今後の影響**: 不変だが、private 化は今後のコードで `pub(crate)` を前提とした誤用を防止する防御的措置。

## 修正順序の設計

### 順序決定の根拠

| 課題 | 今後の影響 | 判断 |
|------|-----------|------|
| C (NarrowingGuard) | 拡大 | **最優先**。narrowing 拡張前に対処 |
| A (entry point) | 不変 | C と独立。認識時点で対処 |
| E (field visibility) | 不変（防御的） | struct 定義変更は早い方が良い |
| B (#1,#2: 2 関数メソッド化) | 不変 | C と独立。C の前でも後でも可 |
| D (let reg 除去) | 不変〜縮小 | **最後**。A/B/C で新メソッドが増えた後に一括実施 |

### 依存関係

```
A (entry point 簡素化) ────────────────┐
B (#1,#2 メソッド化) ─────────────────┤
C (NarrowingGuard → #3,#4 メソッド化) ┼─→ D (let reg 除去: 全メソッド一括)
E (field visibility) ─────────────────┘
```

A, B, C, E は相互に独立。D は全ての完了後に一括実施する（A/B/C で新しいメソッドが追加され、それらの `let reg` も D で一括処理するため）。

## 実装タスク

### Phase D-2-2-A: `resolve_enum_type_name` と `needs_trait_box_coercion` のメソッド化

課題 B の #1, #2。呼び出し元が全て Transformer メソッドなので単純なメソッド化。

- [x] **A-1**: `resolve_enum_type_name`（`patterns.rs:340`）を `impl Transformer` ブロックに移動。シグネチャから `tctx` を削除、`&self` を追加。本体の `tctx` → `self.tctx`。呼び出し元 2 箇所（`try_convert_enum_string_comparison` 内）を `self.resolve_enum_type_name()` に更新
- [x] **A-2**: `needs_trait_box_coercion`（`expressions/mod.rs:215`）を `impl Transformer` ブロックに移動。同様にメソッド化。呼び出し元 1 箇所（`convert_expr` 内）を `self.needs_trait_box_coercion()` に更新
- [x] **A-3**: `cargo check` 通過確認

### Phase D-2-2-B: NarrowingGuard リファクタリング

課題 C。`if_let_pattern` のロジックを Transformer メソッドに移動し、`resolve_typeof_to_enum_variant` と `resolve_instanceof_to_enum_variant` もメソッド化する。

- [x] **B-1**: `resolve_typeof_to_enum_variant`（`patterns.rs:728`）を `impl Transformer` ブロックに移動。シグネチャから `tctx` を削除、`&self` を追加。本体の `tctx.type_registry` → `self.reg()`
- [x] **B-2**: `resolve_instanceof_to_enum_variant`（`patterns.rs:759`）を同様にメソッド化
- [x] **B-3**: Transformer メソッド `resolve_if_let_pattern(&self, guard: &NarrowingGuard) -> Option<(String, bool)>` を作成。`NarrowingGuard::if_let_pattern` のロジックを移動。`self.type_env.get(guard.var_name())` と `self.resolve_typeof_to_enum_variant()` / `self.resolve_instanceof_to_enum_variant()` を直接使用
- [x] **B-4**: `NarrowingGuard::if_let_pattern` メソッドを削除
- [x] **B-5**: 呼び出し元 3 箇所を更新:
  - `expressions/mod.rs:168`: `guard.if_let_pattern(&self.type_env, self.tctx)` → `self.resolve_if_let_pattern(&guard)`
  - `statements/mod.rs:650`: 同上
  - `statements/mod.rs:659`: 同上
- [x] **B-6**: `statements/mod.rs:1864` の `resolve_typeof_to_enum_variant(...)` 呼び出しを `self.resolve_typeof_to_enum_variant(...)` に更新
- [x] **B-7**: 不要な `use` 文（`resolve_typeof_to_enum_variant` の import）を削除
- [x] **B-8**: `cargo check` 通過確認

### Phase D-2-2-C: Transformer フィールド private 化

課題 E。3 フィールドから `pub(crate)` を削除。

- [x] **C-1**: `Transformer` struct 定義の `pub(crate) tctx`, `pub(crate) type_env`, `pub(crate) synthetic` から `pub(crate)` を削除
- [x] **C-2**: `cargo check` 通過確認。子モジュール内の `impl Transformer` ブロックとテストファイルからは private フィールドにアクセス可能なため、コンパイルエラーは発生しないはず
- [x] **C-3**: エラーが発生した場合: `transformer/` モジュール外からのアクセスがないか確認し、アクセサメソッドの追加で対処

### Phase D-2-2-D: `let reg = self.reg()` / `let reg = tctx.type_registry` 除去

課題 D。Phase A/B でメソッド化された関数を含む全メソッドから binding を除去。

置換パターン:
- 通常: `reg.xxx()` → `self.reg().xxx()`
- クロージャ内で使用: `reg.xxx()` → `self.tctx.type_registry.xxx()`（`self` 全体のキャプチャ回避）
- 未使用 binding（18 箇所）: `let reg = self.reg();` を削除するのみ

- [x] **D-1**: GROUP A（未使用 binding 18 箇所）を削除。`cargo check` 通過確認
- [x] **D-2**: GROUP B（使用箇所あり ~17 箇所）を置換。各メソッドで `reg` の全使用箇所を `self.reg()` または `self.tctx.type_registry` に書き換え。クロージャ内使用の有無を確認して適切なパターンを選択
- [x] **D-3**: `cargo check` 通過確認
- [x] **D-4**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [x] **D-5**: `cargo fmt --all --check` 通過
- [x] **D-6**: `cargo test` 全 GREEN

### Phase D-2-2-E: entry point 簡素化

課題 A。ダミーコンテキスト構築のボイラープレートをファクトリメソッドに集約。

- [x] **E-1**: `Transformer::for_single_module(reg)` ファクトリメソッドを追加。内部で空の ModuleGraph / FileTypeResolution / file_path を構築し、`for_module()` 相当の Transformer を返す。synthetic も内部で作成し所有する設計を検討（`transform_module` は synthetic を外部に返す必要があるため、返り値に含めるか、別の方法を検討）
- [x] **E-2**: `transform_module(module, reg)` を新ファクトリ経由に書き換え
- [x] **E-3**: `transform_module_collecting(module, reg)` を新ファクトリ経由に書き換え
- [x] **E-4**: `transform_module_with_context` の呼び出し元を `Transformer::for_module()` 直接呼び出しに移行可能か検討。可能であれば移行し、関数を削除
- [x] **E-5**: `cargo check` 通過確認
- [x] **E-6**: `cargo test` 全 GREEN

## 完了条件

1. `resolve_enum_type_name`, `needs_trait_box_coercion`, `resolve_typeof_to_enum_variant`, `resolve_instanceof_to_enum_variant` が Transformer メソッドになっている
2. `NarrowingGuard::if_let_pattern` が削除され、ロジックが `Transformer::resolve_if_let_pattern` に移動している
3. `NarrowingGuard` のメソッドに `tctx` / `type_env` パラメータが存在しない
4. Transformer フィールドが private（`pub(crate)` なし）
5. `let reg = self.reg();` / `let reg = tctx.type_registry;` が Transformer メソッド内に存在しない
6. entry point のダミーコンテキスト構築ボイラープレートがファクトリメソッドに集約されている
7. `cargo test` 全 GREEN
8. `cargo clippy` 0 エラー・0 警告
