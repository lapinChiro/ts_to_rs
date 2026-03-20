# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

- `backlog/i61-chain-method-type-tracking.md` — チェーンメソッド戻り値型追跡

## 引継ぎ事項

### 直前の作業状態

- 未コミットの変更あり（型 narrowing Phase A + Phase B 全体の実装）
- 全テスト GREEN、clippy 0 警告、fmt 通過の状態

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212（同一 union 型の enum 重複定義）

### 型 narrowing アーキテクチャ概要

```
TypeScript の typeof/instanceof/null-check/truthy
    ↓
extract_narrowing_guards (patterns.rs) — ガードパターン抽出（&& を再帰分解）
    ↓
NarrowingGuard::if_let_pattern — パターン文字列 + swap フラグ生成
    ↓
[if 文] can_generate_if_let / generate_if_let (statements/mod.rs)
    ↓  複数ガード時は build_nested_if_let でネスト if let を生成
    ↓  残余条件は convert_expr で変換し内部 if に包む
    → Stmt::IfLet { pattern, expr, then_body, else_body }

[三項演算子] convert_cond_expr (expressions/mod.rs)
    → Expr::IfLet { pattern, expr, then_expr, else_expr }

[switch typeof] try_convert_typeof_switch (statements/mod.rs)
    → Stmt::Match { expr, arms } — 各アームで variant パターン + narrowing
```

any 型の場合は追加で:
```
any_narrowing.rs — collect_any_constraints → generate_any_enum
    ↓
TypeRegistry に enum 登録（build_registry で register_any_narrowing_enums）
    ↓
convert_fn_decl / convert_var_decl_arrow_fns でパラメータ型を enum に差替え
    ↓
上記の if let / match パイプラインに合流
```

## キュー

- `backlog/i100-generics-foundation.md` — ジェネリック型の基盤 + 具体化 + I-58 統合

## 保留中

（なし）
