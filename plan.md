# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

1. **I-226: TypeEnv の完全除去** — `backlog/i226-typeenv-removal.md`

### I-226 実行計画

第 1 波（T2, T3, T7）完了。依存関係を精査した結果、PRD 記載の実行順序を以下のように最適化する。

#### 依存グラフ

```
T1 ✅
├─ T2 ✅ ───→ T6 ─┐
├─ T3 ✅ → T4 ─────┤
└─ T7 ✅ → T5 ─────┤
                    ↓
                   T8 → T9 ─┐
                   T10 ──────┤
                             ↓
                            T11
```

#### 第 1 波（完了: T2, T3, T7）

| タスク | 状態 | 主な変更 |
|---|---|---|
| T2 | ✅ | `DuFieldBinding` + `is_du_field_binding()` + TypeResolver DU switch 検出 |
| T3 | ✅ | `visit_var_decl` で Fn 型を変数 Ident スパンにも `expr_types` に記録 |
| T7 | ✅ | AnyTypeAnalyzer をパイプラインレベルに移動。`any_enum_overrides` を `FileTypeResolution` に格納。TypeResolver が `declare_var` 時に Any → enum 型で置換。パラメータ・Any enum 関連の type_env 操作を全除去 |

#### 次の作業: 第 2 波（並行: T4, T5, T6）

| タスク | 依存 | 主な変更対象 |
|---|---|---|
| T4 | T3 ✅ | `statements/mod.rs` の insert/get 除去。`calls.rs:44` の get 除去 |
| T5 | T1 ✅, T7 ✅ | `expressions/mod.rs:175-188` の scope 操作除去。`patterns.rs:276` の get 除去。`patterns.rs:404` の type_env フォールバック除去 |
| T6 | T2 ✅ | `statements/mod.rs:2063-2078` の scope 操作除去。`member_access.rs:256` を `is_du_field_binding` に置換 |

#### 第 3 波（T8 + T10 並行）

| タスク | 依存 | 主な変更対象 |
|---|---|---|
| T8 | T4,T5,T6,T7 | `Transformer` struct から `type_env` フィールド除去。sub-transformer の type_env パラメータ除去 |
| T10 | T4-T7 | テストコードの TypeEnv 構築・操作を除去。T8 と並行可能 |

#### 第 4 波

- **T9**: TypeEnv 構造体の削除（T8 後）
- **T11**: 品質チェック + ベンチマーク（T9, T10 後）

## 引継ぎ事項

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
