# モジュール参照解決システムの比較調査

**基準コミット**: `bcfc4a5`（未コミットの変更あり: ベンチマーク改善 + 調査レポートを含む）
**調査日**: 2026-03-21

## 1. 現在の実装の詳細分析

### 1.1 処理フロー

ディレクトリモードの import/export 変換は以下の順序で行われる:

```
1. collect_ts_files(input_dir)           — .ts ファイルを再帰収集
2. build_shared_registry(sources)        — 全ファイルから TypeRegistry を構築（Pass 1）
3. for each (ts_path, ts_source):        — 各ファイルを個別に変換（Pass 2）
   a. current_file_dir = ts_path.parent() - input_dir
   b. transpile_collecting_with_registry_and_path(source, registry, current_file_dir)
      → transform_module_collecting_with_path(module, reg, current_file_dir)
        → transform_module_item() for each item:
          - ImportDecl → transform_import(decl, current_file_dir)
          - ExportNamed → transform_export_named(export, current_file_dir)
          - ExportAll → convert_relative_path_to_crate_path(src, current_file_dir)
   c. 変換結果を .rs ファイルに書き出し
4. collect_output_dirs(output_dir)       — mod.rs 生成対象ディレクトリを収集
5. generate_mod_rs(dir) for each dir     — mod.rs を生成
```

### 1.2 コードパス一覧

| ファイル | 関数 | 役割 |
|---------|------|------|
| `src/main.rs:295` | `transpile_directory_common` | ディレクトリモードの全体制御。`current_file_dir` を計算し変換関数に渡す |
| `src/main.rs:336` | (inline) | `ts_path.parent().strip_prefix(input_dir)` で `current_file_dir` を計算 |
| `src/lib.rs:78` | `transpile_collecting_with_registry_and_path` | パス付き変換の公開 API |
| `src/transformer/mod.rs:127` | `transform_module_with_path` | `current_file_dir` を `transform_module_item` に伝搬 |
| `src/transformer/mod.rs:281` | `transform_import` | `ImportDecl` → `Item::Use` 変換 |
| `src/transformer/mod.rs:318` | `transform_export_named` | `NamedExport` → `Item::Use` 変換 |
| `src/transformer/mod.rs:249` | (inline) | `ExportAll` → `Item::Use { names: ["*"] }` 変換 |
| `src/transformer/mod.rs:366` | `convert_relative_path_to_crate_path` | 相対パス → `crate::` パスの文字列変換 |
| `src/generator/mod.rs:44` | (inline) | `Item::Use` → `use path::{names};` テキスト出力 |
| `src/directory.rs:70` | `generate_mod_rs` | ディレクトリ内の .rs ファイルとサブディレクトリから `pub mod X;` を生成 |
| `src/directory.rs:56` | `compute_output_path` | 入力 .ts パスから出力 .rs パスを計算（ハイフン→アンダースコア変換含む） |

### 1.3 対応パターンと未対応パターン

#### 対応パターン

| TS パターン | 変換結果 | 備考 |
|------------|---------|------|
| `import { Foo } from './bar'` | `use crate::bar::Foo;` | ルートファイルの場合 |
| `import { Foo } from './bar'` | `use crate::adapter::bun::bar::Foo;` | `adapter/bun/server.ts` からの場合 |
| `import { Foo } from '../bar'` | `use crate::adapter::bar::Foo;` | `adapter/bun/server.ts` からの場合 |
| `import { Foo } from '../../bar'` | `use crate::bar::Foo;` | `adapter/bun/server.ts` からの場合 |
| `import { Foo } from './hono-base'` | `use crate::hono_base::Foo;` | ハイフン→アンダースコア変換 |
| `export { Foo } from './bar'` | `pub use crate::bar::Foo;` | re-export |
| `export * from './bar'` | `pub use crate::bar::*;` | ワイルドカード re-export |
| `import { Foo } from 'lodash'` | (スキップ) | 外部パッケージは無視 |
| `import type { Foo } from './bar'` | `use crate::bar::Foo;` | type-only import も同じ扱い |

#### 未対応パターン（エッジケース）

