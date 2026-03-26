# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

（backlog/ が空。次セッションで I-270 の PRD 化から開始）

## 次セッションの検討事項: I-270 ビルトイン型 struct 定義の生成

### 背景

I-223/I-227 の解消後、ディレクトリコンパイル 156/158 の残り 1 ファイル（`types.rs`）の原因を調査した結果、`ArrayBuffer`/`ArrayBufferView` 等の Web API 型が変換出力に struct 定義として存在しないことが判明した。

現在の外部型（`web_api.json`/`ecmascript.json`）は **TypeRegistry への型情報登録のみ** に使われ、メソッドの戻り値型やパラメータ型の解決に利用される。しかし、union enum のバリアントが `ArrayBuffer(ArrayBuffer)` のように外部型を直接参照する場合、Rust 側に対応する型定義が存在しないためコンパイルエラーになる。

### 影響範囲

- **直接影響**: ディレクトリコンパイル `types.rs` のエラー（`ArrayBuffer`, `ArrayBufferView` 未定義）
- **同根の問題**: コンパイルテスト `instanceof-builtin` のスキップ原因（`Date`/`Error`/`RegExp` の struct 定義不在）
- **潜在影響**: 外部型を直接参照する union/struct フィールドが今後増えると、同じ問題が拡大する

### 検討すべき設計方針

以下の 3 つのアプローチが考えられ、トレードオフの検討が必要:

#### A: 参照される外部型の stub struct 自動生成

変換出力で参照される外部型（union バリアント・struct フィールド等）を走査し、対応する struct 定義がない場合に stub（空の struct + derive）を自動生成する。

- **利点**: 型の存在を保証でき、ユーザーが後から実装を埋められる
- **課題**: どこまで stub を生成するか（フィールドなし? メソッドなし? trait impl は?）。stub が多すぎると「コンパイルは通るが何もできない型」が大量に生成される
- **実装箇所**: Generator（`src/generator/mod.rs`）が出力時に未定義型を検出し、ファイル末尾に stub を追加。または pipeline 段階で SyntheticTypeRegistry に登録

#### B: 外部型を型エイリアスにマッピング

`ArrayBuffer` → `type ArrayBuffer = Vec<u8>`、`Date` → `type Date = chrono::DateTime<chrono::Utc>` のように、Rust の適切な型にマッピングする。

- **利点**: 意味的に正しい変換になる。生成コードが実用的
- **課題**: マッピングの定義が必要（JSON に追加? ハードコード?）。全ての Web API 型に対してマッピングを用意するのは現実的でない。`chrono` 等の外部クレートへの依存が発生（I-30 Cargo.toml 依存追加が前提）
- **実装箇所**: `src/external_types.rs` または新規の型マッピング定義ファイル

#### C: union バリアントの外部型を serde_json::Value に型消去

union enum のバリアントが未定義の外部型を参照する場合、`ArrayBuffer(ArrayBuffer)` → `ArrayBuffer(serde_json::Value)` のように型消去する。

- **利点**: 最小変更でコンパイルが通る。外部型の意味情報は enum バリアント名に保持される
- **課題**: 型安全性が失われる。ユーザーが手動で型を復元する必要がある
- **実装箇所**: `src/pipeline/synthetic_registry.rs` の `register_union` または `type_converter.rs` の `convert_union_type`

### 推奨調査手順

1. Hono ベンチマークの `types.rs` で実際にどの外部型が参照されているか全数調査
2. コンパイルテスト `instanceof-builtin` の具体的なエラーを確認し、同根か検証
3. 上記 3 アプローチのうちどれが「理想的でクリーンな実装」かを判断（組み合わせの可能性も含む）
4. PRD 化して backlog/ に配置

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

### 効果予測

