# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

（I-226 完了。次の PRD を選定する）

### I-226 実行計画（完了）

全 11 タスク完了。TypeEnv がコードベースから完全に除去された。

#### 依存グラフ

```
T1 ✅
├─ T2 ✅ ───→ T6 ✅ ┐
├─ T3 ✅ → T4 ✅ ────┤
└─ T7 ✅ → T5 ✅ ────┤
                      ↓
                     T8 ✅ → T9 ✅ ┐
                     T10 ✅ ───────┤
                                   ↓
                                  T11 ✅
```

## 引継ぎ事項

### I-226 完了時の状態

- `TypeEnv` 型がコードベースに存在しない。全ての型情報は `FileTypeResolution` 経由で一本化
- `TypeResolver` から未使用の `module_graph` フィールドも除去済み（`#[allow(dead_code)]` の不適切使用を修正）
- `type_env.rs` は `wrap_trait_for_position` / `TypePosition` のみ残存（独立ユーティリティ）

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-237 (toFixed 未対応) + I-238 (Display 未実装)

## 保留中

（なし）
