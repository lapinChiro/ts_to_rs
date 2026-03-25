# I-112c: オブジェクトリテラルの型推定

## 現状（2026-03-25 時点）

Phase 1-3 を実装済み。70 → 54 件に削減（-16 件、23% 解消）。

### 実装済み

- T1: call signature 型エイリアスを `TypeDef::Function` として登録（`registry.rs`）
- T2: スコープ内 Named 型関数変数の引数型・戻り値型伝搬（`type_resolver.rs`）
- T3: 匿名構造体自動生成 + TypeRegistry 転写（`type_resolver.rs`, `pipeline/mod.rs`）
- T4: パイプライン統合（per-file synthetic, `fork_dedup_state`, `register_inline_struct` 統一）
- T5: スプレッド構文のマージ型推定（`merge_object_fields`, `common_named_type`）
- T6: E2E テスト + スナップショットテスト
- 安全性修正: 部分解決フィルタ、スプレッド未解決フィルタ、`common_named_type` の `type_args` 比較
- return 文の expected type 設定順序修正（`resolve_expr` 前に設定）

### 残り 54 件の原因分析

| カテゴリ | 件数 | 原因 | 解決に必要なイシュー |
|---|---|---|---|
| 戻り値型注釈なし／型エイリアス未解決 | 20 | 関数の戻り値型が TypeRegistry で解決できない（`MiddlewareHandler` 等の複合型エイリアス） | I-112c 追加改善（TypeResolver 強化） |
| スプレッドソースの型未解決 | 18 | ジェネリクスパラメータ（`E extends Env`）や Optional 型のスプレッドソースの TypeRegistry フィールド情報不足 | I-112c 追加改善 + I-224（`this` 型） |
| MiddlewareHandler 戻り値 | 6 | middleware の内部関数から返される Response コンストラクタ引数 | I-211（ECMAScript 標準型） |
| `new Response({...})` 引数 | 5 | `Response` コンストラクタの引数型が TypeRegistry にない | I-211 |
| コンストラクタ引数 | 3 | `new SmartRouter({...})` 等、コンストラクタの引数型未解決 | I-224（`this` 型） |
| その他 | 2 | 複合的な原因 | 個別分析が必要 |

### 完了条件

object literal エラーが **0 件** に到達すること（段階的に関連イシューを解消）。

## 背景・動機

Hono ベンチマークで最大のエラーカテゴリ。当初 70 インスタンス（全 132 エラーの 53.0%）を占めていた。

`const obj = { key: value }` のように型注釈のないオブジェクトリテラルで、Rust の struct 名を推定できずにエラーになる。エラーメッセージ: `object literal requires a type annotation to determine struct name`。

Phase 1-3 の実装で TypeResolver の expected type 設定を強化し、匿名構造体の自動生成とスプレッドマージ型推定を追加した。残り 54 件の解消には以下の関連イシューの解決が必要:

1. **I-211（ECMAScript 標準型追加）**: `Response`, `ReadableStream` 等のコンストラクタ引数型を TypeRegistry に追加。11 件の解消が見込まれる
2. **I-224（`this` 型解決）**: クラスメソッド内の `this.field` 型解決。コンストラクタ引数やメソッドチェーンの型推定に影響。3-8 件の解消が見込まれる
3. **I-112c 追加改善**: TypeResolver のジェネリクス型パラメータ解決や Optional 型のフィールド展開。20-30 件の解消が見込まれる
