# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

1. **I-226: TypeEnv の完全除去** — `backlog/i226-typeenv-removal.md`

### I-226 実行計画

T1 完了。依存関係を精査した結果、PRD 記載の実行順序を以下のように最適化する。

#### 依存グラフ

```
T1 ✅
├─ T2 ─────→ T6 ─┐
├─ T3 ─→ T4 ─────┤
└─ T7 ─→ T5 ─────┤
                  ↓
                 T8 → T9 ─┐
                 T10 ──────┤
                           ↓
                          T11
```

#### 第 1 波（並行: T2, T3, T7）

全て独立。依存なし。

| タスク | 主な変更対象 | 完了基準 |
|---|---|---|
| T2 | `type_resolution.rs` に `DuFieldBinding` 追加、`type_resolver.rs` に DU switch 解析追加 | `is_du_field_binding()` が正しく判定。テスト追加 |
| T3 | `type_resolver.rs` の `visit_var_decl` で Fn 型を変数 Ident スパンに登録 | `get_expr_type(fn_ident)` が Fn 型を返す。テスト追加 |
| T7 | `functions/mod.rs:165,172` / `classes.rs:759` / `statements/mod.rs:2720` の insert 削除。`FileTypeResolution` に `any_enum_overrides` 追加 | パラメータ・Any enum 関連の type_env 操作が全除去。テスト GREEN |

#### 第 2 波（並行: T4, T5, T6）

| タスク | 依存 | 主な変更対象 |
|---|---|---|
| T4 | T3 | `statements/mod.rs:363,392,455,1195,1204,1208,1812` の insert/get 除去。`calls.rs:44` の get 除去 |
| T5 | T1, T7 | `expressions/mod.rs:175-188` の scope 操作除去。`patterns.rs:276` の get 除去。T7 完了により `patterns.rs:408` の type_env フォールバックも除去可能 |
| T6 | T2 | `statements/mod.rs:2063-2078` の scope 操作除去。`member_access.rs:256` を `is_du_field_binding` に置換 |

**T5 が T7 に依存する理由**: `resolve_if_let_pattern`（`patterns.rs:408`）は AnyTypeAnalyzer の enum override が type_env のみにあるため type_env フォールバックを残している。T7 で any_enum_overrides が FileTypeResolution に移行すれば、フォールバックを除去して type_env 参照をゼロにできる。

#### 第 3 波（T8 + T10 並行）

| タスク | 依存 | 主な変更対象 |
|---|---|---|
| T8 | T4,T5,T6,T7 | `Transformer` struct から `type_env` フィールド除去。sub-transformer の type_env パラメータ除去 |
| T10 | T4-T7 | テストコードの TypeEnv 構築・操作を除去。T8 と並行可能 |

#### 第 4 波

- **T9**: TypeEnv 構造体の削除（T8 後）
- **T11**: 品質チェック + ベンチマーク（T9, T10 後）

## 引継ぎ事項

### I-226 T1 完了時の注意点

- `resolve_if_let_pattern`（`patterns.rs:408`）は type_env 優先 → `get_type_for_var` フォールバック構成。AnyTypeAnalyzer の enum override が type_env のみにあるため、T7 完了まで type_env を残す
- TypeResolver の CondExpr ハンドラに `self.resolve_expr(&cond.test)` を追加済み（条件式内の変数型を expr_types に登録するため）
- `get_type_for_var(name, span)` ヘルパーを `type_resolution.rs` に追加済み（`get_expr_type` の Ident 特化版）

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-237 (toFixed 未対応) + I-238 (Display 未実装)

## 保留中

（なし）
