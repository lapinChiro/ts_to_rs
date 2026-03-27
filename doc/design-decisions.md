# 設計判断の記録

各イシュー実装時の設計判断。後続イシューの実装時に「なぜこうなっているか」を追跡するための記録。

## I-270: ビルトイン型 struct 定義自動生成

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

## I-223/I-227: コンパイルエラーのクイックウィン

- **`variant_name_for_type` の統一**: `type_converter.rs` にあった重複実装 `variant_name_from_type` を削除し、`synthetic_registry.rs` の `variant_name_for_type` を `pub(crate)` にして一本化
- **文字列エスケープの 2 箇所**: `Expr::StringLit` の出力と `generate_macro_call` 内の両方で `escape_rust_string` を適用

## I-211: ECMAScript 標準型

- **`RustType::Union` を IR に追加しない**: 既存の `SyntheticTypeRegistry::register_union` 基盤を `external_types.rs` でも使い、union を合成 enum（`RustType::Named`）に変換する
- **外部型ローダーの API**: `load_builtin_types` と `load_types_json` の両方が `Result<(TypeRegistry, SyntheticTypeRegistry)>` を返す
- **オーバーロード解決**: 統一 `select_overload` 関数（5 段階）
- **JSON ファイル分割**: `src/builtin_types/web_api.json`（105 型）+ `ecmascript.json`（57 型）

## I-112c: オブジェクトリテラル型推定 Phase 1-3

- TypeResolver が per-file `SyntheticTypeRegistry` を使用（`fork_dedup_state` で共有レジストリから dedup 情報を引き継ぎ）
- 匿名構造体は `register_synthetic_structs_in_registry()` で TypeRegistry に転写
- return 文の expected type は `resolve_expr` **前**に設定
- 部分解決フィルタ: 全フィールドの型が解決できない場合は匿名構造体を生成しない

## コンパイルテストのスキップ（8 件）

各フィクスチャのスキップ理由と、解消に必要なイシュー:

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-237 (toFixed 未対応) + I-238 (Display 未実装)
6. `array-builtin-methods` — I-217（filter/find closure の &f64 比較）+ I-265（find の Option 二重ラップ）
7. `instanceof-builtin` — I-270c（メソッド impl 不在。struct 定義は I-270 で生成済み）
8. `external-type-struct` — I-270（ビルトイン型読み込みが必要。compile_test は builtins なしで実行）
