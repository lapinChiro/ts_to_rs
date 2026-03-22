# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`

## 引継ぎ事項

### P8 の作業状態（作業中）

**完了済み（Phase A-D の大部分）:**
- Phase A: 統一パイプライン本実装（`transpile_pipeline` 全 Pass 接続、`TranspileOutput` 拡張、`transpile_single` API、`find_common_root`）
- Phase B: lib.rs API 整理（公開 API を `transpile` / `transpile_collecting` の 2 関数に整理。旧 API 5 関数削除。重大バグ 2 件修正: expected_type 優先順位、Promise unwrap）
- Phase C: main.rs 統一（`TranspileInput` + `transpile_pipeline` + `OutputWriter` 直接使用。Hono ベンチ: ディレクトリコンパイル 91.8%→98.7%）
- リファクタリング: DRY 修正（`byte_pos_to_line_col` / `resolve_unsupported` / `run_rustfmt` を lib.rs に集約）、`unwrap()` → エラーハンドリング、`find_common_root` テスト 6 件追加、compile_test を `transpile_pipeline` ベースに書き換え
- Phase D 完了分:
  - D0a: AnyTypeAnalyzer 統合（`generate_any_enum` → `build_any_enum_variants` + `register_any_enum`）
  - D0b: I-212 解消確認（統一パイプライン dedup で解消済み）
  - D7: `to_pascal_case` 重複解消（`any_narrowing.rs` に集約）
  - SyntheticTypeRegistry の `into_items()` / `all_items()` に名前順ソートを追加（出力の決定性保証）
  - D-TR-1: TypeResolver カバレッジギャップ調査完了（`report/d-tr1-type-resolver-coverage-gaps.md`）
  - D1: import 解決に ModuleGraph lookup + fallback を適用
  - D6: `FileOutput` に `source` フィールドを追加し `files.clone()` を解消
  - Phase 1: TypeResolver expected_types 完全化（`propagate_expected` メソッド追加）
  - Phase 2: `ExprContext` 完全削除（パラメータ除去、テスト29件修正、struct削除、clippy 0警告）
    - expected type 伝搬を `convert_expr_with_expected` 経由に統一（calls.rs, binary.rs, assignments.rs, data_literals.rs, member_access.rs, functions.rs, statements/mod.rs, classes.rs）
    - E2E テスト: 60/60 全 GREEN（旧コード 50/60 から改善）
  - Phase 2.5-A: TypeResolver `propagate_expected` ギャップ埋め（5 パターン追加）+ `visit_var_decl` 再構成 + `resolve_arrow_expr`/`resolve_fn_expr` に expected type 読み取り追加
    - `propagate_expected`: DU fields, HashMap value, Arrow body, Rest params, OptChain method args
    - `visit_var_decl`: `resolve_expr` 3回→1回、expected type 設定を resolution の前に移動
    - 関数型エイリアス注釈からの return type/param types 推論（`resolve_fn_type_info` ヘルパー）
    - `test_var_type_alias_arrow` 修正（ネストされた object literal の struct name 推論）
    - integration test 69/69 全 GREEN

**次に着手すべき作業 — Phase 2.5-B〜D: Expected Type 伝搬の一本化:**

`tasks.expected-type-unification.md` に詳細設計を記載。Phase 2.5-A で TypeResolver のギャップを埋めた。残りは B（テストヘルパー整備）→ C（unit test 移行）→ D（Transformer 手動伝搬削除）。

調査レポート: `report/expected-type-dual-propagation.md`, `report/var-type-alias-arrow-failure.md`

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212 は P8 で**解消済み**。残存エラー: `f64.toFixed()` 未対応 + `StringOrF64` の Display 未実装

## 保留中

（なし）
