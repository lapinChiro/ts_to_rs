# コンポーネント責務境界監査レポート

**Base commit**: `0f4a3c3`
**Date**: 2026-03-30

## サマリー

全コンポーネントの責務境界を精査した結果、**重大な境界違反は1件、軽微な設計観察は3件**見つかった。また README.md のディレクトリ構成記載に **陳腐化が5箇所**あった。

---

## 1. コンポーネント責務境界の監査結果

### 1.1 境界違反: registry → transformer の逆方向依存

**場所**: `src/registry/enums.rs:51-132`

`register_any_narrowing_enums()` と `register_any_narrowing_enums_from_expr()` が `crate::transformer::any_narrowing` の関数（`collect_any_constraints`, `build_any_enum_variants`, `collect_any_local_var_names`, `to_pascal_case`）をインポートしている。

これは設計上の **逆方向依存** である:
- パイプラインの正しい依存方向: `parser → registry → type_resolver → transformer → generator`
- registry が transformer に依存すると、循環的な結合リスクが生じる

**同時に** `src/pipeline/any_enum_analyzer.rs` も同じ `transformer::any_narrowing` を使って同様の処理を行っている。両者の違い:

| 観点 | `registry/enums.rs` | `pipeline/any_enum_analyzer.rs` |
|------|---------------------|--------------------------------|
| 呼び出し元 | `registry/collection.rs` (Pass 2: 型収集時) | `pipeline/mod.rs` (Pass 2.5: 型収集後) |
| 入力 | `TypeDef`（型定義登録済み関数の `any` パラメータ） | AST直接走査（型注釈から `any` パラメータを検出） |
| 出力先 | `TypeRegistry` のみ | `FileTypeResolution` + `SyntheticTypeRegistry` |
| スコープ情報 | なし（スコープ開始/終了を記録しない） | あり（AnyEnumOverride にスコープ位置を記録） |

つまり、**registry 側は TypeDef 経由で TypeRegistry に enum を登録**し、**pipeline 側は AST から直接スコープ付き Override を FileTypeResolution に登録**するという二重処理になっている。

**推奨**: `any_narrowing` のユーティリティ関数群（`collect_any_constraints`, `build_any_enum_variants` 等）を `transformer/` から `pipeline/` またはトップレベルの共有モジュールに移動し、依存方向を正す。registry 側の `register_any_narrowing_enums` が本当に必要か（pipeline 側の `analyze_any_enums` で代替可能か）を検証する。

---

### 1.2 観察: registry での型表現決定

**場所**:
- `src/registry/interfaces.rs`: optional フィールドの `Option<T>` ラップ
- `src/registry/functions.rs`: デフォルト引数の `Option<T>` ラップ

registry は本来「TS ソースから型メタデータを抽出するパッシブなストア」であるべきだが、`Option<T>` でラップするという **Rust 型表現の決定** が registry フェーズで行われている。

これは type_converter の責務範囲に属する変換だが、現在の設計では registry が TypeDef を RustType で保持するため、収集時に Rust 表現に変換する必要がある。TypeDef を TS 型のまま保持する設計にリファクタリングしない限り、現状の配置は実用上妥当。

**重要度**: 低（現設計の制約内で合理的）

---

### 1.3 観察: transformer/types/mod.rs が re-export のみ

**場所**: `src/transformer/types/mod.rs`

`pipeline::type_converter::*` を re-export するだけのモジュール。型変換ロジックが `pipeline/type_converter/` に移動した後の互換性レイヤー。

テストコード（`src/transformer/types/tests/`）が `transformer::types` 経由で型変換をテストしており、これらも `pipeline::type_converter` を直接テストすべき。

**重要度**: 低（技術的負債だが機能的問題なし）

---

### 1.4 観察: transformer/any_narrowing.rs の共有ユーティリティ的性質

**場所**: `src/transformer/any_narrowing.rs`

このモジュールは transformer と pipeline の両方から使われている:
- `pipeline/any_enum_analyzer.rs` から利用
- `registry/enums.rs` から利用
- transformer 自身からも利用

実質的に **共有ユーティリティ** であり、transformer に配置されていることで 1.1 の逆方向依存を引き起こしている。

---

## 2. 正常に機能している境界

以下のコンポーネントは責務境界が正しく設計・実装されている:

