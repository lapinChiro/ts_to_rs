# I-368: OutputWriter の共有合成型モジュール `types.rs` パス衝突

## 背景

ディレクトリモードでは、複数ファイルから参照される合成型（union enum 等）を専用モジュール `types.rs` に配置する（`output_writer.rs:147`）。しかし入力に `types.ts` が含まれる場合、ユーザーの変換結果 `types.rs` を共有合成型モジュールが上書きし、**ユーザーコードが完全に消失する**。

Hono フレームワークの `src/types.ts` がこのケースに該当し、dir compile の失敗はこの上書きが主因。

## 現象

1. パイプラインは `types.ts` のユーザー定義型（`TypedResponse`, `StatusCode` 等）を正しく Item に変換する
2. `generate(&all_items)` が正しい Rust コードを生成する（`rust_source` に含まれている）
3. `OutputWriter::write_to_directory` が Step 1 でユーザーコード + インライン合成型を `types.rs` に書き出す
4. Step 2 で共有合成型モジュールを **同じ `types.rs` に上書き** → ユーザーコード消失

**再現確認済み**: `--no-builtin-types` では正しく出力、ビルトインありでは全ユーザーアイテムが消失。

## 根本原因

`output_writer.rs:147`:
```rust
Some((PathBuf::from("types.rs"), content))
```

共有合成型モジュールのパスが `types.rs` にハードコードされており、ユーザーファイル名との衝突チェックがない。

`write_to_directory` の実行順序:
1. Line 186: `std::fs::write(&out_path, &content)` — ユーザーの `types.rs` を書き出し
2. Line 196: `std::fs::write(&types_path, types_code)` — 共有合成型で **同名ファイルを上書き**

## 影響範囲

- `types.ts` を含む全プロジェクトのディレクトリモード変換が影響
- Hono の `src/types.ts`（2488 行、プロジェクト最大のファイル）が直撃
- dir compile 失敗の主因（これが解消しないと types.rs のエラー分析自体が無意味）

## 設計

### 方針: 共有合成型モジュールの衝突回避命名

共有合成型モジュールのファイル名を、ユーザーファイルと衝突しない名前に変更する。

### 変更箇所

1. **`output_writer.rs:147`**: 共有合成型モジュールのパスを動的に決定
   - 候補名リスト: `["_types.rs", "_synthetic_types.rs", "_generated_types.rs"]`
   - `file_outputs` のパス集合と照合し、最初の衝突しない名前を選択
   - 全候補が衝突する場合（極端なケース）: 末尾にカウンタを付与 `_types_1.rs`

2. **`output_writer.rs:205-209`**: `mod.rs` の `pub mod types;` 生成を動的パスに追従
   - `types_rel_path` のステムを使用: `pub mod {stem};`

3. **テスト修正**: `test_write_to_directory_shared_synthetic`（`output_writer.rs:524`）を更新
   - 衝突するケース（`file_outputs` に `types.rs` がある場合）のテスト追加
   - 衝突しないケース（従来動作）の維持確認

### 設計レビュー

- **凝集度**: OutputWriter の責務（出力ファイルの配置決定）に閉じている。命名ロジックは `resolve_synthetic_placement` 内に集約
- **責務分離**: パイプライン本体（`mod.rs`）への変更不要。OutputWriter 内で完結
- **DRY**: 衝突チェックロジックは 1 箇所のみ（`resolve_synthetic_placement`）

## タスク

1. `resolve_synthetic_placement` に衝突回避ロジックを追加
2. `mod.rs` 生成の動的パス対応
3. 既存テスト修正 + 衝突ケースのテスト追加
4. Hono ベンチマークで types.rs のユーザーコードが保持されることを確認

## 完了条件

1. `types.ts` を含むプロジェクトのディレクトリモード変換で、ユーザー定義型が出力に含まれる
2. 共有合成型モジュールがユーザーファイルと衝突しない名前で生成される
3. 既存テスト全 pass
4. 衝突ケースのテスト追加

## 関連

- I-367 調査メモ（旧 PRD）: `backlog/i367-forward-reference-type-converter.md`
- OutputWriter 実装: `src/pipeline/output_writer.rs:83-154`（配置決定）、`159-227`（書き出し）
- パイプライン統合: `src/main.rs:234`（`write_to_directory` 呼び出し）
- 既存テスト: `src/pipeline/output_writer.rs:524`（`test_write_to_directory_shared_synthetic`）
