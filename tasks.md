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
**Transformer:** AnyTypeAnalyzer 統合済み。to_pascal_case 集約済み。SyntheticTypeRegistry ソート修正済み。Transformer struct 導入 D-2-A〜F 全完了（全モジュールのメソッド化 + TypeEnv 所有化 + ファクトリメソッド導入 + 全ラッパー削除 + `convert_default_value` メソッド化 + `convert_class_decl` 削除）。

**残存する実装不足:**
- D-2-2: 監査指摘対応（4 関数メソッド化漏れ、NarrowingGuard リファクタリング、フィールド private 化、`let reg` 除去）
- Phase E: 最終検証

## タスク一覧

### Phase A-C + リファクタリング + D0a/D0b/D7（全完了）

省略（git history 参照）

### Phase D: 残作業

**実施順序と依存関係:**

```
D1 (import 解決)            ─┐
D6 (files.clone 解消)       ─┼─→ 完了
                             │
D-TR〜D4 (型解決の統一)     ─┤
  Phase 1〜3: 完了           │
  Phase 4: TypeEnv 簡素化   │  完了
                             │
D5 (reg パラメータ削除)     ─┘─→ 完了
                             │
D-2 (Transformer struct)    ─┘─→ D5 完了後に実施
```

#### D1: import 解決の ModuleGraph 統合 ✅

- [x] **D1**: `transform_import` / `transform_export_named` / `export_all` に ModuleGraph lookup + fallback を適用
  - `resolve_import_path_with_fallback` ヘルパー追加。`ModuleGraph.resolve_import()` を優先し、失敗時は `convert_relative_path_to_crate_path` にフォールバック
  - re-export chain の解決に対応（テスト 3 件追加）

#### D6: files.clone() 解消 ✅

- [x] **D6**: `FileOutput` に `source: String` フィールドを追加し `main.rs` の `files.clone()` を解消

#### D-TR 〜 D4: 型解決の統一 ✅

TypeResolver（pre-pass）を完全化し、runtime fallback（ExprContext / heuristic / TypeEnv narrowing）を全削除。Phase 1〜4 全完了。詳細は git history 参照。

#### D5: tctx + reg 二重パラメータ統合

Phase 2（ExprContext 削除）で `ctx` パラメータが消えた後、シグネチャが安定した状態で実施する。Phase 2 より前に実施すると、シグネチャ変更が二度手間になる。

- [x] **D5**: 99 関数の `reg: &TypeRegistry` を削除し `tctx.type_registry` に統一（13 ファイル、~350 呼び出し箇所を修正）

#### D-2: Transformer struct 導入 ✅

`tctx`, `type_env`, `synthetic` の 3 パラメータを `Transformer` struct のフィールドに束ね、106 関数をメソッドに変換。全ラッパー削除、current_file_dir 除去、メソッドリネーム完了。詳細は git history 参照。

#### D-2-2: Transformer struct 監査指摘対応

**詳細計画: `tasks.d-2-2.md`**

D-2 完了後の監査で検出された 5 課題の対応。4 関数のメソッド化漏れ、NarrowingGuard リファクタリング、フィールド private 化、`let reg` 除去。

- [ ] **D-2-2**: 監査指摘対応（A〜E の 5 フェーズ）
  - **依存**: D-2 完了後

### Phase E: 最終検証

- [ ] **E1**: `cargo test` 全 GREEN
- [ ] **E2**: `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- [ ] **E3**: `cargo fmt --all --check` 通過
- [ ] **E4**: Hono ベンチマーク実行、結果が改善していることを確認
- [ ] **E5**: pub な型・関数に doc コメントがあることを確認
- [ ] **E-commit**: P8 コミット
