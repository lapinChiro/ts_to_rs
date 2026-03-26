# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 現在のタスク: I-192 大規模ファイルの分割

PRD: `backlog/I-192-large-file-splitting.md`

### ベースライン

- テスト数: 1293 (1225 + 3 + 2 + 63)
- 1000 行超ファイル: 元 18 個

### 完了済みタスク

- **T1: `type_resolver.rs` → `type_resolver/` ディレクトリ化**
  - `type_resolver.rs` (3692行) を 7 サブモジュールに分割
  - `mod.rs` (146), `visitors.rs` (431), `narrowing.rs` (146), `expected_types.rs` (194), `expressions.rs` (976), `du_analysis.rs` (221), `helpers.rs` (243)
  - テスト全 pass、外部 API パス不変
- **T1b: `type_resolver` テスト分割**
  - `tests.rs` (1405行) を `tests/` ディレクトリに 3 サブモジュールに分割
  - `tests/mod.rs` (93), `tests/basics.rs` (425), `tests/expected_types.rs` (434), `tests/complex_features.rs` (453)
  - テスト 65 個全 pass、数不変
- **T2: `type_converter.rs` → `type_converter/` ディレクトリ化**
  - `type_converter.rs` (2691行) を 6 サブモジュールに分割
  - `mod.rs` (289), `interfaces.rs` (433), `intersections.rs` (325), `type_aliases.rs` (518), `unions.rs` (585), `utilities.rs` (467), `tests.rs` (95)
  - PRD 設計（8 ファイル）から実装時に最適化: `annotations.rs` → `intersections.rs`/`type_aliases.rs` に統合、`helpers.rs`（1関数のみ）→ `unions.rs` に統合、`utility_types.rs` → `utilities.rs` に名称変更し共通ヘルパーも含む
  - テスト 4 個全 pass、全テスト pass
- **T3: `statements/mod.rs` サブモジュール分割**
  - `statements/mod.rs` (2656行) を 7 サブモジュールに分割
  - `mod.rs` (198), `control_flow.rs` (753), `switch.rs` (727), `error_handling.rs` (294), `spread.rs` (202), `destructuring.rs` (239), `mutability.rs` (132), `helpers.rs` (180)
  - PRD 設計では `convert_nested_fn_decl` を mod.rs に残す想定だったが、関数本体のネスト変換は制御フローの責務であるため control_flow.rs に配置
  - テスト全 pass、外部 API パス不変
- **T3b: `statements/tests.rs` テスト分割**
  - `tests.rs` (2766行) を `tests/` ディレクトリに 7 サブモジュールに分割
  - `tests/mod.rs` (105), `tests/variables.rs` (448), `tests/control_flow.rs` (319), `tests/loops.rs` (261), `tests/destructuring.rs` (439), `tests/switch.rs` (590), `tests/error_handling.rs` (498), `tests/expected_types.rs` (107)
  - テスト 96 個全 pass、数不変
- **T4: `registry.rs` → `registry/` ディレクトリ化**
  - `registry.rs` (2414行) を 6 サブモジュール + tests に分割
  - `mod.rs` (350), `collection.rs` (400), `interfaces.rs` (98), `unions.rs` (194), `functions.rs` (177), `enums.rs` (132), `tests.rs` (1131)
  - PRD 設計どおりの分割。全テスト pass、外部 API パス不変
  - tests.rs が 1131 行で 1000 行超のため T4b が必要

### 次のタスク（上から順に実施）

1. **T4b: `registry` テスト分割** — T4 で抽出した `tests.rs` (1131行) が 1000 行超のため分割が必要
5. **T5: `classes.rs` → `classes/` ディレクトリ化** — `classes.rs` (2215行) を 5 サブモジュールに分割
6. **T6: `functions/mod.rs` サブモジュール分割** — `functions/mod.rs` (1298行) を 4 サブモジュールに分割
7. **T6b: `functions/tests.rs` テスト分割** — `tests.rs` (1422行) を `tests/` ディレクトリに分割。T6 に依存
8. **T7: `expressions/tests.rs` テスト分割** — `tests.rs` (6814行) を 15 サブモジュールに分割
9. **T8: `types/tests.rs` テスト分割** — `tests.rs` (3333行) を 7 サブモジュールに分割
10. **T9: `transformer/tests.rs` テスト分割** — `tests.rs` (1335行) を 7 サブモジュールに分割
11. **T10: `generator/` テスト抽出** — `mod.rs` (1410行), `expressions.rs` (1267行), `statements.rs` (1019行) のインラインテストを別ファイルに抽出
12. **T11: `ir.rs` テスト抽出** — `ir.rs` (1416行) → `ir/mod.rs` + `ir/tests.rs`
13. **T12: `pipeline/` テスト抽出** — `external_types.rs` (1156行), `module_graph.rs` (1038行), `external_struct_generator.rs` (1132行) のテスト抽出
14. **T13: 最終検証** — 全ファイル 1000 行以下、全テスト pass、clippy 0 警告、fmt pass、Hono ベンチ同一

### 作業上の注意事項

- **並列エージェント禁止**: 同一リポジトリで複数エージェントが同時にファイル操作すると破壊が起きた。全タスクを直列で実施する
- **スクリプトによる一括置換禁止**: sed/Python の一括置換でミスが発生した。手動で正確に編集する
- **分割パターン**: サブモジュールに `use super::*;` で親の名前空間を取り込み、サブモジュール間で呼ばれる関数は `pub(super)`。外部公開 API は mod.rs で `pub use submodule::func;` で re-export
- **検証**: 各タスク完了後に `cargo check` + `cargo test` でテスト数不変を確認。`cargo fmt` も実行する

