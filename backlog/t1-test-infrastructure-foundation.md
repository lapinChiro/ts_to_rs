# T-1: テスト基盤の構造的欠陥修正

## Background

E2E テスト基盤の全件レビュー（`report/e2e-test-infrastructure-review-2026-03-31.md`）で、テスト基盤自体に複数の構造的欠陥が発見された。テスト基盤が脆弱な状態で開発を続けると、品質を担保できず手戻りリスクが増大する。

### 発見された構造的欠陥

1. **collecting/builtins モードの `_unsupported` 未検証**: 9テストが unsupported syntax を収集するが結果を破棄。callable-interface の Factory 欠落を見過ごした直接原因
2. **orphan fixture**（2件）: `explicit-type-args.input.ts`, `private-member-expected-type.input.ts` にテスト関数なし
3. **`strip_internal_use_statements` DRY 違反**: `e2e_test.rs:47` と `compile_test.rs:52` に同一ロジック重複
4. **一時ファイルの残存リスク**: `_exec.ts` が panic 時に残る（`.gitignore` は対応済み）
5. **rust-runner / compile-check 依存バージョン不統一**: regex `"1.12"` vs `"1"` 等のマイナー差異

## Goal

- collecting/builtins モードの全テストで `_unsupported` がスナップショットとして固定される
- orphan fixture が 0 件
- `strip_internal_use_statements` が単一の実装に統合される
- E2E 一時ファイルが RAII で管理され panic 時にもクリーンアップされる
- rust-runner / compile-check の依存バージョンが一致する

## Scope

### In Scope

- `snapshot_test!` マクロの `collecting` / `builtins` variant 改修
- orphan fixture 2件のテスト登録
- `strip_internal_use_statements` の共通モジュール抽出
- E2E 一時ファイルの RAII 化
- rust-runner / compile-check の Cargo.toml 依存同期

### Out of Scope

- スナップショットの内容拡充（T-3）
- E2E スクリプトの追加（T-4）
- コンパイルテストの `#![allow]` 改善（T-2）
- 変換ロジックのバグ修正

## Design

### Technical Approach

#### collecting/builtins マクロの unsupported スナップショット化

`tests/integration_test.rs` の `snapshot_test!` マクロの `collecting` / `builtins` variant で `_unsupported` を捨てずにスナップショット化する。

```rust
($name:ident, collecting) => {
    #[test]
    fn $name() {
        let fixture = stringify!($name)
            .strip_prefix("test_")
            .unwrap_or(stringify!($name))
            .replace('_', "-");
        let input = fs::read_to_string(format!("tests/fixtures/{fixture}.input.ts")).unwrap();
        let (output, unsupported) = transpile_collecting(&input).unwrap();
        insta::assert_snapshot!(output);
        if !unsupported.is_empty() {
            let json = serde_json::to_string_pretty(&unsupported).unwrap();
            insta::assert_snapshot!(format!("{}_unsupported", stringify!($name)), json);
        }
    }
};
```

unsupported が空の場合はスナップショットを生成しない。空であることは `transpile()` モード（unsupported でエラー）のテストが通ることで保証される。

`builtins` variant も同様に改修。

**影響する 9 テスト（collecting）**: callable_interface, intersection_empty_object, intersection_fallback, intersection_union_distribution, interface_methods, narrowing_truthy_instanceof, trait_coercion, anon_struct_inference, instanceof_builtin

**影響する 4 テスト（builtins）**: vec_method_expected_type, external_type_struct, string_methods_with_builtins, instanceof_builtin_with_builtins

#### orphan fixture のテスト登録

- `explicit-type-args.input.ts`: ジェネリック型引数の明示的インスタンス化をテスト。`transpile()` モードで登録
- `private-member-expected-type.input.ts`: private メソッドの expected type 伝播をテスト。`transpile()` モードで登録

#### `strip_internal_use_statements` の共通化

`tests/test_helpers.rs` を新規作成し、`e2e_test.rs` と `compile_test.rs` から `#[path = "test_helpers.rs"] mod test_helpers;` で参照。

#### E2E 一時ファイルの RAII 化

`TempFile` 構造体を `test_helpers.rs` に定義:

```rust
pub struct TempFile { path: String }
impl TempFile {
    pub fn new(path: String, content: &str) -> Self {
        fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
        Self { path }
    }
}
impl Drop for TempFile {
    fn drop(&mut self) { let _ = fs::remove_file(&self.path); }
}
```

`execute_e2e` と `run_e2e_multi_file_test` を改修。

### Design Integrity Review

- **Higher-level consistency**: `snapshot_test!` マクロは `integration_test.rs` 内でのみ使用される。マクロ変更は同ファイル内に閉じる
- **DRY**: `strip_internal_use_statements` の重複を解消
- **Coupling**: `test_helpers.rs` は `e2e_test.rs` と `compile_test.rs` に新たな依存を導入するが、テストヘルパーとして妥当な粒度

Verified, 上記以外の問題なし。

### Impact Area

