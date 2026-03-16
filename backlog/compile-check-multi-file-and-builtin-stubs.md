# コンパイルチェックのディレクトリ対応 + TS ビルトイン型スタブ生成

対象 TODO: I-32（残り 2 件のスキップ解消）

## 背景・動機

コンパイルテストは変換結果が Rust としてコンパイル可能かを検証する重要な品質ゲートだが、2 つの構造的制約がある:

1. **単一ファイルでのコンパイルチェックのみ** — 外部型参照（`Env['Bindings']` → `Env::Bindings`）を含むコードが検証できない。Hono のような複数ファイルプロジェクトの変換結果を検証するには、ディレクトリ単位のコンパイルチェックが必要。
2. **TS ビルトイン型のスタブがない** — `Promise<T>` は TS のビルトイン型だが、変換後の `<T as Promise>::Output` は Rust 側で `Promise` trait が定義されていないためコンパイルエラー。Hono は `Promise<T>` を多用するため、ここでの対応が必須。

## ゴール

- ディレクトリ単位の compile-check テスト関数が実装され、複数ファイルの変換結果をまとめて `cargo check` できる
- TS ビルトイン型（`Promise<T>`）に対応する Rust trait スタブが変換時に自動生成される
- コンパイルテストのスキップが 2 → 0 件になる
- 既存テストに退行がない

## スコープ

### 対象

- `tests/compile_test.rs` にディレクトリ compile-check 関数を追加
- `tests/fixtures/multi/` にディレクトリ型 fixture を新設（既存の単一ファイル fixture は変更しない）
- 残り 2 件のスキップ対象（conditional-type, indexed-access-type）をディレクトリ fixture として再構成
  - `conditional-type`: `Promise` trait 定義を含む補助ファイルを同梱
  - `indexed-access-type`: `Env` 型定義を含む補助ファイルを同梱
- 変換器に TS ビルトイン型（`Promise`）のスタブ trait 自動生成を追加

### 対象外

- `Array`, `Map`, `Set` 等の他のビルトイン型スタブ（需要発生時に対応）
- 既存の単一ファイル fixture の構造変更
- E2E テストの変更

## 設計

### 技術的アプローチ

#### ディレクトリ compile-check

`tests/compile_test.rs` に `assert_compiles_directory(dir: &str)` を追加:

1. ディレクトリ内の全 `.ts` ファイルを `transpile_collecting` で変換
2. `main.ts` → `src/lib.rs`（エントリポイント）、その他 → `src/<name>.rs`
3. `src/lib.rs` 先頭に `mod <name>;` を挿入
4. `cargo check` を実行
5. テスト後にモジュールファイルをクリーンアップ

#### TS ビルトイン型スタブ

変換時に `Promise<T>` が conditional type の `infer` パターンで使用された場合、出力に `Promise` trait を自動挿入:

```rust
pub trait Promise {
    type Output;
}
```

実装箇所: `src/transformer/types/mod.rs` の conditional type 変換で、`<T as Promise>::Output` を生成する際に `Promise` trait が必要なことをマークし、最終出力の `items` に追加。

#### fixture 再構成

- `tests/fixtures/multi/conditional-type/`: `main.ts`（conditional type 定義）のみ。`Promise` スタブは変換器が自動生成。
- `tests/fixtures/multi/indexed-access-type/`: `main.ts`（getBindings 関数）+ `env.ts`（`Env` interface 定義）

### 影響範囲

- `tests/compile_test.rs` — ディレクトリ compile-check 関数追加
- `tests/fixtures/multi/` — ディレクトリ型 fixture 新設
- `src/transformer/types/mod.rs` — Promise trait スタブ生成
- `src/lib.rs` — transpile の出力に prelude を含める仕組み（必要に応じて）

## 作業ステップ

- [ ] ステップ 1: `tests/compile_test.rs` に `assert_compiles_directory` 関数を実装
- [ ] ステップ 2: `tests/fixtures/multi/indexed-access-type/` を作成（`main.ts` + `env.ts`）。ディレクトリ compile-check テストで PASS 確認
- [ ] ステップ 3: 変換器に `Promise` trait スタブの自動生成を追加。ユニットテスト作成
- [ ] ステップ 4: `tests/fixtures/multi/conditional-type/` を作成（`main.ts` のみ、`Promise` は自動生成）。ディレクトリ compile-check テストで PASS 確認
- [ ] ステップ 5: 単一ファイル fixture のスキップリストから `conditional-type` と `indexed-access-type` を除去。単一ファイルで通らない場合は fixture 内容を自己完結的に修正するか、ディレクトリ版のみでカバー
- [ ] ステップ 6: 全テスト退行チェック

## テスト計画

- **ディレクトリ compile-check**: 複数ファイルの変換結果が `cargo check` を通ること
- **Promise スタブ**: `Unwrap<T> = <T as Promise>::Output` を含むコードがコンパイル可能であること
- **indexed-access-type**: `Env::Bindings` を含むコードが `Env` 定義と合わせてコンパイル可能であること
- **退行テスト**: 既存の全テスト（ユニット、統合、E2E、単一ファイル compile-check）が PASS

## 完了条件

- [ ] ディレクトリ compile-check 関数が実装され、2 件のディレクトリ fixture が PASS
- [ ] `Promise` trait スタブが変換時に自動生成される
- [ ] コンパイルテストのスキップが 0 件
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] `cargo fmt --all --check` が PASS
- [ ] `cargo test` が全 PASS
