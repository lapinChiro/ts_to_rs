# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

- `backlog/i61-chain-method-type-tracking.md` — チェーンメソッド戻り値型追跡

## 引継ぎ事項

### 直前の作業状態

- 未コミットの変更あり（型 narrowing Phase A + Phase B 全体の実装 + Placeholder 除去）
- 全テスト GREEN、clippy 0 警告、fmt 通過の状態

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212（同一 union 型の enum 重複定義）

## キュー

- `backlog/i100-generics-foundation.md` — ジェネリック型の基盤 + 具体化 + I-58 統合

## 保留中

（なし）