| ファイル | 変更内容 |
|---------|---------|
| `tests/integration_test.rs` | マクロ改修 + orphan fixture 登録 |
| `tests/e2e_test.rs` | `strip_internal_use_statements` → 共通モジュール参照 + RAII 化 |
| `tests/compile_test.rs` | `strip_internal_use_statements` → 共通モジュール参照 |
| `tests/test_helpers.rs` | 新規作成 |
| `tests/e2e/rust-runner/Cargo.toml` | 依存バージョン統一 |
| `tests/compile-check/Cargo.toml` | 依存バージョン統一 |

### Semantic Safety Analysis

Not applicable — テスト基盤の変更であり、型解決や型フォールバックの変更なし。

## Task List

### T1: `snapshot_test!` マクロ collecting variant の unsupported スナップショット化

- **Work**: `tests/integration_test.rs` の `snapshot_test!` マクロ `collecting` variant を改修。unsupported が非空の場合、`serde_json::to_string_pretty` で JSON 化し `insta::assert_snapshot!` で固定。`serde_json` の依存が `dev-dependencies` に必要なら追加
- **Completion criteria**: collecting モード 9 テスト全てで、unsupported が非空のテストに `_unsupported.snap` が生成される。callable_interface の unsupported に Factory の construct signature が記録される
- **Depends on**: None
- **Prerequisites**: None

### T2: `snapshot_test!` マクロ builtins variant の unsupported スナップショット化

- **Work**: T1 と同様に builtins variant を改修
- **Completion criteria**: builtins モード 4 テストで unsupported スナップショットが生成される（非空の場合）
- **Depends on**: T1（マクロの設計パターンを共有）
- **Prerequisites**: None

### T3: 新スナップショットの承認

- **Work**: `cargo test --test integration_test` を実行し、新スナップショットを `cargo insta review` で承認。各スナップショットの内容を目視確認し、unsupported の kind と location が正確であることを検証
- **Completion criteria**: 全スナップショットが承認済み。callable_interface の unsupported に `ConstructSignature` が含まれることを確認
- **Depends on**: T1, T2
- **Prerequisites**: None

### T4: orphan fixture のテスト登録

- **Work**: `tests/integration_test.rs` に `snapshot_test!(test_explicit_type_args);` と `snapshot_test!(test_private_member_expected_type);` を追加。`cargo test` で生成されるスナップショットを `cargo insta review` で承認
- **Completion criteria**: 2件の orphan fixture がゼロ。全 fixture に対応するテスト関数が存在
- **Depends on**: None
- **Prerequisites**: None

### T5: `strip_internal_use_statements` の共通モジュール抽出

- **Work**: `tests/test_helpers.rs` を新規作成し `strip_internal_use_statements` を移動。`e2e_test.rs` と `compile_test.rs` に `#[path = "test_helpers.rs"] mod test_helpers;` を追加し、ローカル定義を削除
- **Completion criteria**: `strip_internal_use_statements` の実装が 1 箇所のみ。`cargo test` pass
- **Depends on**: None
- **Prerequisites**: None

### T6: E2E 一時ファイルの RAII 化

- **Work**: `test_helpers.rs` に `TempFile` 構造体を定義。`e2e_test.rs` の `execute_e2e` 内の `ts_exec_path` 書き込み + 削除を `TempFile::new` に置換。`run_e2e_multi_file_test` の `main_exec.ts` も同様に改修
- **Completion criteria**: `_exec.ts` の手動 `fs::remove_file` が 0 箇所。`cargo test --test e2e_test` pass
- **Depends on**: T5（`test_helpers.rs` が存在すること）
- **Prerequisites**: None

### T7: Cargo.toml 依存バージョン統一

- **Work**: `tests/e2e/rust-runner/Cargo.toml` と `tests/compile-check/Cargo.toml` のバージョン表記を統一（rust-runner に合わせる: regex "1.12", scopeguard "1.2", serde "1.0", serde_json "1.0"）。両ファイルに `# NOTE: Keep dependencies in sync with tests/{相手}/Cargo.toml` コメントを追加
- **Completion criteria**: 両 Cargo.toml の依存が完全一致。`cargo test` pass
- **Depends on**: None
- **Prerequisites**: None

## Test Plan

- T1-T3: `cargo test --test integration_test` で全 snapshot テスト pass。unsupported スナップショットの内容を目視確認
- T4: 新規登録 2 テストの pass + スナップショット承認
- T5: `cargo test --test e2e_test` + `cargo test --test compile_test` pass
- T6: テスト pass + panic 時のクリーンアップを手動確認（テスト実行中に Ctrl+C で中断し `_exec.ts` が残らないことを確認）
- T7: `cargo test` 全体 pass

## Completion Criteria

1. collecting/builtins モードの全テストで unsupported がスナップショット化されている
2. callable_interface の unsupported に Factory の construct signature が記録されている
3. orphan fixture が 0 件
4. `strip_internal_use_statements` が `test_helpers.rs` に単一実装
5. E2E 一時ファイルが RAII 管理
6. rust-runner / compile-check の Cargo.toml 依存が一致
7. `cargo test` 全 pass、`cargo clippy --all-targets -- -D warnings` 0 warnings、`cargo fmt --all --check` pass