| TS パターン | 現在の出力 | 正しい出力 | 原因 |
|------------|-----------|-----------|------|
| `import { Context } from '../..'` | `use crate::adapter::..::Context;` | `use crate::Context;` または re-export チェーン解決 | `../..` でルートに到達するケース。`remaining` が空になり `parts.join("/")` が空文字列を含む |
| `import { stream } from './'` | `use crate::helper::streaming::::stream;` | `use crate::helper::streaming::stream;` | `./` の末尾スラッシュで `stripped` が空文字列になり `dir/` の末尾に `/` が付く |
| `import { Foo } from './mod'` | `pub use crate::adapter::netlify::mod::Foo;` | `pub use crate::adapter::netlify::Foo;` | `mod` は Rust の予約語 |
| default import: `import Foo from './bar'` | (スキップ) | 対応が必要 | `ImportDefaultSpecifier` をフィルタしている |
| namespace import: `import * as bar from './bar'` | (スキップ) | 対応が必要 | `ImportStarAsSpecifier` をフィルタしている |
| re-export チェーン: `A → B → C` | 各ファイルで `use` を個別生成 | 最終解決先を参照 | チェーンを辿らない |

### 1.4 エッジケース分析

現在の方式の根本的な制約:

1. **ファイル単位の逐次変換**: 各ファイルが独立に変換されるため、他のファイルの export 情報を参照できない
2. **文字列操作によるパス解決**: `convert_relative_path_to_crate_path` は純粋な文字列操作であり、ファイルシステムの実体（`index.ts` の存在、re-export チェーン等）を考慮しない
3. **TS と Rust のモジュールモデルの不一致**:
   - TS: `index.ts` がディレクトリの入口。`from './'` や `from '../..'` で暗黙に index を参照
   - Rust: `mod.rs` がディレクトリの入口。`use crate::foo` は `foo/mod.rs` を参照
   - TS の re-export チェーン（`index.ts` → `context.ts`）は Rust では `mod.rs` の `pub use` で表現されるが、現在の変換器は re-export チェーンを解決せずに直接パスを生成

## 2. 代替手法

### 2.1 手法 A: 現在の方式（逐次変換 + パス文字列操作）

各ファイルを独立に変換し、import の相対パスを文字列操作で `crate::` パスに変換する。

**概要**: `convert_relative_path_to_crate_path(rel_path, current_file_dir)` で `../foo` → `crate::parent::foo` のようにパス解決。

**実装状態**: 実装済み。I-222 のエッジケースが残存。

### 2.2 手法 B: import/export マップ事前構築（依存グラフ方式）

全ファイルの import/export を先に収集してファイル間の依存グラフを構築し、そのグラフに基づいて Rust の `use` 文を生成する。

**概要**:
1. **Pass 0（新規）**: 全 .ts ファイルをパースし、各ファイルの import/export を収集:
   - import: `{ source_file, specifiers: [name], from_path }` のリスト
   - export: `{ source_file, specifiers: [name], from_path? }` のリスト（local export と re-export を区別）
2. **ファイルパス → Rust モジュールパスのマッピングテーブル** を構築:
   - `adapter/bun/server.ts` → `crate::adapter::bun::server`
   - `index.ts` → `crate`（ルート index）
   - `adapter/bun/index.ts` → `crate::adapter::bun`（ディレクトリ index）
3. **re-export チェーンの解決**: `from '../..'` → `index.ts` → `export { Context } from './context'` → `context.ts` のチェーンを辿り、最終的な定義元ファイルを特定
4. **use 文の生成**: 各ファイルの import に対し、マッピングテーブルで定義元のモジュールパスを引き、`use crate::context::Context;` のように生成

**設計の詳細**:
```rust
struct ModuleGraph {
    /// TS ファイルパス → Rust モジュールパス
    file_to_module: HashMap<PathBuf, String>,
    /// ファイルごとの export 一覧（名前 → 定義元 or re-export 先）
    exports: HashMap<PathBuf, HashMap<String, ExportSource>>,
}

enum ExportSource {
    /// このファイルで定義されている
    Local,
    /// 別ファイルから re-export（チェーンを辿って解決済み）
    ReExport { resolved_module: String },
}
```

### 2.3 手法 C: Rust モジュール構造先行設計（トップダウン方式）

TS のディレクトリ構造から Rust のモジュールツリーを先に設計し、そのツリーに基づいて `use` 文と `mod.rs` を生成する。

**概要**:
1. **Pass 0（新規）**: TS のディレクトリ構造を走査し、Rust のモジュールツリーを設計:
   - 各 .ts ファイル → Rust モジュール（ファイル名がモジュール名）
   - 各ディレクトリ → `mod.rs`（index.ts の内容を含む）
   - `index.ts` は特別扱い: ディレクトリの `mod.rs` にマージ
