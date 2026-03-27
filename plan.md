# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 完了: I-192 大規模ファイルの分割

### ベースライン

- テスト数: 1369 (1225 + 3 + 2 + 63 + 76)
- 1000 行超ファイル: 元 18 個

### 完了済みタスク（T1-T13）

カテゴリ A（プロダクションコード分割）6 ファイル全完了 + テスト分割 10 ファイル完了:

| タスク | 元ファイル | 元行数 | サブモジュール数 |
|--------|-----------|--------|----------------|
| T1+T1b | `type_resolver.rs` | 3692 | 7 + tests/3 |
| T2 | `type_converter.rs` | 2691 | 6 + tests |
| T3+T3b | `statements/mod.rs` + `tests.rs` | 2656+2766 | 7 + tests/7 |
| T4+T4b | `registry.rs` | 2414 | 6 + tests/4 |
| T5 | `classes.rs` | 2215 | 5 + tests |
| T6+T6b | `functions/mod.rs` + `tests.rs` | 1298+1422 | 4 + tests/4 |
| T7 | `expressions/tests.rs` | 6814 | tests/19 (論理分類ベース) |
| T8 | `types/tests.rs` | 3333 | tests/7 |
| T9 | `transformer/tests.rs` | 1335 | tests/6 |
| T10 | `generator/` テスト抽出 | mod.rs:1410, expressions.rs:1267, statements.rs:1019 | 3ファイル分割 |
| T11 | `ir.rs` テスト抽出 | 1416 | ir/mod.rs:858 + ir/tests.rs:558 |
| T12 | テスト抽出 | external_types.rs:1156, external_struct_generator.rs:1132, module_graph.rs:1038 | 3ファイル分割 |
| T13 | 最終検証 | — | 全ファイル1000行以下、テスト1369不変、clippy 0警告、fmt pass |

全タスク完了。テスト数不変（1369）、1000行超ファイル 0 個。

### 再発防止

`scripts/check-file-lines.sh`（閾値 1000 行）を `/quality-check` スキルに組み込み済み。

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

設計判断の詳細は [doc/design-decisions.md](doc/design-decisions.md) を参照。

### コンパイルテストのスキップ（8 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-237 (toFixed 未対応) + I-238 (Display 未実装)
6. `array-builtin-methods` — I-217（filter/find closure の &f64 比較）+ I-265（find の Option 二重ラップ）
7. `instanceof-builtin` — I-270c（メソッド impl 不在。struct 定義は I-270 で生成済み）
8. `external-type-struct` — I-270（ビルトイン型読み込みが必要。compile_test は builtins なしで実行）
