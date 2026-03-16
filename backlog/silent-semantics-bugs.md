# サイレント意味相違の修正

対象 TODO: I-60, I-17

## 背景・動機

変換結果がコンパイルは通るが、TS と異なる意味の Rust コードを生成する問題が 2 件ある。コンパイラが検出できないため最も危険。

1. **I-60**: switch の非リテラル case が Rust の変数バインディングになり、常にマッチする
2. **I-17**: body 内で変更される `const` 変数が `let`（不変）のままで、TS のセマンティクス（const はオブジェクト参照の不変であってフィールド変更は可能）と異なる

## ゴール

- 非リテラル case を含む switch が、マッチガード付き match または if-chain に正しく変換される
- body 内で変更される変数が `let mut` として生成される
- 各修正に対応する E2E テストが PASS する

## スコープ

### 対象

- I-60: switch 変換（`src/transformer/statements/mod.rs`）で非リテラル case を検出し、マッチガード `x if x == A =>` を生成
- I-17: 変数宣言（`src/transformer/statements/mod.rs` の `convert_var_decl`）で body 内の代入先を解析し、`let mut` を生成

### 対象外

- switch の fall-through パターン改善
- `const` の完全なフリーズセマンティクス（deep freeze）

## 設計

### 技術的アプローチ

#### I-60: switch 非リテラル case

`convert_switch_stmt` で case の値がリテラル（数値、文字列、bool）かどうかを検査。非リテラルの場合:
- `match x { A => ... }` の代わりに `match x { _v if _v == A => ... }` を生成
- または、全 case が非リテラルなら if-else chain にフォールバック

#### I-17: const ミュータビリティ

既に `mark_mut_params_from_body` が関数パラメータに対して実装済み。同じパターンを `convert_var_decl` に適用:
- `const` 宣言後の statement list をスキャンし、変数名が代入先に出現するか検査
- 出現する場合は `let mut` を生成

### 影響範囲

- `src/transformer/statements/mod.rs` — switch 変換、変数宣言
- `tests/e2e/scripts/` — E2E テスト追加

## 作業ステップ

- [ ] ステップ 1: I-60 — 非リテラル case の検出ロジックを実装。ユニットテスト作成
- [ ] ステップ 2: I-60 — マッチガード生成または if-chain フォールバックを実装。ユニットテスト PASS
- [ ] ステップ 3: I-60 — E2E テストで非リテラル case を含む switch の動作を検証
- [ ] ステップ 4: I-17 — `convert_var_decl` に body スキャン + `let mut` 生成を実装。ユニットテスト作成
- [ ] ステップ 5: I-17 — E2E テストで const 変数の変更を含むスクリプトを検証
- [ ] ステップ 6: 全テスト退行チェック

## テスト計画

- **I-60 正常系**: 非リテラル case（定数変数を参照）が正しい値と比較されること
- **I-60 境界値**: リテラル case と非リテラル case が混在する switch
- **I-17 正常系**: `const x = 0; x = 1;` が `let mut x = 0.0; x = 1.0;` に変換されること
- **I-17 境界値**: body 内で変更されない const は `let` のままであること

## 完了条件

- [ ] 非リテラル case が正しいマッチガード付きで生成される
- [ ] body 内で変更される const が `let mut` で生成される
- [ ] E2E テスト PASS
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] `cargo fmt --all --check` が PASS
- [ ] `cargo test` が全 PASS
