# T-2: コンパイルテストの品質改善

## Background

`tests/compile_test.rs` が `#![allow(unused, dead_code, unreachable_code)]` で全警告を抑制しており、変換品質の指標（不要な `let mut`、dead code、到達不能コード）が隠蔽されている。また skip リスト 12 件のうち一部は TODO との紐付けが不完全。

テスト基盤が品質情報を隠すことは品質担保の妨げであり、検出すべき警告を段階的に表面化させる必要がある。

## Goal

- `unused_mut` 警告がコンパイルテストで検出可能になる
- `unreachable_code` 警告が検出可能になる（到達不能な catch ブロック等）
- skip リスト全項目が TODO Issue ID と紐付いている
- レビューで発見された新規バグ（S1/S2/SD）が TODO に追記されている

## Scope

### In Scope

1. `#![allow(...)]` の分解と段階的な警告検出
2. `unused_mut` で新たに失敗する fixture への対応（変換ロジック修正 or 個別 allow + TODO 紐付け）
3. skip リストの TODO ID 紐付け確認・補完
4. レビューで発見された新規バグ 4 件の TODO 追記

### Out of Scope

- skip リスト項目の変換バグ修正（既存 TODO の各 Issue で対応）
- E2E テストの追加（T-4）
- fixture 内容の拡充（T-3）

## Design

### Technical Approach

#### `#![allow]` の分解

現状:
```rust
"#![allow(unused, dead_code, unreachable_code)]\n"
```

改修後:
```rust
"#![allow(dead_code, unused_variables, unused_imports, unused_assignments)]\n"
```

**除去する allow**: `unused_mut`, `unreachable_code`

**残す allow の理由**:
- `dead_code`: トランスパイラ出力では未使用の struct/enum が型定義として必要（テストからは呼ばれない）
- `unused_variables`: TS の `const _ = expr` パターンが頻出
- `unused_imports`: use 文の最適化は変換品質ではなく生成コード品質の問題
- `unused_assignments`: 分割代入のパターンで発生しうる

#### `unused_mut` 対応

`const` 宣言が `let mut` に変換されるケースが複数 fixture で確認されている（object-literal, string-to-string, trait-coercion, type-infer-unannotated）。これはミュータビリティ推論（`src/transformer/statements/mutability.rs`）の不正確さに起因する。

対応方針:
1. まず `unused_mut` を allow から除外して失敗する fixture を全列挙
2. 根本原因がミュータビリティ推論の「const は全て let mut にする」というロジックであれば、`const` 宣言を `let`（mut なし）にする修正を行う
3. 個別の特殊ケース（再代入が別スコープ等）は TODO に記録

#### 新規バグの TODO 追記

レビューで発見された未追跡の 4 件:

| バグ | 内容 | 深刻度 |
|------|------|--------|
| f64 パターンマッチ | switch の数値 case が `match x { 1.0 => ... }` に変換。f64 のパターンマッチは非推奨、NaN で予期しない挙動 | S1 |
| optional chaining 配列アクセス | `x?.[0]` → `x.as_ref().map(|_v| _v[0])` で境界外アクセス時に panic（TS は undefined） | S1 |
| Result prelude シャドウイング | `type Result = Success | Failure` → `enum Result` が `std::result::Result` をシャドウイング | S1 |
| _TypeLit 重複生成 | inline-type-literal-param で `_TypeLit0`/`_TypeLit1` が名前付き struct と重複して生成される | SD |

### Design Integrity Review

- **Higher-level consistency**: コンパイルテストの allow 変更は `compile_test.rs` 内に閉じる。他テストレイヤーへの影響なし
- **DRY**: `assert_compiles` 関数内の allow 文字列を定数化することを検討（`test_all_fixtures_compile` と `test_all_fixtures_compile_with_builtins` で共有）

Verified, 上記以外の問題なし。

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `tests/compile_test.rs` | `#![allow]` 分解、allow 定数化 |
| `src/transformer/statements/mutability.rs` | const → let（mut なし）修正（範囲次第） |
| `TODO` | 新規バグ 4 件追記、skip リスト紐付け補完 |

