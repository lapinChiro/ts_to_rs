# E2E テスト信頼性: スキップ解消 + 非決定性排除

対象 TODO: I-32, I-44

## 背景・動機

テスト基盤の信頼性に 2 つの問題がある:

1. **コンパイルテストのスキップ**: 51 件中 4 件（indexed-access-type, conditional-type, interface-mixed, union-type）がスキップされており、これらの変換結果がコンパイル可能かどうかが検証されていない。スキップされたテストは「検証されていないコード」の存在を意味する。
2. **合成 struct 名の非決定性**: `_TypeLit0`, `_Intersection1` 等の合成名がグローバル `AtomicU32` カウンタで生成されるため、テスト実行順序によって名前が変わる。スナップショットテストで厳密な名前一致が書けず、テストの信頼性が低い。

## ゴール

- 4 件のスキップされたコンパイルテストがすべて有効化され PASS する（変換結果のコンパイルエラーは修正する）
- 合成 struct 名がテスト間で決定的に生成される仕組みが導入されている
- 既存のスナップショットテストが安定して PASS する

## スコープ

### 対象

- スキップ中の 4 コンパイルテストの有効化と、変換結果の修正
  - `indexed-access-type`: indexed access type の変換結果がコンパイルを通るように修正
  - `conditional-type`: conditional type の変換結果がコンパイルを通るように修正
  - `interface-mixed`: mixed interface の変換結果がコンパイルを通るように修正
  - `union-type`: union type の変換結果がコンパイルを通るように修正
- 合成 struct 名のカウンタをファイル/テスト単位でリセット可能にする

### 対象外

- スキップ解消に伴うスナップショット更新以外のスナップショット変更
- 合成 struct 名の命名規則自体の変更（`_TypeLit` プレフィックス等は維持）
- E2E テストスクリプトの追加（本 PRD はテスト信頼性の改善のみ）

## 設計

### 技術的アプローチ

#### コンパイルテストのスキップ解消

各スキップ対象について:
1. 現在の変換結果を確認（`cargo test` でスナップショット出力を取得）
2. 変換結果を `tests/compile-check/` に配置してコンパイルエラーを確認
3. コンパイルエラーの原因を特定し、変換ロジックを修正
4. スキップ指定を除去してテストを有効化
5. スナップショットを更新

#### 合成 struct 名の決定性

現状: `src/transformer/types/mod.rs` 等でグローバル `AtomicU32` カウンタを使用

対策案（推奨: ファイル単位のローカルカウンタ）:
- `Transformer` 構造体にカウンタフィールドを追加（`type_lit_counter: u32`, `intersection_counter: u32` 等）
- `Transformer::new()` でカウンタを 0 に初期化
- グローバルカウンタの参照を `Transformer` のメソッド呼び出しに置き換え
- テストごとに `Transformer` が新規作成されるため、カウンタは自動的にリセットされる

### 影響範囲

- `src/transformer/types/mod.rs` (カウンタの移動)
- `src/transformer/mod.rs` (Transformer 構造体にカウンタ追加)
- `tests/` (スキップ解除、スナップショット更新)
- 4 件の fixture の変換結果に関連するコード

## 作業ステップ

- [x] ステップ 1: スキップ中の 4 件それぞれについて、現在の変換結果とコンパイルエラーの内容を調査・記録
- [ ] ステップ 2: `indexed-access-type` — 未宣言型参照 (Env::Bindings)。型解決基盤が必要（I-35 関連）。現時点ではスキップ維持
- [ ] ステップ 3: `conditional-type` — 未使用型パラメータ + 未定義 Promise trait。型別名の構造的変更が必要（I-28 関連）。現時点ではスキップ維持
- [x] ステップ 4: `interface-mixed` — Generator で trait impl の `pub` 除去 + 空ボディに `todo!()` 生成。スキップ解除、テスト PASS
- [ ] ステップ 5: `union-type` — derive 伝播 + PhantomData 不足。変更範囲が大きい（I-26 関連）。現時点ではスキップ維持
- [x] ステップ 6: `reset_synthetic_counter()` を追加し、各 `transpile` エントリポイントの先頭で呼び出し。テスト実行順序に依存しない決定的な名前生成を実現
- [x] ステップ 7: 全テスト PASS（744 件）、スナップショット安定
- [x] ステップ 8: `cargo insta accept` で interface-mixed のスナップショット更新済み

## テスト計画

- **スキップ解消**: 4 件すべてのコンパイルテストが PASS すること
- **カウンタ決定性**: 同じテストを 10 回実行して、スナップショットが同一であること
- **退行テスト**: 既存の全テスト（ユニット、統合、E2E）が PASS すること

## 完了条件

- [ ] 51 件のコンパイルテスト中、スキップが 0 件 → **4 → 3 に削減**（interface-mixed 解消）。残 3 件は変換ロジックの構造的問題（I-26/28/35 関連）
- [x] グローバルカウンタにリセット関数を追加し、transpile 呼び出しごとにリセット
- [x] `cargo test` を複数回実行しても結果が安定していること
- [x] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [x] `cargo fmt --all --check` が PASS
- [x] `cargo test` が全 PASS（744 件）
