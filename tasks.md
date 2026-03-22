# P7 OutputWriter — tasks.md

## 完了条件（PRD より）

1. Generator にセマンティック判断が一切残っていない
2. `OutputWriter` が実装されている
3. `generate_mod_rs` が `ModuleGraph.children_of()` + `reexports_of()` から mod.rs を正しく生成する
4. 合成型配置ロジックが実装されている
5. `write_to_directory` がファイル書き出し + mod.rs + 合成型配置 + rustfmt を実行する
6. テスト計画の全テストが GREEN
7. 既存テスト全 GREEN
8. clippy 0 警告
9. Hono ベンチマーク悪化なし
10. pub な型・関数に `///` ドキュメントコメントがある

## タスク一覧

### Phase A: Generator 純粋化の検証テスト

- [x] **A1**: Generator 純粋性確認 — P6 で既に検証済み（context.rs の test_generator_no_regex_scan_transparent, test_generator_no_match_as_str_injection）。Generator の全 use 文が crate::ir のみであることも確認

### Phase B: OutputWriter の実装（TDD）

- [x] **B1-B4**: OutputWriter 実装 — `src/pipeline/output_writer.rs` を新規作成
  - `OutputWriter` struct（`module_graph: &'a ModuleGraph`）
  - `generate_mod_rs`: ModuleGraph.children_of() で pub mod、reexports_of() で pub use を生成
  - `resolve_synthetic_placement`: 合成型の参照ファイル数で配置先を決定（1→inline, 2+→shared, 0→未使用）
  - `write_to_directory`: ファイル書き出し + mod.rs + 合成型配置 + rustfmt
  - `SyntheticPlacement` struct（inline, shared_module）
  - テスト 10 件追加（mod.rs 4 件 + 合成型配置 3 件 + write 3 件）

### Phase C: 統合テスト + 検証

- [x] **C1**: 全テスト GREEN — lib 1092 + E2E 60 + integration 69 + compile 3 + doc 2
- [x] **C2**: clippy 0 警告
- [x] **C3**: fmt 通過
- [x] **C4**: Hono ベンチマーク変化なし（53.2%）
- [x] **C5**: pub な型・関数に `///` ドキュメントコメントあり
- [ ] **C-commit**: `[WIP] P7: OutputWriter 実装`
