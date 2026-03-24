# ts_to_rs 開発計画

PRD 化済みタスクの消化順序。次のタスクから順に着手する。

## 次のタスク

パイプライン再設計（`report/pipeline-component-design.md` 第4版に基づく）:

1. **P8: 統合 + 既存 API 置き換え** — `backlog/p8-integration.md`

## 引継ぎ事項

### P8 の作業状態（作業中）

**Phase A〜D + 全リファクタリング + パス解決一本化が完了。** 残り: Phase E（E5: doc コメント確認 + E-commit）のみ。

#### 完了済み（詳細は git history 参照）

Phase A〜C（パイプライン本実装）、D0a/D0b/D1/D6/D7（各種統合）、D-TR〜D4（型解決の統一 — Phase 1〜4 全完了）、D5（reg パラメータ削除）、D-2（Transformer struct 導入）、D-2-2（監査指摘 5 課題）、D-2-2-2（type_resolution メソッド化）。Phase E の E1〜E4 完了。パス解決バグ修正（C案: フォールバック廃止）完了。

#### 次に着手すべき作業 — Phase E 残り

- E5: pub な型・関数に doc コメントがあることを確認
- E-commit: P8 コミット

#### パス解決の設計判断（後続への引継ぎ）

**フォールバック廃止（C案）**: `convert_relative_path_to_crate_path` を削除し、全ての import/export パス解決を `ModuleGraph.resolve_import()` に一本化した。単一ファイルモードでは `TrivialResolver`（ファイルシステム不要の相対パス解決）を使用。`NullModuleResolver` はテスト用にのみ残存。

**理由**: パスからモジュールパスへの変換ロジックが `file_path_to_module_path()` の一箇所に集約され、index.ts 処理・r# プレフィクス・ハイフン変換等の差異が解消された。

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