2. **ファイルパス → モジュールパス変換テーブル** を構築（手法 B と同様）
3. **import 変換**: 変換テーブルを引くだけ。文字列操作によるパス解決は不要
4. **mod.rs 生成**: モジュールツリーに基づいて自動生成

**手法 B との違い**: re-export チェーンは解決せず、Rust 側の `pub use` で表現する。TS の `index.ts` が `pub use` を含む `mod.rs` に変換される。import 先が `index.ts` を経由する場合は `use crate::adapter::bun::Context;` のように `mod.rs` 経由の参照になる。

### 2.4 手法 D: ハイブリッド方式（現在の方式 + index.ts 解決テーブル）

現在の逐次変換方式を維持しつつ、`index.ts` の位置と re-export 内容のテーブルだけを事前構築する。

**概要**:
1. **Pass 0（軽量）**: 全ディレクトリの `index.ts` を走査し、どのディレクトリが `index.ts` を持つかのセットを構築
2. **パス解決の拡張**: `convert_relative_path_to_crate_path` に `index.ts` テーブルを渡し、`./` や `../..` が index を参照する場合にモジュールパスを補正
3. **その他は現在の方式を維持**: re-export チェーンの完全解決は行わない

**手法 A との違い**: `index.ts` の存在を考慮するため、`./` や `../..` のエッジケースが解消される。ただし re-export チェーンは未解決。

## 3. 評価観点の定義

以下の観点で全手法を評価する。各観点は意思決定（どの方式を採用するか）に必要な項目として選定した。

### 3.1 正確性: エッジケースの網羅性

import パスの変換が正しい `use` 文を生成するか。既知の未対応パターン（`../..`、`./`、re-export チェーン、`mod` 予約語、default/namespace import）をどこまでカバーできるか。

**なぜ必要か**: 不正な `use` 文はコンパイルエラーの直接原因。調査で 3 ファイルが import パスエッジケースでコンパイルエラーになっている。

### 3.2 re-export チェーン解決

`A.ts` が `index.ts` を経由して `B.ts` の export を参照するパターンで、最終的な定義元を正しく解決できるか。

**なぜ必要か**: Hono では `index.ts` による re-export が多用されている（`from '../..'` でルート index を参照 → 内部モジュールに転送）。Rust では `mod.rs` + `pub use` で同じ構造を表現するが、`use crate::Context` のように直接参照する方が idiomatic な場合もある。

### 3.3 既存アーキテクチャとの整合性

現在の 2-pass アーキテクチャ（Pass 1: レジストリ構築、Pass 2: 各ファイル変換）への影響。変更の侵入度。

**なぜ必要か**: 既存コードとの整合性が低い手法は、バグの温床になり、他の開発者（将来の自分を含む）が理解しにくくなる。

### 3.4 実装の複雑さ

新規に書くコードの量と、そのコードの概念的な複雑さ。

**なぜ必要か**: 複雑な実装はバグを生みやすく、メンテナンスコストが高い。

### 3.5 計算コスト

追加される処理のファイル数に対するスケーリング特性（O(n)、O(n²) 等）。Hono（158 ファイル）規模での実用性。

**なぜ必要か**: ベンチマークで毎回実行するため、実用的な速度が必要。

### 3.6 拡張性: 将来の要件への対応力

以下の将来の要件にどれだけ自然に対応できるか:
- barrel file（re-export のみのファイル）の最適化
- 循環依存の検出
- 未使用 import の除去
- 外部パッケージの Rust クレートへのマッピング

**なぜ必要か**: モジュール解決は今後も機能追加が見込まれる領域。基盤の拡張性が低いと、追加機能ごとにアドホックな対応が増える。

### 3.7 デバッグ容易性

問題が発生したとき、原因の特定と修正がどれだけ容易か。

**なぜ必要か**: import パスの問題は実際にエッジケースとして発生しており、今後も発生する可能性がある。

### 3.8 テスト容易性

ユニットテスト・統合テストの書きやすさ。テストの入出力が明確に定義できるか。

**なぜ必要か**: テストが書きにくいと品質が維持できない。

## 4. 星取表

5段階評価: ◎ = 優秀、○ = 良い、△ = 普通、▲ = やや劣る、× = 劣る

