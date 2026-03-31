# I-319 + I-343: Vec インデックスアクセスの安全化

## Background

TypeScript の `arr[0]` は空配列で `undefined` を返すが、現在の変換は Rust `arr[0]` を生成し境界外で panic する。これは S1（サイレント意味変更）— コードはコンパイルされるが実行時の挙動が TS と異なる最も危険なバグカテゴリ。

同一コードパス上の I-343（`Vec<Struct>` インデックスで非 Copy 型の move エラー）も、`.get().cloned()` パターンにより自動解消される。

### 現状のコード生成

```typescript
arr[0]        // TS: undefined on empty
arr[i]        // TS: undefined on out-of-bounds
const [a, b] = arr;  // TS: undefined for missing elements
```

```rust
// 現在の生成コード — 全て境界外 panic
arr[0]
arr[i as usize]
let a = arr[0];
let b = arr[1];
```

### 既存の安全パターン

optional chaining（`arr?.[0]`）は既に安全な `.get().cloned()` パターンを使用している（`member_access.rs:113-127`）。この実績あるパターンを通常のインデックスアクセスにも適用する。

## Goal

1. Vec のインデックス読み取りアクセスを `.get(idx).cloned()` に変換し、**S1（panic）を排除**する
2. 結果型を `Option<T>` にすることで、値の不在可能性を Rust の型システムで表現する
3. 代入ターゲット（`arr[0] = value`）は直接インデックスを維持する
4. I-343（Vec\<Struct\> move エラー）を副次的に解消する

**定量目標**:
- S1 バグ: 2 → 1（I-319 解消、I-298 残存）
- Hono クリーン率: 変化なし（新規 conversion error なし）
- コンパイル率: 一部低下の可能性あり（`Option<T>` 型不一致による compile error は Tier 2 であり S1 より安全）

## Scope

### In Scope

- `convert_member_expr` の Computed 分岐で、読み取りアクセスを `.get(idx).cloned()` に変換
- 代入ターゲットの分離（`convert_member_expr_for_write`）
- 配列デストラクチャリングの安全化
- `convert_index_to_usize` の可視性変更（`pub(crate)`）
- 関連するユニットテスト、スナップショットテスト、E2E テストの更新

### Out of Scope

- HashMap インデックスアクセスの修正（I-310 — 別の根本原因）
- Range インデックス（slice/substring — 安全にトランケートされる）
- Tuple インデックス（`.0` フィールドアクセスに変換済み — コンパイル時安全）
- TypeResolver の `resolve_member_type` 変更（代入 LHS の expected type 伝播に影響するため変更しない）
- `Option<T>` の自動 unwrap（downstream で必要になるが別スコープ）

## Design

### Technical Approach

#### 1. Transformer: read/write 分離

`convert_member_expr` に内部メソッド `convert_member_expr_inner(member, for_write: bool)` を導入。

- `convert_member_expr(member)` → `inner(member, false)` — 読み取り用
- `convert_member_expr_for_write(member)` → `inner(member, true)` — 代入ターゲット用

Computed 分岐の新ロジック:

```rust
// for_write=false の場合: 安全インデックス
// Tuple と Range は除外（既に安全）
let index = self.convert_expr(&computed.expr)?;
let safe_index = convert_index_to_usize(index);
return Ok(Expr::MethodCall {
    object: Box::new(Expr::MethodCall {
        object: Box::new(object),
        method: "get".to_string(),
        args: vec![safe_index],
    }),
    method: "cloned".to_string(),
    args: vec![],
});
```

**型不明の場合も安全側に倒す**（ユーザー判断）。`.get()` メソッドがない型ではコンパイルエラーになるが、panic（S1）より安全（Tier 2）。

#### 2. 代入ターゲット

`assignments.rs:14` で `convert_member_expr_for_write` を使用。代入ターゲットは常に直接インデックス `arr[idx]` を維持。

#### 3. 配列デストラクチャリング

`destructuring.rs:228-236` で `.get(i).cloned()` を生成。`convert_index_to_usize` を `pub(crate)` に変更して再利用。

Rest 要素（`[first, ...rest]`）は Range インデックスのため変更不要。

#### 4. TypeResolver は変更しない

`resolve_member_type`（`expressions.rs:576`）は代入 LHS の expected type 伝播にも使われる（`expressions.rs:88`）。`Option<T>` に変更すると `arr[0] = 42` の RHS に `Option<f64>` が伝播し、誤った型変換を引き起こす。

