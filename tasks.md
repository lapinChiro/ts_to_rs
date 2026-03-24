# P8 統合 + 既存 API 置き換え — tasks.md

## 完了条件（PRD より）

1. 統一パイプライン `transpile_pipeline(TranspileInput) -> TranspileOutput` が全コンポーネントを接続して動作する
2. 既存 lib.rs 公開 API が統一パイプラインのラッパーになっている
3. 既存 main.rs のディレクトリ/単一ファイルモードが統一パイプライン呼び出しになっている
4. `transpile_single()` の簡易 API が提供されている
5. 不要コードが削除されている
6. 既存の全 E2E テスト・スナップショットテストが GREEN
7. cargo test 全 GREEN
8. clippy 0 警告
9. Hono ベンチマーク結果が改善している
10. pub な型・関数に doc コメントがある

## 現在の状況

**パイプライン:** 全 Pass 接続済み。`transpile_pipeline` 本実装。
**lib.rs:** `transpile()` / `transpile_collecting()` の 2 関数のみ。旧 API 削除済み。
**main.rs:** `TranspileInput` + `transpile_pipeline` + `OutputWriter` 直接使用。旧 API 依存なし。
**Transformer:** Transformer struct 導入完了。全関数メソッド化、フィールド private 化、`let reg` 除去、entry point 簡素化、type_resolution メソッド化まで完了。
**パス解決:** C案（フォールバック廃止）完了。`convert_relative_path_to_crate_path` 削除、`TrivialResolver` 導入、ModuleGraph に一本化。絶対パスリグレッション解消。

**残存する実装不足:**
- Phase E: 最終検証（E5: doc コメント確認 のみ未実施）

## タスク一覧

### Phase A〜D（全完了）

省略（git history 参照）。D-2-2 + D-2-2-2（監査指摘対応 + type_resolution メソッド化）まで全完了。

### Phase E: 最終検証

- [x] **E1**: `cargo test` 全 GREEN（1225 テスト通過）
- [x] **E2**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [x] **E3**: `cargo fmt --all --check` 通過
- [x] **E4**: Hono ベンチマーク実行、結果が改善していることを確認（84→86 clean, 133→132 errors, compile file 79→85）
- [x] **E5**: pub な型・関数に doc コメントがあることを確認
- [ ] **E-commit**: P8 コミット

### Phase E 中に発見・修正したバグ

- **絶対パスリーク**: `Transformer::current_file_dir()` が絶対パスを返し、`convert_relative_path_to_crate_path()` が `crate::/tmp/...` を生成していた。C案（フォールバック廃止、ModuleGraph 一本化）で修正:
  - `TrivialResolver` 追加（単一ファイルモード用）
  - `resolve_import()` にワイルドカード対応 + 動的モジュールパス計算を追加
  - `convert_relative_path_to_crate_path` / `current_file_dir()` / `resolve_import_path_with_fallback()` を削除
  - 絶対パスのテスト 15 件追加（以前はゼロ）
- **ベンチマーク再現性**: Hono コミットハッシュを `bench-history.jsonl` に記録するよう改善
