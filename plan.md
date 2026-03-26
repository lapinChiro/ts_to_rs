# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

- **I-270: 参照されるビルトイン型の struct 定義自動生成** → `backlog/I-270-builtin-type-struct-generation.md`
  - 進捗: T1〜T4 + derive 修正完了。36 → **26 エラー**（10 件解消）
  - 残: T3（type_params 抽出 — 7 件）→ T5（インポート + スタブ — 19 件）→ T6（検証）
  - 詳細タスク: `tasks.md` 参照
- **I-192: 大規模ファイルの分割** → `backlog/I-192-large-file-splitting.md`
  - 1000 行超の全 18 ファイルを分割。プロダクション 6 + テスト 5 + テスト抽出 7
  - T1〜T13 の段階的タスク。各タスクは独立して実行可能（T*b のみ親タスクに依存）

## 設計判断の記録: I-270

### アプローチ選定

3 つのアプローチ（A: stub struct 自動生成、B: 型エイリアスマッピング、C: serde_json::Value 型消去）を検討し、**アプローチ A の強化版**（JSON フィールド情報に基づく struct 生成）を採用。

- **B を不採用にした理由**: 外部クレート依存（chrono 等）が発生し、マッピング定義が主観的。変換器の責務を超える
- **C を不採用にした理由**: 型安全性の喪失
- **A を採用した理由**: JSON に既にフィールド情報が豊富に存在。生成コードが自己完結しコンパイル可能

### types.rs 36 エラーの根本原因分析（`report/i-270-types-rs-errors-analysis.md`）

| カテゴリ | 件数 | 根本原因 | 修正 |
|---|---|---|---|
| A: 不正フィールド名 | 6 | generator の `escape_ident` がハイフン・ブラケット未対応 | T1: フィールド名サニタイズ |
| B: 外部型除外リスト + 推移的依存 | 5 | `Uint8Array` 誤除外 + 1 回のみの走査 | T2: 除外修正 + 固定点計算 |
| C: type_params 未抽出 | 10 | 抽出スクリプトが interface の型パラメータを無視 | T3: 抽出パイプライン改修 |
| D: 再帰型 | 1 | `Function.caller: Function` に Box なし | T4: 自己参照検出 + Box |
| E: types.rs インポート不在 | 6 | OutputWriter がインポートを生成しない | T5: インポート + スタブ生成 |
| F: 未定義型（Hono 固有 + 型パラメータ） | 8 | 変換未成功の型 + 型パラメータ漏出 | T5: スタブ + ジェネリクス化 |

### TypeRegistry.is_external の導入

外部型（JSON 由来）とユーザー定義型（TS ソース由来）を TypeRegistry レベルで区別する。`register_external` で登録された型のみ `is_external() == true`。外部 struct 生成はこの判定に基づき、ユーザー定義型の不要な struct 生成を防止する。

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

### I-270 実装中（設計判断の記録）

- **TypeRegistry.is_external**: 外部型トラッキングを TypeRegistry に内蔵。`register_external` / `is_external` メソッド。`external_types: HashSet<String>` フィールド。`merge` で伝播
- **transpile_with_builtins**: ビルトイン型付きの公開 API を `lib.rs` に追加。統合テストで使用
- **外部 struct の配置**: per-file の `all_items` と共有 `synthetic_items` の両方で外部型 struct を生成。固定点計算で推移的依存を解決（`generate_external_structs_to_fixpoint`）
- **camel_to_snake**: 外部型フィールド名を camelCase → snake_case 変換。略語対応（`toISOString` → `to_iso_string`）
- **sanitize_field_name**: ハイフン→アンダースコア、ブラケット除去、`_` → `_field`。generator の struct フィールド生成で使用
- **再帰型 Box ラップ**: `generate_external_struct` が自己参照フィールドを `Box<T>` でラップ。`references_type_name` で検出
- **is_derivable_type 修正**: `RustType::Any`（= `serde_json::Value`）は Debug/Clone/PartialEq を実装しているため derivable に変更。旧実装は Fn/DynTrait と一緒に false にしていたがバグ

### I-223/I-227 完了済み（設計判断の記録）

- **`variant_name_for_type` の統一**: `type_converter.rs` にあった重複実装 `variant_name_from_type` を削除し、`synthetic_registry.rs` の `variant_name_for_type` を `pub(crate)` にして一本化
- **文字列エスケープの 2 箇所**: `Expr::StringLit` の出力と `generate_macro_call` 内の両方で `escape_rust_string` を適用
- **ディレクトリコンパイル 156/158 の残り**: `types.rs` は I-270 で対応中

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
7. `instanceof-builtin` — I-270（メソッド impl 不在。struct 定義は生成済み）
8. `external-type-struct` — I-270（ビルトイン型読み込みが必要。compile_test は builtins なしで実行）

### I-112c Phase 1-3 実装の技術的詳細

- TypeResolver が per-file `SyntheticTypeRegistry` を使用（`fork_dedup_state` で共有レジストリから dedup 情報を引き継ぎ）
- 匿名構造体は `register_synthetic_structs_in_registry()` で TypeRegistry に転写
- return 文の expected type は `resolve_expr` **前**に設定
- 部分解決フィルタ: 全フィールドの型が解決できない場合は匿名構造体を生成しない
