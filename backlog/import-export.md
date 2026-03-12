# import/export 対応

## 背景・動機

現在、`export` は宣言を `pub` にするだけで、`import` は完全に無視されている。
複数ファイルで構成される実際の TS プロジェクトを変換するには、モジュール間の依存関係を Rust の `mod` / `use` に変換する必要がある。

## ゴール

- `import { Foo } from "./bar"` → `use crate::bar::Foo;` に変換できる
- `export` された宣言は `pub` 付きで生成される（既存動作の維持）
- `export` されていない宣言は `pub` なしで生成される

## スコープ

### 対象

- named import: `import { A, B } from "./module"`
- named export: `export { A, B }` （宣言と同時の export は既存で対応済み）
- `export` の有無による `pub` / 非 `pub` の切り替え
- 相対パスの解決（`./foo` → `crate::foo`）

### 対象外

- default import/export (`import Foo from ...`, `export default ...`)
- re-export (`export { Foo } from "./bar"`)
- 外部パッケージの import (`import _ from "lodash"`)
- dynamic import (`import()`)
- namespace import (`import * as Foo from ...`)
- `require()`

## 設計

### 技術的アプローチ

1. **IR の拡張**: トップレベルに `Use` アイテムを追加する
   ```rust
   pub enum Item {
       Use { path: String, names: Vec<String> },
       // ... existing variants
   }
   ```

2. **Transformer の拡張**: `ModuleDecl::Import` を処理する
   - `import { A, B } from "./module"` → `Item::Use { path: "crate::module", names: vec!["A", "B"] }`
   - パス変換: `./foo/bar` → `crate::foo::bar`

3. **Visibility の切り替え**: 現在すべて `Public` だが、`export` なしの宣言は `Private` にする
   - `transform_module` で `ModuleItem::Stmt` は `Private`、`ModuleItem::ModuleDecl::ExportDecl` は `Public`

4. **Generator の拡張**: `Item::Use` → `use path::{names};` を生成

### 影響範囲

| ファイル | 変更内容 |
|----------|----------|
| `src/ir.rs` | `Item::Use` バリアント追加 |
| `src/transformer/mod.rs` | import 処理追加、visibility 切り替え |
| `src/generator.rs` | `Item::Use` の生成 |
| `tests/fixtures/` | import/export のテスト fixture 追加 |
| `tests/snapshots/` | 新しいスナップショット |

## 作業ステップ

- [ ] Step 1: `export` なし宣言を `Private`（`pub` なし）で生成するよう修正
- [ ] Step 2: IR に `Item::Use` を追加
- [ ] Step 3: Generator に `use` 文の生成を実装
- [ ] Step 4: Transformer で `ModuleDecl::Import` → `Item::Use` 変換を実装
- [ ] Step 5: 相対パスの `crate::` パスへの変換を実装
- [ ] Step 6: E2E fixture テスト追加

## テスト計画

| # | 入力 | 期待出力 | 種別 |
|---|------|----------|------|
| 1 | `export interface Foo {}` | `pub struct Foo {}` | 正常系（既存動作維持） |
| 2 | `interface Foo {}` (export なし) | `struct Foo {}` (pub なし) | 正常系 |
| 3 | `import { Foo } from "./bar"` | `use crate::bar::Foo;` | 正常系 |
| 4 | `import { A, B } from "./bar"` | `use crate::bar::{A, B};` | 正常系（複数） |
| 5 | `import { Foo } from "./sub/bar"` | `use crate::sub::bar::Foo;` | 正常系（ネストパス） |
| 6 | `import { Foo } from "lodash"` | スキップ（警告なし） | 対象外入力 |
| 7 | `export default function ...` | スキップ | 対象外入力 |

## 完了条件

- 上記テストが全パス
- 既存の95テストが引き続き全パス
- `cargo clippy` 0警告、`cargo fmt --check` 0エラー
