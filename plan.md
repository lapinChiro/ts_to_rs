# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`

## 引継ぎ事項

### P8 の作業状態（作業中）

**Phase A〜D + 全リファクタリングが完了。** 残り: Phase E（最終検証）のみ。

#### 完了済み（詳細は git history 参照）

Phase A〜C（パイプライン本実装）、D0a/D0b/D1/D6/D7（各種統合）、D-TR〜D4（型解決の統一 — Phase 1〜4 全完了）、D5（reg パラメータ削除）、D-2（Transformer struct 導入）、D-2-2（監査指摘 5 課題）、D-2-2-2（type_resolution メソッド化）。

#### 次に着手すべき作業 — Phase E（最終検証）

`tasks.md` の Phase E チェックリストに従う。

#### D-2 の設計判断（後続への引継ぎ）

**サブ Transformer パターン**: `convert_fn_decl` は `local_synthetic` 分離パターンを使用。成功時のみ `self.synthetic.merge(local_synthetic)`。サブ Transformer は独自のローカル TypeEnv を move で所有する。

**TypeEnv 所有化**: `type_env` フィールドは `TypeEnv`（所有）。ファクトリメソッド `for_module()` で内部作成。過渡的パターン（take+restore / clone）は全ラッパー削除で完全解消済み。

### コンパイルテストのスキップ（5 件）

1. `indexed-access-type` — I-35（indexed access type の非文字列キー）
2. `trait-coercion` — I-201（null as any → None）
3. `union-fallback` — I-202（Box<dyn Fn> derive 不適合）
4. `any-type-narrowing` — I-209（serde_json::Value → enum 型強制）
5. `type-narrowing` — I-212 は P8 で**解消済み**。残存エラー: `f64.toFixed()` 未対応 + `StringOrF64` の Display 未実装

## 保留中

（なし）
