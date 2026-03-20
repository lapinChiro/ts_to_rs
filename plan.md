# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

- `backlog/i189-trait-call-site-coercion.md` — trait 型の呼び出し側型強制（ExprContext ベースの統一メカニズム）

## キュー

- `backlog/i171-union-member-fallback.md` — union 未対応メンバーの型付きフォールバック（関数型→Box<dyn Fn>、タプル型→タプルバリアント）
- `backlog/i69-type-narrowing.md` — 型ガード後の型絞り込み（typeof/instanceof/null チェック → TypeEnv 分岐更新）
- `backlog/i61-chain-method-type-tracking.md` — チェーンメソッド戻り値型追跡（MethodSignature 拡張 + resolve_call_return_type 対応）
- `backlog/i100-generics-foundation.md` — ジェネリック型の基盤 + 具体化 + I-58 統合（TypeDef/TypeRegistry/Trait の型パラメータ対応）

## 保留中

（なし）
