# P7: Generator の純粋化 + OutputWriter

## 背景・動機

P6 で Generator のセマンティック判断（`.as_str()` 付加、enum 分類、regex import スキャン）を Transformer に移動した。本 PRD では Generator が純粋な IR→テキスト変換であることを検証・確認し、新規に `OutputWriter` を構築する。

現在の出力フローの問題:

1. **mod.rs 生成の分散**: `src/directory.rs` の `transpile_directory` が mod.rs を生成するが、`convert_relative_path_to_crate_path` を使った手続き的なパス変換で、再エクスポートチェーンの解決が不完全（I-222）
2. **合成型配置の不在**: 合成型（union enum, any-enum, inline struct）の配置先が決まっていない。単一ファイルに閉じた合成型はインラインで良いが、複数ファイルから参照される合成型は専用モジュールに配置すべき
3. **rustfmt の呼び出し**: 現在は `directory.rs` 内で行われるが、OutputWriter に統合することでテスト容易性が向上する

`report/pipeline-component-design.md` セクション 6.10（Generator）、セクション 6.11（OutputWriter）に基づく。

## ゴール

1. Generator が IR のみに依存し、セマンティック判断が一切ないことを確認・保証する
2. `OutputWriter` を構築し、ファイル書き出し・mod.rs 生成・合成型配置・rustfmt を統一的に処理する
3. mod.rs を `ModuleGraph` の query API（`children_of`, `reexports_of`）から生成する
4. 合成型の配置ロジック: 単一ファイル使用→インライン、複数ファイル使用→専用モジュール

## スコープ

### スコープ内

- Generator の純粋化確認:
  - P6 で移動済みのセマンティック判断が Generator に残っていないことの検証
  - Generator が `Vec<Item> → String` の純粋関数であることのテスト
- `OutputWriter` struct の実装:
  - `ModuleGraph` を参照して mod.rs を生成（`pub mod` + `pub use`）
  - 合成型の配置先決定ロジック
  - ファイル書き出し（出力ディレクトリへの書き込み）
  - rustfmt の実行
- mod.rs 生成:
  - `module_graph.children_of(path)` から `pub mod child_module;` を生成
  - `module_graph.reexports_of(path)` から `pub use child_module::ExportedType;` を生成
  - ネストしたディレクトリ構造への対応
- 合成型配置ロジック:
  - 各合成型が参照されるファイルを IR から検索
  - 単一ファイルからのみ参照 → そのファイルにインラインで配置
  - 複数ファイルから参照 → 専用モジュール（`types.rs` 等）に配置し、mod.rs で re-export

### スコープ外

- Transformer の改修（P6 で完了済み）
- TypeResolver の改修（P5 で完了済み）
- 統一パイプラインの組み立て（P8）
- 既存 API の置き換え・削除（P8）
- `convert_relative_path_to_crate_path` の削除（P8。OutputWriter が代替する）

## 設計

`report/pipeline-component-design.md` セクション 6.10（Generator）、セクション 6.11（OutputWriter）、セクション 7（Pass 6, Pass 7）に準拠。

### Generator の確認事項

```rust
// src/generator/mod.rs（確認のみ、変更なし）

/// IR → Rust ソースコードのテキスト変換。
/// セマンティック判断を行わない。IR の構造をそのまま文字列化する。
pub fn generate(items: &[Item]) -> String;
```

Generator の各関数を走査し、以下が存在しないことを確認:
- 型に基づく条件分岐（「この型なら `.as_str()` を付ける」等）
- `TypeRegistry` / `SyntheticTypeRegistry` / `ModuleGraph` への参照
- import/export のスキャン

### OutputWriter

```rust
// src/pipeline/output_writer.rs（新規）

/// 変換結果の出力を担当する。
pub struct OutputWriter<'a> {
    module_graph: &'a ModuleGraph,
}

impl<'a> OutputWriter<'a> {
    pub fn new(
        module_graph: &'a ModuleGraph,
    ) -> Self { ... }

    /// 変換結果をディレクトリに書き出す。
    /// 合成型の Item は呼び出し元が SyntheticTypeRegistry.all_items() で取得して渡す。
    pub fn write_to_directory(
        &self,
        output_dir: &Path,
        file_outputs: &[(PathBuf, String)],
        synthetic_items: &[Item],
        run_rustfmt: bool,
    ) -> Result<()> { ... }

    /// mod.rs の内容を生成する（テスト用に公開）。
    pub fn generate_mod_rs(&self, dir_path: &Path) -> String { ... }

    /// 合成型の配置先を決定する。
    pub fn resolve_synthetic_placement(
        &self,
        file_outputs: &[(PathBuf, String)],
        synthetic_items: &[Item],
    ) -> SyntheticPlacement { ... }
}

/// 合成型の配置結果。
pub struct SyntheticPlacement {
    /// ファイルにインラインで追加する合成型: (ファイルパス, 合成型コード)
    pub inline: HashMap<PathBuf, Vec<String>>,
    /// 専用モジュールに配置する合成型: (モジュールパス, 合成型コード)
    pub shared_module: Option<(PathBuf, String)>,
}
```

### mod.rs 生成ロジック

```rust
fn generate_mod_rs(&self, dir_path: &Path) -> String {
    let mut lines = Vec::new();

    // 子モジュールの pub mod 宣言
    for child in self.module_graph.children_of(dir_path) {
        lines.push(format!("pub mod {};", child.module_name()));
    }

    // re-export の pub use 宣言
    for reexport in self.module_graph.reexports_of(dir_path) {
        lines.push(format!(
            "pub use {}::{};",
            reexport.module_path, reexport.name
        ));
    }

    lines.join("\n")
}
```