| 評価観点 | A: 逐次変換（現在） | B: 依存グラフ | C: モジュールツリー先行 | D: ハイブリッド |
|---------|:---:|:---:|:---:|:---:|
| 3.1 正確性 | ▲ | ◎ | ○ | △ |
| 3.2 re-export チェーン解決 | × | ◎ | △ | × |
| 3.3 既存アーキテクチャ整合性 | ◎ | △ | △ | ○ |
| 3.4 実装の複雑さ | ◎ | ▲ | △ | ○ |
| 3.5 計算コスト | ◎ | ○ | ○ | ◎ |
| 3.6 拡張性 | × | ◎ | ○ | ▲ |
| 3.7 デバッグ容易性 | ○ | △ | △ | ○ |
| 3.8 テスト容易性 | ○ | ○ | ○ | ○ |

## 5. 各手法の詳細評価

### 5.1 正確性

- **A (▲)**: `../..`、`./`、`mod` 予約語の3パターンが未対応（I-222）。文字列操作の限界でエッジケースが漏れやすい
- **B (◎)**: ファイルパス → モジュールパスの変換テーブルを事前構築するため、文字列操作のエッジケースが原理的に発生しない。全 import は変換テーブルの lookup で解決
- **C (○)**: B と同様にテーブルベースだが、re-export チェーンの解決は Rust の `pub use` に委ねるため、TS の意味論と Rust の意味論のギャップが残る場合がある（例: TS で `from '../..'` で import した名前が Rust 側の `mod.rs` に `pub use` されていない場合）
- **D (△)**: `index.ts` の存在は考慮するが、`mod` 予約語や他のエッジケースは個別対応が必要。A よりは改善されるが、根本的な文字列操作の制約は残る

### 5.2 re-export チェーン解決

- **A (×)**: re-export チェーンを一切解決しない。`from '../..'` は `crate::` パスに直接変換するが、TS の `index.ts` の re-export 先（`export { Context } from './context'`）を辿れない
- **B (◎)**: 依存グラフに re-export 情報を含むため、チェーンを辿って最終定義元を特定できる。`from '../..'` → `index.ts` → `context.ts` の解決が可能
- **C (△)**: Rust の `pub use` で re-export を表現するため、直接の解決はしない。ただし `index.ts` → `mod.rs` の変換で `pub use` が生成されていれば、Rust コンパイラがチェーンを解決する。問題は、現在の `generate_mod_rs` は `pub mod` のみ生成し `pub use` を生成しないため、`index.ts` の re-export 内容が `mod.rs` に反映されないこと
- **D (×)**: A と同様、re-export チェーンは未解決

### 5.3 既存アーキテクチャ整合性

- **A (◎)**: 現在の実装そのもの。変更不要
- **B (△)**: 新たな Pass 0（import/export 収集）が必要。`ModuleGraph` 構造体の新規追加。`transform_module` に `ModuleGraph` 参照を渡す必要がある。既存の `convert_relative_path_to_crate_path` は廃止
- **C (△)**: B と同様の侵入度。さらに `index.ts` → `mod.rs` マージのロジックが必要
- **D (○)**: 既存の `convert_relative_path_to_crate_path` を拡張するだけ。Pass 0 は軽量（index.ts の存在チェックのみ）

### 5.4 実装の複雑さ

- **A (◎)**: 実装済み。追加コードなし
- **B (▲)**: `ModuleGraph` の構築（全ファイルの import/export をパース）、re-export チェーンの解決アルゴリズム（循環検出を含む）、テーブル lookup への変換パス切り替え。概念は明快だが実装量は多い
- **C (△)**: `index.ts` → `mod.rs` マージのルール設計が必要。どの export を `pub use` にするか、local 宣言と re-export をどう区別するか。B よりは単純だが、Rust モジュール構造の設計ルールが複雑
- **D (○)**: `index.ts` 位置テーブルの構築は単純（ディレクトリ走査のみ）。`convert_relative_path_to_crate_path` の拡張は小規模

### 5.5 計算コスト

- **A (◎)**: ファイルごとの文字列操作のみ。O(n) で n = ファイル数
- **B (○)**: Pass 0 で全ファイルをパース（既存の Pass 1 と一部重複）。re-export チェーン解決は O(E) で E = export 数。Hono 規模（158 ファイル）では問題にならない
- **C (○)**: B と同程度
- **D (◎)**: 追加コストは index.ts の存在チェック（O(D) で D = ディレクトリ数）のみ

