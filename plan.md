# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

1. **I-226: TypeEnv の完全除去** — `backlog/i226-typeenv-removal.md`

### I-226 実行計画

第 1 波（T2, T3, T7）、第 2 波（T4, T5, T6）、第 3 波前半（T8）完了。

#### 依存グラフ

```
T1 ✅
├─ T2 ✅ ───→ T6 ✅ ┐
├─ T3 ✅ → T4 ✅ ────┤
└─ T7 ✅ → T5 ✅ ────┤
                      ↓
                     T8 ✅ → T9 ─┐
                     T10 ────────┤
                                 ↓
                                T11
```

#### 第 1 波（完了: T2, T3, T7）

| タスク | 状態 | 主な変更 |
|---|---|---|
| T2 | ✅ | `DuFieldBinding` + `is_du_field_binding()` + TypeResolver DU switch 検出 |
| T3 | ✅ | `visit_var_decl` で Fn 型を変数 Ident スパンにも `expr_types` に記録 |
| T7 | ✅ | AnyTypeAnalyzer をパイプラインレベルに移動。`any_enum_overrides` を `FileTypeResolution` に格納。TypeResolver が `declare_var` 時に Any → enum 型で置換。パラメータ・Any enum 関連の type_env 操作を全除去 |

#### 第 2 波（完了: T4, T5, T6）

| タスク | 状態 | 主な変更 |
|---|---|---|
| T4 | ✅ | `statements/mod.rs` の insert 6 箇所 + `calls.rs` の get 1 箇所を除去。`infer_fn_type_from_closure` / `extract_var_decl_init` 削除 |
| T5 | ✅ | `expressions/mod.rs` の push_scope/insert/pop_scope 除去。`patterns.rs` の type_env フォールバック除去（`get_type_for_var` 一本化）。`narrowed_type_for_then/else` / `typeof_string_to_rust_type` 削除 |
| T6 | ✅ | `statements/mod.rs` の DU switch スコープ操作除去。`member_access.rs` を `is_du_field_binding` に置換 |

#### 第 3 波前半（完了: T8）

| タスク | 状態 | 主な変更 |
|---|---|---|
| T8 | ✅ | `Transformer` struct から `type_env` フィールド除去。全 sub-transformer 構築箇所から type_env を除去。`type_resolver.rs` の `P-1`〜`P-6` レガシーコメントを修正 |

#### 次の作業: 第 3 波後半（T10）→ 第 4 波（T9, T11）

| タスク | 依存 | 主な変更対象 |
|---|---|---|
| T10 | T4-T8 ✅ | テストコードの TypeEnv 構築・操作を除去 |
| T9 | T8 ✅ | TypeEnv 構造体の削除（`type_env.rs` から TypeEnv struct 除去） |
| T11 | T9, T10 | 品質チェック + ベンチマーク |

## 引継ぎ事項

### 第 2 波 + T8 完了時の状態

- **production code に `type_env` の使用箇所はゼロ**。Transformer struct から `type_env` フィールドが除去済み
- テストコード（`expressions/tests.rs`, `statements/tests.rs`, `tests.rs`）にはまだ `type_env` 参照が残存 → T10 で除去
- `mod.rs:12,15` の `pub(crate) mod type_env` / `pub use type_env::TypeEnv` はテストコードが参照中のため残存 → T9（T10 完了後）で除去
- `NarrowingGuard` の `narrowed_type_for_then/else` メソッドおよび `typeof_string_to_rust_type` は T5 で削除済み（TypeResolver の narrowing_events が position-based で narrowed type を提供するため不要）
- `type_resolver.rs` の `propagate_expected` 内の `P-1`〜`P-6` ラベルを説明的なコメントに修正済み

### 第 1 波完了時の状態

- AnyTypeAnalyzer はパイプラインレベル（`pipeline/any_enum_analyzer.rs`）に移動済み。TypeResolver の前に実行され、結果は `FileTypeResolution.any_enum_overrides` に格納される
- TypeResolver の `declare_var` が Any 型パラメータ/変数を override で enum 型に置換するため、`expr_types` は最初から正しい型を持つ
- `get_expr_type` / `get_type_for_var` にフォールバックロジックは不要（単一ソース）
- TypeResolver の `LogicalAnd/Or` resolve で両辺を解決するよう修正済み（compound guard の左辺式の型が `expr_types` に記録されなかったバグを修正）
- `convert_instanceof` の type_env.get を `get_expr_type` に置換済み（T5 のスコープだが T7 の any_enum_override が正しく動作するために必要）
- `try_convert_typeof_switch` の type_env.get を `get_expr_type` に置換済み（同上）
- `convert_constructor_body` から未使用の `params` パラメータを除去済み

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-237 (toFixed 未対応) + I-238 (Display 未実装)

## 保留中

（なし）