### Semantic Safety Analysis

Not applicable — テスト基盤の変更とミュータビリティ推論の修正であり、型フォールバックの変更なし。

## Task List

### T1: `#![allow]` の分解

- **Work**: `tests/compile_test.rs` の `assert_compiles` 関数内の `#![allow(unused, dead_code, unreachable_code)]` を `#![allow(dead_code, unused_variables, unused_imports, unused_assignments)]` に変更。`assert_compiles_directory` 関数内も同様
- **Completion criteria**: `unused_mut` と `unreachable_code` が allow されない状態で `cargo check --test compile_test` が通る（テスト実行前の段階）
- **Depends on**: None
- **Prerequisites**: None

### T2: `unused_mut` 失敗 fixture の全列挙

- **Work**: T1 の状態で `cargo test --test compile_test 2>&1 | tee /tmp/compile-test-result.txt` を実行し、`unused_mut` で失敗する fixture を全て特定。各 fixture について根本原因を分類（const→let mut の一律変換 / 他の原因）
- **Completion criteria**: 失敗 fixture の一覧と根本原因分類が完了
- **Depends on**: T1
- **Prerequisites**: None

### T3: ミュータビリティ推論の修正

- **Work**: `src/transformer/statements/mutability.rs` で `const` 宣言（TS の `const`）が `let mut` ではなく `let` に変換されるよう修正。`VarDeclKind::Const` の場合は mutation detection をスキップし常に immutable にする
- **Completion criteria**: object-literal, string-to-string 等の fixture で `const` が `let`（mut なし）に変換される。既存テスト pass。`unused_mut` で失敗する fixture が 0 件
- **Depends on**: T2
- **Prerequisites**: None

### T4: skip リストの TODO ID 紐付け確認

- **Work**: `compile_test.rs` の `skip_compile` リストと `skip_compile_with_builtins` リストの全項目について、TODO の Issue ID との対応を確認。`intersection-empty-object` の未使用型パラメータ問題は新規 Issue ID を採番して TODO に追加
- **Completion criteria**: skip リスト全項目（`indexed-access-type` と `external-type-struct` の builtins テストでカバー済み2件を除く）が TODO Issue ID と紐付いている
- **Depends on**: None
- **Prerequisites**: None

### T5: 新規バグ 4 件の TODO 追記

- **Work**: TODO に以下 4 件を新規 Issue ID で追記: (1) f64 パターンマッチ（S1）、(2) optional chaining 配列アクセス panic（S1）、(3) Result prelude シャドウイング（S1）、(4) _TypeLit 重複生成（SD）。各項目は TODO entry standards に従い、ベンチマーク実測値・ソースコード参照・解決方向を含む
- **Completion criteria**: 4 件が TODO に Issue ID 付きで追記されている
- **Depends on**: None
- **Prerequisites**: None

### T6: plan.md 更新

- **Work**: `plan.md` のコンパイルテストスキップ欄を最新化。`intersection-empty-object` の新 Issue ID を反映
- **Completion criteria**: plan.md が現状を正確に反映
- **Depends on**: T4, T5
- **Prerequisites**: None

## Test Plan

- T1-T3: `cargo test --test compile_test` 全 pass（`unused_mut` 警告なし）
- T3: `cargo test --test integration_test` でスナップショットが更新される場合は `cargo insta review` で承認
- 全体: `cargo test` pass、`cargo clippy --all-targets -- -D warnings` 0 warnings

## Completion Criteria

1. `#![allow]` が個別 lint に分解され、`unused_mut` と `unreachable_code` が検出可能
2. `unused_mut` で失敗する fixture が 0 件（修正済み）
3. skip リスト全項目が TODO Issue ID と紐付いている
4. 新規バグ 4 件が TODO に追記されている
5. `plan.md` が最新化されている
6. `cargo test` 全 pass、`cargo clippy` 0 warnings、`cargo fmt --all --check` pass