| コンポーネント | 責務 | 境界の状態 |
|---------------|------|-----------|
| **parser.rs** | TS ソース → SWC AST | ✅ 最小限・単一責務 |
| **ir/mod.rs** | 中間表現のデータモデル | ✅ 純粋なデータ定義、generator/transformer に依存なし |
| **generator/** | IR → Rust ソースコード | ✅ 純粋なテキスト生成、意味解析なし |
| **pipeline/mod.rs** | パイプライン全体のオーケストレーション | ✅ 各パスの調整に専念 |
| **pipeline/module_graph/** | import/export グラフ構築 | ✅ モジュール関係のみ、型意味論に踏み込まない |
| **pipeline/module_resolver.rs** | import specifier → ファイルパス解決 | ✅ ファイルシステムレベルの解決のみ |
| **pipeline/type_converter/** | TS 型注釈 → RustType 変換 | ✅ 型注釈変換に専念、ランタイム型推論なし |
| **pipeline/type_resolver/** | 式の型・期待型・narrowing 事前計算 | ✅ 不変の FileTypeResolution を生成 |
| **pipeline/type_resolution.rs** | FileTypeResolution データ構造 | ✅ 純粋なデータコンテナ |
| **pipeline/synthetic_registry.rs** | 合成型の重複排除レジストリ | ✅ 登録と重複排除に専念 |
| **pipeline/external_struct_generator/** | 未定義外部型の struct 自動生成 | ✅ gap 補完に専念 |
| **pipeline/output_writer.rs** | ファイル出力・mod.rs 生成 | ✅ 出力に専念 |
| **transformer/mod.rs** | AST + 型情報 → IR 変換 | ✅ generator 未使用、型変換は type_converter に委譲 |
| **transformer/context.rs** | 不変コンテキストコンテナ | ✅ 依存注入の整理 |
| **transformer/expressions/** | 式の AST → IR 変換 | ✅ 型解決は FileTypeResolution から読取のみ |
| **transformer/statements/** | 文の AST → IR 変換 | ✅ 可変性推論は post-IR 分析として適切 |
| **transformer/functions/** | 関数宣言の変換 | ✅ ローカル SyntheticRegistry でファイルレベル分離 |
| **transformer/classes/** | クラス宣言の変換 | ✅ pre-scan + generation の2パス設計 |
| **directory.rs** | ファイルシステムユーティリティ | ✅ TS/Rust 意味論に踏み込まない |
| **external_types/** | JSON 型定義 → TypeRegistry | ✅ transformer/generator に依存なし |
| **lib.rs** | ライブラリエントリポイント | ✅ pipeline のファサード |
| **main.rs** | CLI エントリポイント | ✅ IO・CLI 引数処理に専念 |

---

## 3. README.md 陳腐化チェック

### 3.1 修正が必要な箇所

| セクション | README の記載 | 実際 | 修正内容 |
|-----------|-------------|------|---------|
| ディレクトリ構成: fixtures | `74 件` | **84 件** | 件数更新 |
| ディレクトリ構成: scripts | 3 スクリプト | **5 ファイル** (`bench_categories.py`, `inspect-errors.py` が未記載) | 追加 |
| ディレクトリ構成: pipeline | `type_converter/` の記載が 1 行 | 実際は **サブモジュール 6 ファイル**（`indexed_access.rs`, `interfaces.rs`, `intersections.rs`, `type_aliases.rs`, `unions.rs`, `utilities.rs`） | 記載の粒度は現状で妥当（省略可） |
| ディレクトリ構成: type_resolver | `type_resolver/` の記載が 1 行 | 実際は **サブモジュール 7 ファイル**（`call_resolution.rs`, `du_analysis.rs`, `expected_types.rs`, `expressions.rs`, `helpers.rs`, `narrowing.rs`, `visitors.rs`） | 同上 |
| ディレクトリ構成: transformer | `any_narrowing.rs` が未記載 | 実際に存在 | 追加 |
| ディレクトリ構成: transformer/expressions | 多くのサブモジュールが未記載 | `assignments.rs`, `binary.rs`, `calls.rs`, `data_literals.rs`, `functions.rs`, `literals.rs`, `member_access.rs`, `methods.rs`, `patterns.rs`, `type_resolution.rs` | 記載の粒度は現状で妥当（省略可） |

### 3.2 修正推奨（必須）

1. **fixture 件数**: `74 件` → `84 件`
2. **scripts/ ディレクトリ**: `bench_categories.py` と `inspect-errors.py` を追加
3. **transformer/ ディレクトリ**: `any_narrowing.rs` を追加
4. **pipeline/ ディレクトリ**: `type_resolution.rs` を追加（FileTypeResolution は重要なデータ構造であり記載すべき）

### 3.3 変換テーブルの正確性

変換テーブルの内容は現在の実装と整合している。新たに追加された機能で README に反映されていないものは確認されなかった。

---

## 4. 推奨アクション

### 優先度: 高

1. **`transformer/any_narrowing.rs` を `pipeline/` に移動**
   - 現在 registry と pipeline の両方から使われている共有ユーティリティ
   - transformer に置くことで registry → transformer の逆方向依存が発生
   - `pipeline/any_narrowing.rs`（ユーティリティ部分）として独立させ、`pipeline/any_enum_analyzer.rs` と統合を検討

### 優先度: 中

2. **registry 側の any-narrowing enum 登録が pipeline 側と重複していないか検証**
   - registry 側は TypeRegistry に登録、pipeline 側は FileTypeResolution + SyntheticTypeRegistry に登録
   - 両方が必要な理由を doc comment で明文化するか、一方に統合する

### 優先度: 低

3. **README.md の陳腐化修正**（本レポートのセクション 3.2 参照）
4. **`transformer/types/mod.rs` の re-export 層の解消**（テストの直接参照化を含む）