型の変化は Transformer の IR 生成で表現し、TypeResolver は `T` を維持する。

#### 5. 共通ヘルパー `build_safe_index_expr`

`.get(idx).cloned()` の `Expr::MethodCall` 構築を module-level `pub(crate) fn build_safe_index_expr(object: Expr, index: Expr) -> Expr` として `member_access.rs` に配置する。`convert_index_to_usize` と同じ抽象レベルの pure function。

呼び出し元:
- `convert_member_expr_inner` — 読み取りインデックス
- `convert_opt_chain_expr` — optional chaining の computed access（既存コード 113-127 行をリファクタ）
- `destructuring.rs` — 配列デストラクチャリング

新 IR ノードは不要。既存の `Expr::MethodCall` 入れ子パターンを使用。

#### 6. 複合代入 (`+=`, `-=` 等) の扱い

`arr[0] += 1` は `convert_assign_expr` 経由で処理される。LHS の `arr[0]` は `convert_member_expr_for_write` で直接インデックスを生成。compound desugar (`target = target + value`) で `target.clone()` が RHS にも使われるため、読み書き両方が直接インデックス。これは複合代入の意図的なセマンティクス — プログラマが明示的にインデックスを指定しているため、直接アクセスが正しい。

### Design Integrity Review

- **Higher-level consistency**: 変換パイプライン（Parser → Transformer → Generator）の依存方向は維持。IR に新バリアントを追加せず、既存の `Expr::MethodCall` で表現
- **DRY**: `.get().cloned()` パターンは `build_safe_index_expr` ヘルパーに集約し、3 箇所（`convert_member_expr_inner`, `convert_opt_chain_expr`, `destructuring.rs`）から共有。`convert_index_to_usize` は既存ヘルパーを `pub(crate)` 化して再利用
- **Orthogonality**: read/write の責務分離を `for_write` パラメータで明確化。代入ターゲットの変換は `convert_member_expr_for_write` に分離
- **Coupling**: TypeResolver を変更しないことで、型解決と IR 生成の結合度増加を回避。`build_safe_index_expr` は `self` に依存しない pure function で、モジュール間結合を増やさない
- **Broken windows**: Impact Area Code Review で以下を発見
  - **Tuple デストラクチャリング**: `try_convert_array_destructuring` が Tuple 型でも `Expr::Index`（`tuple[0]`）を生成するが、Rust の Tuple は `tuple.0` 構文。本 PRD のスコープ外 — TODO に記録（既存のコンパイルエラーで検出可能、S1 ではない）

### Impact Area

| ファイル | 変更内容 |
|----------|----------|
| `src/transformer/expressions/member_access.rs` | read/write 分離 + Vec safe indexing |
| `src/transformer/expressions/assignments.rs` | `convert_member_expr_for_write` 使用 |
| `src/transformer/statements/destructuring.rs` | デストラクチャリング safe indexing |

### Semantic Safety Analysis

本 PRD は型解決ロジック自体は変更しない（TypeResolver 変更なし）。生成コードの型が `T` → `Option<T>` に変化する。

**影響パターン分析**:

| パターン | 変換前 | 変換後 | 分類 |
|----------|--------|--------|------|
| `arr[0]` 単独式 | `T` (panic リスク) | `Option<T>` (安全) | **Safe**: compile error or identical |
| `arr[0]` 関数引数 | `T` | `Option<T>` — 型不一致 | **Safe**: compile error |
| `arr[0]` 算術演算 | `T` | `Option<T>` — 演算不可 | **Safe**: compile error |
| `arr[0] = value` | `T` (代入) | `T` (変更なし) | **Safe**: 直接インデックス維持 |
| `const [a, b] = arr` | `T` (panic リスク) | `Option<T>` (安全) | **Safe**: compile error or identical |

**UNSAFE パターン**: なし。全パターンが compile error（Tier 2）または identical behavior に分類。S1（panic）は排除される。

### Impact Area Code Review

#### Production Code Issues

| Issue | Location | Category | Severity | Action |
|-------|----------|----------|----------|--------|
| P1 | `member_access.rs:113-127` と新コードで `.get().cloned()` パターン重複 | DRY | Low | ヘルパー関数抽出で DRY 化（T1 で対応） |
| P2 | `convert_index_to_usize` が private | Coupling | Low | `pub(crate)` に変更（T1 で対応） |
| P3 | `try_convert_array_destructuring` が Tuple 型でも `Expr::Index` を生成（Rust Tuple は `.0` 構文） | Correctness | Medium | TODO 記録（本 PRD スコープ外 — コンパイルエラーで検出可能） |