| 段階 | 累積解消 | 残 object literal エラー |
|---|---|---|
| Phase 1-3 + I-211 実装済み | 18 件 | 52 件 |
| I-224 完了後 | 21-23 件 | 47-49 件 |
| I-266 完了後 | 41-43 件 | 27-29 件 |
| I-268 + I-269 完了後 | 55-61 件 | 9-15 件 |
| I-267 完了後 | 65-70 件 | 0-5 件 |

## 引継ぎ事項

### I-223/I-227 完了済み（設計判断の記録）

- **`variant_name_for_type` の統一**: `type_converter.rs` にあった重複実装 `variant_name_from_type` を削除し、`synthetic_registry.rs` の `variant_name_for_type` を `pub(crate)` にして一本化。パス区切り `::` を含む型名（`serde_json::Value` 等）は `rsplit_once("::")` で最後のセグメントのみを抽出
- **文字列エスケープの 2 箇所**: `Expr::StringLit` の出力（`generate_expr`）と `generate_macro_call` 内の単一文字列引数ショートパスの両方で `escape_rust_string` を適用。SWC の `Str.value` はデコード済みのため、Generator が Rust ソースのエスケープを担う
- **ディレクトリコンパイル 156/158 の残り**: `types.rs` の `ArrayBuffer`/`ArrayBufferView` 未定義は I-270 として新規追跡。I-227 の `serde_json::Value` 混入は解消済み

### I-211 完了済み（設計判断の記録）

- **`RustType::Union` を IR に追加しない**: 既存の `SyntheticTypeRegistry::register_union` 基盤を `external_types.rs` でも使い、union を合成 enum（`RustType::Named`）に変換する。IR 変更なし、Generator フォールバックなし、暫定策なし
- **外部型ローダーの API**: `load_builtin_types` と `load_types_json` の両方が `Result<(TypeRegistry, SyntheticTypeRegistry)>` を返す。`load_types_into` で複数 JSON を既存 TypeRegistry にマージ。`TranspileInput.base_synthetic` で pipeline にシード
- **オーバーロード解決**: 統一 `select_overload` 関数（5 段階: 単一 → 同一戻り値 → 引数数 → 引数型互換 → フォールバック）。`lookup_method_params` と `resolve_method_return_type` が同一関数を使用し、パラメータ型と戻り値型が常に同一シグネチャから取得される
- **JSON ファイル分割**: `src/builtin_types/web_api.json`（105 型）+ `ecmascript.json`（57 型）。ECMAScript → Web API の順で読み込み（後勝ちで Web API 版が優先）
- **抽出スクリプト**: `--ecmascript` / `--server-web-api` フィルタフラグ（排他的）。Symbol メソッド名 (`__@iterator@35` 等) は抽出時に除外
- **`ExternalTypeDef::Function`** は依然 `signatures.first()` のみ使用。`TypeDef::Function` が単一シグネチャ構造のため。関数オーバーロード対応が必要になった場合は `TypeDef::Function` の構造変更が必要

### コンパイルテストのスキップ（7 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-237 (toFixed 未対応) + I-238 (Display 未実装)
6. `array-builtin-methods` — I-217（filter/find closure の &f64 比較）+ I-265（find の Option 二重ラップ）
7. `instanceof-builtin` — I-270（ビルトイン型の struct 定義が変換出力に存在しない）

### I-112c Phase 1-3 実装の技術的詳細

- TypeResolver が per-file `SyntheticTypeRegistry` を使用（`fork_dedup_state` で共有レジストリから dedup 情報を引き継ぎ）
- 匿名構造体は `register_synthetic_structs_in_registry()` で TypeRegistry に転写（Transformer の `resolve_field_type()` が動作するため）
- `type_converter.rs` の `convert_type_lit_in_annotation` を `register_inline_struct` に統一済み
- return 文の expected type は `resolve_expr` **前**に設定（匿名構造体の不要な生成を防ぐ）
- 部分解決フィルタ: 全フィールドの型が解決できない場合は匿名構造体を生成しない（不完全な struct によるサイレント意味変更を防止）
