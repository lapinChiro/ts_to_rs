# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

（backlog/ に PRD なし。TODO から次の PRD を作成する）

## OBJECT_LITERAL_NO_TYPE 完全解消ロードマップ

I-112c Phase 1-3 + I-211 実装済み（70→52 件）。残り 52 件を 4 つのイシューに分解。

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
7. `instanceof-builtin` — ビルトイン型（Date/Error/RegExp）の struct 定義が変換出力に存在しない

### I-112c Phase 1-3 実装の技術的詳細

- TypeResolver が per-file `SyntheticTypeRegistry` を使用（`fork_dedup_state` で共有レジストリから dedup 情報を引き継ぎ）
- 匿名構造体は `register_synthetic_structs_in_registry()` で TypeRegistry に転写（Transformer の `resolve_field_type()` が動作するため）
- `type_converter.rs` の `convert_type_lit_in_annotation` を `register_inline_struct` に統一済み
- return 文の expected type は `resolve_expr` **前**に設定（匿名構造体の不要な生成を防ぐ）
- 部分解決フィルタ: 全フィールドの型が解決できない場合は匿名構造体を生成しない（不完全な struct によるサイレント意味変更を防止）
