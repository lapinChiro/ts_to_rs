# main.rs のビジネスロジック抽出リファクタリング

## 背景・動機

`src/main.rs` に以下のビジネスロジックが I/O コードと混在しており、テストが存在しない:

1. **複数ソースからの共有 TypeRegistry 構築**（128-165行）: クロスファイル型解決のコア機能だが、`fs::read_to_string` と混在しているためユニットテストが書けない。「ファイル A で定義した型をファイル B で参照できるか」を検証するテストが存在しない
2. **デフォルト出力ディレクトリ名の計算**（139-147行）: `<dirname>_rs` という命名ロジックが純粋関数であるにもかかわらず main.rs に閉じ込められ、テストされていない

これらを適切なモジュールに抽出し、テストを追加する。

## ゴール

- main.rs からビジネスロジックが抽出され、main.rs には I/O グルーコードのみが残る
- 抽出したロジックに対するユニットテストが存在する
- 既存の CLI 動作は一切変わらない（リファクタリングのみ）

## スコープ

### 対象

- 複数ソースからの共有 TypeRegistry 構築ロジック → `lib.rs` に抽出
- デフォルト出力ディレクトリ名の計算ロジック → `directory.rs` に抽出
- 抽出した関数のユニットテスト追加

### 対象外

- location の filepath remap（7行×2箇所。抽出しても得られるテスト価値が低い）
- `run_rustfmt` の抽出（外部プロセス呼び出しでテスト困難。ロジックも薄い）
- main.rs への integration test の追加（CLI 全体の E2E テストは別スコープ）
- 機能追加・API 変更

## 設計

### 技術的アプローチ

#### 1. 共有 TypeRegistry 構築の抽出

`lib.rs` に以下の pub 関数を追加:

```rust
/// Build a shared TypeRegistry from multiple TypeScript sources.
pub fn build_shared_registry(sources: &[&str]) -> TypeRegistry {
    let mut shared = TypeRegistry::new();
    for source in sources {
        if let Ok(module) = parser::parse_typescript(source) {
            let reg = build_registry(&module);
            shared.merge(&reg);
        }
    }
    shared
}
```

main.rs の `transpile_directory_common` は、ファイル読み込み後に `build_shared_registry` を呼び出す形に変更。

#### 2. デフォルト出力ディレクトリ名の抽出

`directory.rs` に以下の pub 関数を追加:

```rust
/// Compute default output directory path by appending `_rs` suffix.
pub fn default_output_dir(input_dir: &Path) -> PathBuf {
    let mut name = input_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    name.push_str("_rs");
    input_dir.with_file_name(name)
}
```

### 影響範囲

- `src/lib.rs` — `build_shared_registry` 関数の追加 + テスト
- `src/directory.rs` — `default_output_dir` 関数の追加 + テスト
- `src/main.rs` — 抽出した関数の呼び出しに変更（ロジック削減）

## 作業ステップ

- [ ] ステップ1: `build_shared_registry` のテストを `lib.rs` に追加（RED）
- [ ] ステップ2: `build_shared_registry` を `lib.rs` に実装（GREEN）
- [ ] ステップ3: `default_output_dir` のテストを `directory.rs` に追加（RED）
- [ ] ステップ4: `default_output_dir` を `directory.rs` に実装（GREEN）
- [ ] ステップ5: `main.rs` の `transpile_directory_common` を抽出した関数の呼び出しにリファクタリング
- [ ] ステップ6: 全テスト・clippy・fmt 通過を確認

## テスト計画

### `build_shared_registry`

- 単一ソースからレジストリ構築
- 複数ソースで型が共有される（ファイル A の型をファイル B で参照可能）
- パース失敗するソースが混在しても他のソースは処理される
- 空のソースリスト → 空のレジストリ

### `default_output_dir`

- 通常ケース: `src` → `src_rs`（同階層に `_rs` サフィックス）
- ルートディレクトリ: `/` → 境界ケースの挙動確認

## 完了条件

- `cargo test` 全テスト通過
- `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
- `cargo fmt --all --check` フォーマット通過
- main.rs に `build_registry` / `merge` の直接呼び出しが残っていないこと
- main.rs にデフォルト出力ディレクトリ名の計算ロジックが残っていないこと