### 5.6 拡張性

- **A (×)**: パス文字列操作の関数に新機能を追加するのは困難。barrel file の最適化、循環依存の検出は原理的に不可能
- **B (◎)**: 依存グラフがあるため、循環依存の検出（グラフの DFS）、未使用 import の検出（逆参照の追跡）、barrel file の最適化（re-export のみのノードの圧縮）が自然に実装できる。外部パッケージのマッピングもグラフにノードを追加すれば対応可能
- **C (○)**: モジュールツリーの構造情報があるため、循環依存の検出や barrel file の扱いは可能。ただしグラフ構造ほど柔軟ではない
- **D (▲)**: A よりわずかに改善されるが、index.ts テーブルだけでは循環依存検出や barrel file 最適化は不可能

### 5.7 デバッグ容易性

- **A (○)**: `convert_relative_path_to_crate_path` は純粋関数で、入出力が明確。エッジケースの原因追跡は容易
- **B (△)**: 依存グラフの構築過程が複雑。re-export チェーンの解決で意図しない結果が出た場合、グラフ内の経路をトレースする必要がある
- **C (△)**: モジュールツリーの構築ルールが複雑。`index.ts` のマージルールが期待通りに動作しない場合のデバッグが困難
- **D (○)**: A に近いシンプルさ。追加テーブルの内容を確認すれば問題の原因が分かる

### 5.8 テスト容易性

- **A (○)**: `convert_relative_path_to_crate_path` の入出力テストが書きやすい。現在 6 テストが存在
- **B (○)**: `ModuleGraph` の構築テスト、テーブル lookup テスト、re-export チェーン解決テストが独立して書ける。テスト対象が明確
- **C (○)**: モジュールツリー構築テスト、`index.ts` マージテストが書ける
- **D (○)**: 既存テストの拡張 + index.ts テーブルのテスト追加

## 6. 手法 B vs C の詳細比較

### 6.1 Hono の index.ts の実態

手法 C の中核は「index.ts → mod.rs にマージする」設計だが、Hono の index.ts の実態を調査した結果、以下が判明した:

- Hono の index.ts は **57 ファイル**中、**純粋な re-export ファイル（local 宣言なし）はゼロ**
- 大半が **local 宣言 + re-export の混合ファイル**。例:
  - `helper/cookie/index.ts`: local 宣言 9 個（`getCookie`, `setCookie` 等の関数定義）+ re-export 3 個
  - `middleware/combine/index.ts`: local 宣言 4 個 + re-export 14 個
  - `index.ts`（ルート）: local 宣言 4 個（`Hono` の import + re-export）+ re-export 6 個

この事実は手法 B と C の設計に以下の影響を与える。

### 6.2 具体的シナリオでの動作比較

#### シナリオ 1: `import { Context } from '../..'`（adapter/bun/conninfo.ts）

**TS の意味**: `../../index.ts` の `export type { Context } from './context'` を経由して `context.ts` の `Context` を参照。

**手法 B の動作**:
1. Pass 0 で `index.ts` の export を収集: `Context → ReExport { from: "./context" }`
2. re-export チェーンを解決: `Context` の定義元は `context.ts` → `crate::context`
3. `adapter/bun/conninfo.ts` の `import { Context } from '../..'` を lookup: `Context` → `crate::context::Context`
4. 生成: `use crate::context::Context;`

**手法 C の動作**:
1. Pass 0 で `index.ts` → `mod.rs` にマージ。ルート `index.ts` の re-export `export type { Context } from './context'` を `pub use crate::context::Context;` に変換して `lib.rs`（ルート mod.rs 相当）に含める
2. `adapter/bun/conninfo.ts` の `import { Context } from '../..'` をテーブルで解決: `../..` → ルートモジュール → `crate`
3. 生成: `use crate::Context;`
4. Rust コンパイラが `lib.rs` の `pub use crate::context::Context;` を辿って解決

**比較**: B は定義元を直接参照（`use crate::context::Context`）。C は `pub use` を経由（`use crate::Context`）。**B の方が生成コードが明示的**で、Rust 開発者にとって読みやすい。C は TS の re-export 構造を Rust に直訳するため、間接参照が残る。

#### シナリオ 2: `import { stream } from './'`（helper/streaming/text.ts）