### 合成型配置ロジック

1. 呼び出し元から渡された `synthetic_items`（`SyntheticTypeRegistry.all_items()` の結果）を使用
2. 各合成型の名前で全ファイルの生成コードを検索し、参照ファイルを特定
3. 参照ファイル数で配置先を決定:
   - 1 ファイル → そのファイルの先頭にインラインで挿入
   - 2+ ファイル → 共有モジュール（`types.rs`）に配置し、各ファイルから `use` する
   - 0 ファイル → 未使用として警告（ログ出力のみ、エラーにはしない）

### 影響ファイル

- **新規**: `src/pipeline/output_writer.rs`
- **変更**: `src/pipeline/mod.rs`（モジュール追加）
- **確認（変更なし）**: `src/generator/mod.rs`, `src/generator/expressions.rs`, `src/generator/types.rs`
- **参照（変更なし）**: P2 の ModuleGraph、P3 の SyntheticTypeRegistry

## 作業ステップ

### Step 1: Generator 純粋化の確認（検証）

1. `src/generator/` 内の全ファイルを走査し、セマンティック判断が残っていないことを確認
2. 確認項目:
   - TypeRegistry / SyntheticTypeRegistry / ModuleGraph への参照がない
   - 型に基づく条件分岐がない（IR の `Item` / `Expr` の種別に基づく分岐は OK）
   - import スキャンがない
3. 残っている場合は P6 の漏れとして修正する

### Step 2: テスト設計（RED）

1. `generate_mod_rs` のテスト:
   - 子モジュール 2 つ → `pub mod a;\npub mod b;`
   - re-export 1 つ → `pub use child::Foo;`
   - 子モジュール + re-export → 両方が含まれる
   - 空ディレクトリ → 空文字列
2. `resolve_synthetic_placement` のテスト:
   - 合成型が 1 ファイルのみで参照 → `inline` に含まれる
   - 合成型が 2 ファイルで参照 → `shared_module` に含まれる
   - 未使用の合成型 → inline にも shared にも含まれない
3. `write_to_directory` のテスト:
   - 出力ディレクトリにファイルが書き出される
   - mod.rs が生成される
   - 合成型がインラインまたは専用モジュールに配置される

### Step 3: mod.rs 生成の実装（GREEN）

1. `OutputWriter` の骨格を実装
2. `generate_mod_rs` を `ModuleGraph.children_of()` + `reexports_of()` で実装
3. Step 2-1 のテストを GREEN にする

### Step 4: 合成型配置の実装（GREEN）

1. `resolve_synthetic_placement` を実装
2. 合成型の参照検索（生成コード内の名前マッチ）
3. 配置先の決定ロジック
4. Step 2-2 のテストを GREEN にする

### Step 5: ファイル書き出しの実装（GREEN）

1. `write_to_directory` を実装
2. ディレクトリ作成、ファイル書き出し、mod.rs 配置
3. 合成型のインライン挿入 / 専用モジュール生成
4. rustfmt の実行（オプション）
5. Step 2-3 のテストを GREEN にする

### Step 6: 統合テスト + リファクタリング（REFACTOR）

1. 既存のディレクトリ変換テストと同等の結果が OutputWriter で得られることを確認
2. Hono ベンチマークで mod.rs の生成が正しいことを確認
3. `cargo clippy`, `cargo fmt --check`
4. ドキュメントコメントの整備

## テスト計画

| テスト | 検証内容 | 期待結果 |
|--------|---------|---------|
| `test_generator_is_pure` | Generator にセマンティック判断がない | 型参照・import スキャンなし |
| `test_mod_rs_children` | 子モジュールの pub mod 生成 | `pub mod a;\npub mod b;` |
| `test_mod_rs_reexports` | re-export の pub use 生成 | `pub use child::Foo;` |
| `test_mod_rs_mixed` | 子モジュール + re-export | 両方が含まれる |
| `test_mod_rs_empty` | 空ディレクトリ | 空文字列 |
| `test_placement_single_file` | 合成型が 1 ファイルのみで参照 | `inline` に配置 |
| `test_placement_multi_file` | 合成型が 2+ ファイルで参照 | `shared_module` に配置 |
| `test_placement_unused` | 未使用の合成型 | どちらにも含まれない |
| `test_write_directory_structure` | ディレクトリ出力 | ファイル + mod.rs が正しく配置 |
| `test_write_with_inline_synthetic` | インライン合成型 | ファイル先頭に合成型コード |
| `test_write_with_shared_synthetic` | 共有合成型 | 専用モジュール + use 文 |
| 既存テスト全体 | 後方互換性 | `cargo test` が全 GREEN |
| Hono ベンチマーク | 変換品質 | ベンチマーク結果が悪化していない |

## 完了条件

- [ ] Generator にセマンティック判断が一切残っていない（IR → テキストの純粋変換のみ）
- [ ] `OutputWriter` が実装されている
- [ ] `generate_mod_rs` が `ModuleGraph.children_of()` + `reexports_of()` から mod.rs を正しく生成する
- [ ] 合成型配置ロジックが実装されている（単一ファイル→インライン、複数ファイル→専用モジュール）
- [ ] `write_to_directory` がファイル書き出し + mod.rs + 合成型配置 + rustfmt を実行する
- [ ] 上記テスト計画の全テストが GREEN
- [ ] `cargo test` で既存テストが全て GREEN（後方互換）
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 警告
- [ ] Hono ベンチマークで結果が悪化していない
- [ ] pub な型・関数に `///` ドキュメントコメントがある