#### Test Coverage Gaps

| Gap | Missing Pattern | Technique | Severity | Action |
|-----|----------------|-----------|----------|--------|
| G1 | Vec 型の computed access テスト（型情報あり） | Equivalence Partition | High | T3 で追加 |
| G2 | 代入ターゲットの computed access テスト | C1 Branch | High | T3 で追加 |
| G3 | `process.env.VAR` パターンのテスト | C1 Branch | Medium | T3 で追加 |
| G4 | `.length` → `.len() as f64` 変換のテスト | C1 Branch | Medium | T3 で追加 |
| G5 | `convert_opt_chain_expr` の computed access テスト | Equivalence Partition | Medium | T3 で追加 |
| G6 | 配列デストラクチャリング（空配列、型情報あり） | Boundary Value | Medium | T4 で追加 |

## Task List

### T1: Transformer — read/write 分離 + 安全インデックス生成

- **Work**:
  1. `member_access.rs`: `convert_index_to_usize` を `pub(crate)` に変更
  2. `member_access.rs`: `convert_member_expr_inner(member, for_write: bool)` を追加。既存 `convert_member_expr` は `inner(member, false)` を呼ぶラッパーに
  3. `member_access.rs`: `pub(crate) fn convert_member_expr_for_write` を追加（`inner(member, true)` を呼ぶ）
  4. `convert_member_expr_inner` の Computed 分岐で、`!for_write` かつ非 Range インデックスの場合、`.get(idx).cloned()` を生成。Tuple チェックは既存のまま維持（先に分岐するため影響なし）
  5. DRY 化: `.get(idx).cloned()` の `Expr::MethodCall` 構築を module-level `pub(crate) fn build_safe_index_expr(object: Expr, index: Expr) -> Expr` として `member_access.rs` に配置。引数 `index` は `convert_index_to_usize` 適用済みを前提。`convert_opt_chain_expr` の Computed 分岐（113-127行）もこのヘルパーを使用するようリファクタ
- **Completion criteria**:
  - `convert_member_expr` が読み取り時に `.get().cloned()` の `Expr::MethodCall` を生成
  - `convert_member_expr_for_write` が常に `Expr::Index` を生成
  - `convert_opt_chain_expr` が `build_safe_index_expr` を使用
  - `cargo check` 通過
- **Depends on**: None

### T2: 代入ターゲット修正

- **Work**:
  1. `assignments.rs:14`: `self.convert_member_expr(member)?` → `self.convert_member_expr_for_write(member)?` に変更
- **Completion criteria**:
  - 代入式の LHS が常に `Expr::Index`（直接インデックス）を生成
  - `cargo check` 通過
- **Depends on**: T1

### T3: ユニットテスト — member_access + assignments

- **Work**:
  1. `tests/member_access.rs`: 既存テスト `test_convert_member_expr_array_index_literal_generates_index` を更新 — 型情報なしで `.get().cloned()` MethodCall を期待
  2. `tests/member_access.rs`: 既存テスト `test_convert_member_expr_array_index_variable_generates_index` を同様に更新
  3. `tests/member_access.rs`: 既存テスト `test_convert_member_expr_non_tuple_index_unchanged` を同様に更新
  4. 新規テスト追加:
     - `test_convert_member_expr_vec_literal_index_generates_safe_get` — Vec 型の literal index
     - `test_convert_member_expr_vec_variable_index_generates_safe_get` — Vec 型の variable index
     - `test_convert_member_expr_for_write_keeps_direct_index` — 代入ターゲットが `Expr::Index` を維持（G2）
     - `test_convert_member_expr_range_index_unchanged` — Range インデックスが直接のまま
     - `test_resolve_member_access_length_generates_len_cast` — `.length` → `.len() as f64`（G4）
     - `test_convert_member_expr_process_env_var` — `process.env.VAR` パターン（G3）
     - `test_convert_opt_chain_computed_uses_safe_index_helper` — optional chaining の computed access が `build_safe_index_expr` を使用（G5）
- **Completion criteria**:
  - 全テスト通過
  - C1 branch coverage: Computed 分岐の read/write 両パスをカバー
