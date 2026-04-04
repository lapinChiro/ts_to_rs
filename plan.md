# ts_to_rs 開発計画

## 次のアクション

**Batch 4d-C: declaration 変換の責務分離**

PRD 設計完了（`backlog/4d-c-declaration-responsibility-separation.md`）。調査レポート: `report/batch-4d-c-investigation.md`。

タスク順序: T1（resolve 関数公開 + extract_discriminated_variant 拡張）→ T2（discriminated union 移行）→ T3（intersection with union 移行）→ T4（extract_intersection_members 移行）→ T5（general union 移行）→ T6（品質確認）。T2-T5 は T1 完了後に着手可能。

---

## 現在のフェーズ: コンパイル品質 + 設計基盤

フェーズ移行基準: **S1 バグ 0 + ディレクトリコンパイルエラー 0**
現状: S1 残 1 件（I-298）、ディレクトリコンパイル残 2 ファイル（I-273）

### バッチ実行計画

優先度は `todo-prioritization.md` の L1〜L4 レベルで決定。同一レベル内はレバレッジ → 拡大速度 → 修正コストの順。

| Batch | イシュー | 根本原因 | Level | 状態 |
|-------|---------|---------|-------|------|
| **4d-C** | — | **declaration 変換の責務分離（union/intersection の型解決を resolve に統一）** | **L2: 設計** | |
| 4c | I-347+I-348 | TypeDef 型操作メソッド補完（Function/ConstValue） | L2: 設計 | |
| 5 | I-334+I-333+I-327 | narrowing 基盤欠陥（RC-1 内 7 件中コア 3 件） | L2: 設計 | |
| 5b | I-215+I-213+I-214+I-256 | narrowing 残課題（Batch 5 基盤の上に構築） | L2: 設計 | |
| 6 | I-338+I-318 | 構造的同値性欠如（synthetic 型の重複生成） | L2: 設計 | |
| 7 | I-320+I-328+I-323 | 個別修正（optional param + never + toString） | L3: ブロッカー | |
| 8 | I-324+I-325+I-344 | 文字列型モデル（&str/String 不一致） | L3: 拡大 | |
| 9 | I-340 | 所有権: Generic Clone bound | L3: 拡大 | |
| 10 | I-336+I-337 | abstract class 変換パス欠陥 | L3: 安定 | |
| 11 | I-326+I-330+I-331+I-332+I-322 | 個別コンパイルエラー修正 | L4: 局所 | |
| 12 | I-329 | 文字列メソッド変換（charAt, repeat）※indexOf 実装済み | L4: 局所 | |
| 13 | I-342 | getter/setter 呼び出し側変換 | L4: 局所 | |
| 14 | I-273+I-217+I-265+I-237+I-238+I-310 | 従来課題（先行修正で一部緩和） | L4: 混合 | |

---

## 後続フェーズ

### Phase C-5: 匿名構造体 + 残存パターン

Batch 4（I-312: registry 責務正常化）完了後に実施。I-301（~7件）、I-306（~1件）。

### Phase E: DX + 生成コード品質

| 対象 | 内容 | 効果 |
|------|------|------|
| I-30 | Cargo.toml 依存追加 | I-183, I-34 のゲート解除 |
| I-182 | Hono クリーンファイルのコンパイルテスト CI 化 | 回帰検出自動化 |
| I-282+I-283 | デフォルトパラメータの DRY 化 + unwrap_or_else | 生成コード品質 |

---

## ベースライン（ベンチマーク計測: 2026-04-03、Batch 4b 完了時点で同値確認済み）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 110/158 (69.6%) |
| エラーインスタンス | 58 |
| コンパイル(file) | 109/158 (69.0%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1900（ユニット 1721 + コンパイル 3 + スナップショット 3 + E2E 84 + integration 89）※Batch 4d-B 完了時点 |
| コンパイルテストスキップ | 25 件（builtins なし）/ 24 件（builtins あり） |

### 長期ビジョン

| マイルストーン | 指標 |
|---------------|------|
| 変換率 80% | クリーン 126/158（現在 110） |
| コンパイル率 80% | ファイルコンパイル 126/158（現在 109） |
| コンパイルテストスキップ 0 | 全 fixture がコンパイル通過（現在 25 件） |

---

## リファレンス

- 設計判断: [doc/design-decisions.md](doc/design-decisions.md)
- 調査レポート: `report/`
- 完了済みバッチ: `git log`（Batch 1〜3b, R-1, C-4, T-1〜T-4, S1, D-1）