**TS の意味**: `./index.ts` の `export { stream } from './stream'` を経由して `stream.ts` の `stream` を参照。

**手法 B の動作**:
1. `helper/streaming/index.ts` の export を収集: `stream → ReExport { from: "./stream" }`
2. チェーン解決: `stream` の定義元は `helper/streaming/stream.ts` → `crate::helper::streaming::stream`
3. 生成: `use crate::helper::streaming::stream::stream;`

**手法 C の動作**:
1. `helper/streaming/index.ts` → `helper/streaming/mod.rs` にマージ。`pub use crate::helper::streaming::stream::stream;` を含む
2. `./` → 自ディレクトリの index → `crate::helper::streaming`
3. 生成: `use crate::helper::streaming::stream;`
4. Rust コンパイラが `mod.rs` の `pub use` を辿って解決

**比較**: B は `use crate::helper::streaming::stream::stream;`（モジュール名と関数名が重複して冗長）。C は `use crate::helper::streaming::stream;`（`mod.rs` 経由で自然）。**このケースでは C の方が idiomatic Rust に近い**。

#### シナリオ 3: `helper/cookie/index.ts`（混合ファイル）

**TS の構造**: `index.ts` に `getCookie` 等の関数定義（local）と `export type { Cookie } from '../../utils/cookie'` 等の re-export が混在。

**手法 B の動作**:
1. `helper/cookie/index.ts` は通常ファイルとして変換。`getCookie` 等は `helper/cookie/index.rs` に出力
2. 他のファイルから `import { getCookie } from './helper/cookie'` は `crate::helper::cookie::index::getCookie` に解決
3. re-export の `Cookie` 型は定義元 `utils/cookie.ts` を直接参照

**手法 C の動作**:
1. `helper/cookie/index.ts` の内容を `helper/cookie/mod.rs` にマージ。local 宣言 + re-export の `pub use` が `mod.rs` に入る
2. 他のファイルから `import { getCookie } from './helper/cookie'` は `crate::helper::cookie::getCookie` に解決
3. `mod.rs` に関数本体が含まれるため、Rust の慣習（`mod.rs` はサブモジュール宣言のみ）から外れる

**比較**: C は `mod.rs` に関数本体を含めるため、**Rust の慣習に反する**。Rust では `mod.rs` はサブモジュール宣言と `pub use` のみが一般的。B は `index.rs` を通常モジュールとして扱い、`mod.rs` からは `pub mod index;` で参照するため、慣習に沿う。

ただし、C でも「`index.ts` の local 宣言は `index.rs` にそのまま変換し、re-export のみを `mod.rs` の `pub use` に抽出する」という変形設計も可能。

#### シナリオ 4: 循環的な re-export

**TS の構造**（仮定）:
```
a/index.ts: export { Foo } from './foo'; export { Bar } from '../b'
b/index.ts: export { Bar } from './bar'; export { Baz } from '../a'
```

**手法 B の動作**:
1. re-export チェーン解決時に循環を検出 → 循環ノードはチェーン解決を打ち切り、直接の参照先モジュールを使用
2. 実装が必要: DFS で visited set を管理

**手法 C の動作**:
1. 各 `index.ts` → `mod.rs` に `pub use` を生成。循環はそのまま Rust に渡される
2. Rust コンパイラが循環 `pub use` を処理（Rust は `pub use` の循環を許容する）
3. 追加の実装不要

**比較**: C は循環を Rust コンパイラに委ねるため、実装が単純。B は自前で循環検出・処理が必要。

### 6.3 index.ts の扱いの本質的な違い

| 観点 | B: 依存グラフ | C: モジュールツリー先行 |
|------|:---:|:---:|
| index.ts の扱い | 通常ファイルとして変換（index.rs 生成）。re-export は解決してスキップ | mod.rs にマージ。re-export は `pub use` に変換 |
| 生成される use パス | 定義元を直接参照（`use crate::context::Context`） | re-export 経由の参照も許容（`use crate::Context`） |
| mod.rs の内容 | `pub mod` 宣言のみ（Rust 慣習通り） | `pub mod` + `pub use` + 場合により関数本体 |
| 循環 re-export | 自前で検出・処理が必要 | Rust コンパイラに委ねる |
| 依存解決の主体 | 変換器（全てを事前解決） | 変換器（構造設計）+ Rust コンパイラ（`pub use` 解決） |