## OBJECT_LITERAL_NO_TYPE 完全解消ロードマップ

I-112c Phase 1-3 + I-211 実装済み（70→53 件）。残り 53 件を 4 つのイシューに分解。

### 開発順序

| 順序 | イシュー | 解消見込み | 理由 |
|---|---|---|---|
| 1 | **I-224: `this` 型解決** | 3-5 件 | クラスメソッド内の `this.field` / `this.method()` の型解決。独立して実施可能 |
| 2 | **I-266: 関数引数 expected type** | ~20 件 | シグネチャのパラメータ型から expected type を逆引き。最大効果 |
| 3 | **I-268: ジェネリクスフィールド展開** | ~14 件 | `E extends Env` の制約型からフィールド展開 |
| 4 | **I-269: Optional スプレッド unwrap** | 4 件 | `Option<T>` → `T` のフィールド展開。I-268 と同じ基盤 |
| 5 | **I-267: return/new 型逆引き** | ~10 件 | コンストラクタ引数は I-266 で解消。残りは戻り値型からの逆引き |

### 依存関係

```
I-224（独立）─────────────────────────┐
I-266（関数引数 expected type）───────├──→ I-267（return/new、I-266 の拡張）
I-268（ジェネリクス展開）─→ I-269 ───┘
```

## 引継ぎ事項

### I-270 完了済み（設計判断の記録）

- **TypeRegistry.is_external**: 外部型トラッキングを TypeRegistry に内蔵。`register_external` / `is_external` メソッド。`external_types: HashSet<String>` フィールド。`merge` で伝播
- **transpile_with_builtins**: ビルトイン型付きの公開 API を `lib.rs` に追加。統合テストで使用
- **外部 struct の配置**: per-file の `all_items` と共有 `synthetic_items` の両方で外部型 struct を生成。固定点計算で推移的依存を解決（`generate_external_structs_to_fixpoint`）
- **sanitize_field_name / camel_to_snake**: `ir.rs` に配置（pub 関数）。IR の StructField.name は常に有効な Rust 識別子を保持する不変条件を確立。generator は `escape_ident` のみ使用
- **type_params 外部型パイプライン**: TypeScript エクストラクタで `node.typeParameters` を抽出 → JSON `type_params` フィールド → Rust `ExternalTypeParam` → `TypeDef::new_interface` に `Vec<TypeParam>` を渡す。FORMAT_VERSION: 1 → 2
- **generate_stub_structs**: 固定点ループ（最大 10 回）で未定義型にスタブ struct を生成。`collect_all_undefined_references` は `is_external` フィルタなし、インポート済み型・型パラメータ・パス形式型名を除外
- **types.rs インポート生成**: `output_writer.rs` の `generate_types_rs_imports` が `serde_json::` → `use serde_json;`, `HashMap<` → `use std::collections::HashMap;` を自動追加
- **synthetic placement の相互参照**: `resolve_synthetic_placement` が他の synthetic item から参照されるスタブを shared_module に配置
- **再帰型 Box ラップ**: `generate_external_struct` が自己参照フィールドを `Box<T>` でラップ。`references_type_name` で検出
- **is_derivable_type 修正**: `RustType::Any`（= `serde_json::Value`）は Debug/Clone/PartialEq を実装しているため derivable に変更
- **残課題**: types.rs に 5 件の trait/struct 混同エラー（I-273）、フィールドアクセス名の不一致（I-274）、O(n²) 性能問題（I-275）

### I-223/I-227 完了済み（設計判断の記録）

- **`variant_name_for_type` の統一**: `type_converter.rs` にあった重複実装 `variant_name_from_type` を削除し、`synthetic_registry.rs` の `variant_name_for_type` を `pub(crate)` にして一本化
- **文字列エスケープの 2 箇所**: `Expr::StringLit` の出力と `generate_macro_call` 内の両方で `escape_rust_string` を適用

### I-211 完了済み（設計判断の記録）

- **`RustType::Union` を IR に追加しない**: 既存の `SyntheticTypeRegistry::register_union` 基盤を `external_types.rs` でも使い、union を合成 enum（`RustType::Named`）に変換する
- **外部型ローダーの API**: `load_builtin_types` と `load_types_json` の両方が `Result<(TypeRegistry, SyntheticTypeRegistry)>` を返す
- **オーバーロード解決**: 統一 `select_overload` 関数（5 段階）
- **JSON ファイル分割**: `src/builtin_types/web_api.json`（105 型）+ `ecmascript.json`（57 型）

### コンパイルテストのスキップ（8 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-237 (toFixed 未対応) + I-238 (Display 未実装)
6. `array-builtin-methods` — I-217（filter/find closure の &f64 比較）+ I-265（find の Option 二重ラップ）
7. `instanceof-builtin` — I-270c（メソッド impl 不在。struct 定義は I-270 で生成済み）
8. `external-type-struct` — I-270（ビルトイン型読み込みが必要。compile_test は builtins なしで実行）

### I-112c Phase 1-3 実装の技術的詳細

- TypeResolver が per-file `SyntheticTypeRegistry` を使用（`fork_dedup_state` で共有レジストリから dedup 情報を引き継ぎ）
- 匿名構造体は `register_synthetic_structs_in_registry()` で TypeRegistry に転写
- return 文の expected type は `resolve_expr` **前**に設定
- 部分解決フィルタ: 全フィールドの型が解決できない場合は匿名構造体を生成しない
