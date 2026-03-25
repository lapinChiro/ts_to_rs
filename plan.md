# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

1. **[I-211-a](backlog/i-211-a-overload-support.md)**: メソッドオーバーロード対応 + Union の合成 enum 変換
2. **[I-211-b](backlog/i-211-b-ecmascript-extraction.md)**: ECMAScript 標準型の抽出 + JSON ファイル分割
3. **[I-211-c](backlog/i-211-c-verification.md)**: 検証 + E2E テスト + ベンチマーク効果測定

全体計画: [I-211 全体](backlog/i-211-ecmascript-builtin-types.md)

## I-112c 完全解消ロードマップ

I-112c（object literal 型推定）の Phase 1-3 は実装済み（70→54 件）。残り 54 件を 0 件にするためのイシュー消化順序:

### 開発順序

| 順序 | イシュー | 解消見込み | 理由 |
|---|---|---|---|
| 1 | **I-211: ECMAScript 標準型追加** | 11 件 | ECMAScript 標準型の TypeRegistry 追加。メソッドオーバーロード対応 + Union の合成 enum 変換を含む。I-112c の型推定インフラが自動的に追加ケースをカバーする。レバレッジ最大。**PRD 作成済み（I-211-a/b/c）** |
| 2 | **I-224: TypeResolver の `this` 型解決** | 3-8 件 | クラスメソッド内の `this.field` / `this.method()` の型解決。コンストラクタ引数やメソッドチェーンの型推定精度向上 |
| 3 | **I-112c 追加改善: ジェネリクスパラメータのフィールド展開** | 15-20 件 | `E extends Env` のようなジェネリクス型パラメータをスプレッドソースとして展開。`TypeRegistry::instantiate` を活用し、ジェネリクス型のフィールド情報を取得 |
| 4 | **I-112c 追加改善: Optional 型スプレッドの unwrap** | 3-5 件 | `options?: CORSOptions` のスプレッドで `Option<CORSOptions>` → `CORSOptions` のフィールド展開 |

### 依存関係

```
I-211-a → I-211-b → I-211-c ─┐
                               ├──→ I-112c 追加改善（ジェネリクス）──→ I-112c 追加改善（Optional）
I-224 ────────────────────────┘
```

I-211 と I-224 は独立して実施可能。I-112c 追加改善はこれらの完了後に最大効果。

### 効果予測

| 段階 | 累積解消 | 残 object literal エラー |
|---|---|---|
| Phase 1-3 実装済み | 16 件 | 54 件 |
| I-211 完了後 | 27 件 | 43 件 |
| I-224 完了後 | 30-35 件 | 35-40 件 |
| I-112c 追加改善完了後 | 48-55 件 | 15-22 件 |

## 引継ぎ事項

### I-211 の設計判断

- **`RustType::Union` を IR に追加しない**: 既存の `SyntheticTypeRegistry::register_union` 基盤を `external_types.rs` でも使い、union を合成 enum（`RustType::Named`）に変換する。IR 変更なし、Generator フォールバックなし、暫定策なし
- **`load_builtin_types` の戻り値**: `Result<(TypeRegistry, SyntheticTypeRegistry)>` に変更。Pipeline で base synthetic を per-file synthetic に merge してシードする
- **JSON ファイル分割**: `src/builtin_types.json` → `src/builtin_types/web_api.json` + `ecmascript.json`。`web_api.json` の再抽出は不要（JSON は全シグネチャを保持済み、Rust ローダーの `first()` 除去で全シグネチャがロードされる）

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-237 (toFixed 未対応) + I-238 (Display 未実装)

### I-112c Phase 1-3 実装の技術的詳細

- TypeResolver が per-file `SyntheticTypeRegistry` を使用（`fork_dedup_state` で共有レジストリから dedup 情報を引き継ぎ）
- 匿名構造体は `register_synthetic_structs_in_registry()` で TypeRegistry に転写（Transformer の `resolve_field_type()` が動作するため）
- `type_converter.rs` の `convert_type_lit_in_annotation` を `register_inline_struct` に統一済み
- return 文の expected type は `resolve_expr` **前**に設定（匿名構造体の不要な生成を防ぐ）
- 部分解決フィルタ: 全フィールドの型が解決できない場合は匿名構造体を生成しない（不完全な struct によるサイレント意味変更を防止）

## 保留中

（なし）