- **Depends on**: T1, T2

### T4: デストラクチャリング安全化 + テスト

- **Work**:
  1. `destructuring.rs:228-236`: 個別要素アクセスを `build_safe_index_expr` で `.get(i).cloned()` に変更（`convert_index_to_usize` で IntLit 生成）
  2. Rest 要素（Range）は変更なし
  3. テスト更新:
     - `tests/destructuring.rs`: 既存テスト `test_convert_stmt_list_array_destructuring_basic` 等を更新 — `Expr::Index` → `.get().cloned()` MethodCall を期待
     - `test_convert_stmt_list_array_destructuring_skip_element` を更新 — skip 後のインデックスが正しいことを検証
     - 新規テスト `test_array_destructuring_rest_keeps_range_index` — rest 要素が Range のまま
- **Completion criteria**:
  - デストラクチャリングが `.get(i).cloned()` を生成
  - Rest 要素が Range index を維持
  - 全テスト通過
- **Depends on**: T1

### T5: スナップショット + E2E テスト更新

- **Work**:
  1. `cargo test` でスナップショット差分を確認し、`cargo insta review` で更新
  2. E2E テスト影響分析: `arr[0]` が `Option<T>` になるため、生成コードが `println!` で Option を使用しコンパイルエラーになる。影響を受ける E2E テストを特定し、一時スキップ（skip 理由に I-319 を記録）
     - `array_ops.ts` — `arr[0]`, `arr[4]`, `arr[idx]` を使用
     - `destructuring.ts` — `const [a, b] = arr` を使用（配列デストラクチャリング部分）
     - その他影響テストがないか `tests/e2e/scripts/` を走査して確認
  3. スキップされた E2E テストの復活は、downstream の `Option<T>` handling（自動 unwrap 等）の実装時に行う
- **Completion criteria**:
  - `cargo test` 全通過
  - スナップショットが `.get().cloned()` パターンを反映
  - 影響を受ける E2E テストがスキップされ、skip 理由に I-319 を記録
- **Depends on**: T1, T2, T3, T4

**Note**: 新規 E2E テストは追加しない。`Option<T>` の downstream handling（`println!` での unwrap 等）がない現状では、安全インデックスの実行時動作を E2E で検証できない。検証はユニットテスト + スナップショットで行う。

### T6: 品質チェック + ベンチマーク

- **Work**:
  1. `cargo fix --allow-dirty --allow-staged`
  2. `cargo fmt --all --check`
  3. `cargo clippy --all-targets --all-features -- -D warnings`
  4. `cargo test`（全テスト通過確認）
  5. `./scripts/hono-bench.sh` でクリーン率変化を確認
  6. `./scripts/check-file-lines.sh` で行数チェック
- **Completion criteria**:
  - 0 errors, 0 warnings
  - ベンチマークのクリーン率が低下していないことを確認
- **Depends on**: T5

## Test Plan

### ユニットテスト（T3, T4）

| テスト | 目的 | 手法 |
|--------|------|------|
| Vec literal index → safe get | 正常パス | Equivalence Partition |
| Vec variable index → safe get | 正常パス | Equivalence Partition |
| 型不明 literal index → safe get | 安全側フォールバック | Equivalence Partition |
| for_write → direct index | 代入ターゲット | C1 Branch |
| Range index → unchanged | 除外パス | C1 Branch |
| Tuple index → field access | 除外パス（既存） | C1 Branch |
| destructuring → safe get | 正常パス | Equivalence Partition |
| destructuring rest → range | 除外パス | C1 Branch |
| .length → .len() as f64 | 既存ギャップ | C1 Branch (G4) |
| process.env.VAR | 既存ギャップ | C1 Branch (G3) |
| opt_chain computed | DRY リファクタ | Equivalence Partition (G5) |

### E2E テスト（T5）

- 新規追加なし（`Option<T>` の downstream handling 未実装のため E2E 検証不可）
- 影響を受ける既存テスト（`array_ops.ts`, `destructuring.ts`）を一時スキップ

## Completion Criteria

1. `cargo test` 全通過（0 failures）
2. `cargo clippy` 0 warnings
3. `cargo fmt --all --check` 通過
4. S1 バグ: I-319 解消（2 → 1）
5. I-343 解消（Vec\<Struct\> move エラー）
6. Hono ベンチマーク: クリーン率低下なし
7. `./scripts/check-file-lines.sh` 通過