### 6.4 設計上のトレードオフ

**手法 B の核心的な強み**: 変換器が import の全てを制御する。生成される `use` 文は定義元への直接参照であり、Rust コンパイラに頼る間接参照がない。このため、生成コードが自己完結的で予測可能。

**手法 B の核心的な弱み**: re-export チェーンの解決ロジックが複雑。特に:
- 循環 re-export の検出と処理
- `export *`（ワイルドカード re-export）の解決: 元ファイルの全 export を列挙する必要がある
- `export { Foo as Bar }` のリネーム追跡

**手法 C の核心的な強み**: TS のモジュール構造を Rust のモジュール構造に「構造的に対応」させる。re-export チェーンの解決は Rust コンパイラに委ねるため、変換器の実装が単純。TS と Rust のモジュール構造が 1:1 で対応し、元のコード構造が保存される。

**手法 C の核心的な弱み**: `index.ts` が混合ファイル（local 宣言 + re-export）の場合の扱いが複雑。「mod.rs に何を入れるか」のルール設計が必要:
- 選択肢 1: 全てを mod.rs に入れる → Rust 慣習に反する
- 選択肢 2: local 宣言は index.rs、re-export は mod.rs の `pub use` → index.ts を2つのファイルに分割する必要がある
- 選択肢 3: index.ts を index.rs としてそのまま変換し、mod.rs には `pub mod index; pub use index::*;` を生成 → `pub use *` で名前衝突のリスク

### 6.5 B vs C 星取表（詳細版）

| 評価観点 | B: 依存グラフ | C: モジュールツリー先行 | 判定根拠 |
|---------|:---:|:---:|---------|
| 基本的な import 解決の正確性 | ◎ | ◎ | 両方ともテーブル lookup。差なし |
| re-export チェーン解決 | ◎ | ○ | B は自前で完全解決。C は `pub use` 経由で Rust コンパイラが解決するが、`pub use` が正しく生成される前提 |
| `export *` の扱い | △ | ○ | B は元ファイルの全 export 列挙が必要（実装コスト高）。C は `pub use module::*;` を生成するだけ |
| 混合 index.ts の扱い | ○ | △ | B は通常ファイルとして変換（問題なし）。C は分割ルールの設計が必要 |
| 生成コードの Rust らしさ | ◎ | ○ | B は定義元への直接参照。C は re-export 経由の間接参照が残る |
| mod.rs の Rust 慣習準拠 | ◎ | △ | B の mod.rs は `pub mod` のみ。C は `pub use` + 場合により本体コード |
| 循環 re-export への対応 | △ | ◎ | B は自前で検出・処理が必要。C は Rust コンパイラに委ねる |
| 実装の複雑さ | △ | ○ | B は re-export チェーン解決 + 循環検出。C はテーブル構築 + `pub use` 生成 |
| TS の構造保存度 | △ | ◎ | B は TS の re-export 構造を圧縮（直接参照化）。C は TS の構造を Rust に 1:1 対応 |
| 外部パッケージ対応の拡張性 | ◎ | ○ | B はグラフにノード追加で対応可能。C はテーブルに手動マッピング追加 |
| tsconfig 由来の解決（baseUrl/paths 等） | ◎ | △ | 後述 6.6 節参照 |
| デバッグ容易性 | △ | ○ | B はグラフ経路のトレースが必要。C は構造的対応が明確 |

### 6.6 tsconfig 由来のモジュール解決への対応

#### 問題の整理

TypeScript では、ファイルとディレクトリ構造だけでは import 元が確定しないケースがある:

| tsconfig 機能 | 効果 | 例 |
|--------------|------|-----|
| `baseUrl` | 非相対 import のルートディレクトリを指定 | `baseUrl: "src"` → `import { Foo } from "utils/foo"` が `src/utils/foo.ts` に解決 |
| `paths` | パスエイリアスを定義 | `"@/*": ["src/*"]` → `import { Foo } from "@/utils/foo"` が `src/utils/foo.ts` に解決 |
| `rootDirs` | 複数ディレクトリを論理的に1つのルートとして扱う | `rootDirs: ["src", "generated"]` → 両ディレクトリが同一ルートとして解決 |
| `moduleResolution` | 解決アルゴリズムの選択 | `"bundler"` では拡張子省略可、`"node16"` では ESM で拡張子必須 |

これらの機能が使われている場合、`import { Foo } from "utils/foo"` のようなパスは相対パスではないため、現在の実装（`./` / `../` で始まるかの判定）ではスキップされる。

#### 各手法の対応力

**手法 A（現在の逐次変換）**:
- `convert_relative_path_to_crate_path` は `./` / `../` 始まりのみを処理し、非相対パスはスキップ（`None` を返す）
- baseUrl/paths によるパス解決は一切行われない
- **対応力: ×** — 構造的に対応不可能。文字列操作関数にプロジェクト設定を渡す術がない

**手法 B（依存グラフ）**:
- Pass 0 で tsconfig.json を読み込み、baseUrl/paths の設定を `ModuleGraph` の構築時に使用できる
- 非相対 import `import { Foo } from "utils/foo"` に対して、baseUrl を基準にファイルを探索し、ファイルパス → モジュールパスの変換テーブルで解決
- paths エイリアスも同様: `@/utils/foo` → tsconfig の paths 設定でファイルパスを解決 → テーブル lookup
- **対応力: ◎** — グラフ構築の入力として tsconfig を自然に組み込める。解決ロジックは `ModuleGraph` の内部に閉じ込められ、後段の変換処理には影響しない

```rust
// 手法 B の設計イメージ
struct ModuleGraphBuilder {
    base_url: Option<PathBuf>,       // tsconfig.baseUrl
    path_aliases: HashMap<String, Vec<String>>,  // tsconfig.paths
    root_dirs: Vec<PathBuf>,         // tsconfig.rootDirs
}

impl ModuleGraphBuilder {
    fn resolve_import(&self, specifier: &str, from_file: &Path) -> Option<PathBuf> {
        if specifier.starts_with("./") || specifier.starts_with("../") {
            // 相対パス解決
            self.resolve_relative(specifier, from_file)
        } else if let Some(resolved) = self.resolve_path_alias(specifier) {
            // paths エイリアス解決
            Some(resolved)
        } else if let Some(base) = &self.base_url {
            // baseUrl 解決
            self.resolve_from_base(specifier, base)
        } else {
            None // 外部パッケージ
        }
    }
}
```

**手法 C（モジュールツリー先行）**:
- Pass 0 でディレクトリ構造を走査してモジュールツリーを構築するが、このとき baseUrl/paths の情報がないと非相対 import を正しく配置できない
- tsconfig を読み込んでパスマッピングテーブルを構築すること自体は可能だが、手法 C の設計思想（「ディレクトリ構造からモジュールツリーを構築」）と tsconfig の設定（「ディレクトリ構造を仮想的に変更する」）が本質的に衝突する
- 例: `baseUrl: "src"` で `import { Foo } from "utils/foo"` の場合、`utils/foo.ts` は物理的には `src/utils/foo.ts` にあるが、import 元ファイルが `src/adapter/bun/server.ts` だとすると、相対パスに変換してからディレクトリ構造と突き合わせる必要がある。これは実質的に手法 B のパス解決と同じ処理
- **対応力: △** — 対応可能だが、tsconfig 解決を組み込むとモジュールツリーの「ディレクトリ構造ベース」の前提が崩れ、手法 B に近づく

**手法 D（ハイブリッド）**:
- 現在の `convert_relative_path_to_crate_path` に tsconfig 情報を追加パラメータで渡すことは可能だが、非相対パスのスキップ判定を変更する必要がある
- **対応力: ▲** — 場当たり的な対応は可能だが、パス解決ロジックが肥大化する

#### 実プロジェクトでの baseUrl/paths の使用頻度

baseUrl/paths は多くの TS プロジェクトで使用されている。特に:
- **monorepo**: `baseUrl` で内部パッケージの import を短縮
- **大規模 SPA**: `paths` で `@/components/*` のようなエイリアスを定義
- **Next.js/Nuxt.js 等のフレームワーク**: デフォルトで `paths` が設定される

Hono 自体は使っていないが、ts_to_rs が変換対象とするプロジェクトでは高い確率で遭遇する。

#### 影響のまとめ

tsconfig 対応は手法 B の設計に自然に組み込めるが、手法 C では「ディレクトリ構造 = モジュール構造」の前提が崩れるため、追加のパス解決レイヤーが必要になる。この追加レイヤーは実質的に手法 B のパス解決と同等であり、手法 C の「実装がシンプル」という利点を相殺する。
