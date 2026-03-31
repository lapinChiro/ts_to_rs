# ts_to_rs 開発計画

## 現在のベースライン（2026-03-31 S1修正後）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 110/158 (69.6%) |
| エラーインスタンス | 58 |
| コンパイル(file) | 109/158 (69.0%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1541 |
| コンパイルテストスキップ | 11 件（builtins なし） / 10 件（builtins あり） |

### エラーカテゴリ内訳（58 件）

| カテゴリ | 件数 | 主要イシュー |
|----------|------|-------------|
| OBJECT_LITERAL_NO_TYPE | 25 | I-301(7), I-306(1), I-300 imported(11), 他(6) |
| OTHER | 10 | parseInt(2), delete(2), class expr(1), 他 |
| QUALIFIED_TYPE | 3 | I-36 |
| FN_TYPE_PARAM | 3 | |
| MEMBER_PROPERTY | 3 | |
| ASSIGN_TARGET | 3 | |
| その他 | 11 | OBJ_KEY(2), INTERFACE_MEMBER(2), CALL_TARGET(2), 各1件×5 |

---

## ロードマップ

### フェーズ移行基準

| フェーズ | 基準 | 現状 |
|---------|------|------|
| **現在: 変換率改善** | エラー < 60 件 | 58 件 ✅ 達成 |
| **次: コンパイル品質** | ディレクトリコンパイルエラー 0 | 残 2 ファイル（I-273） |
| **その後: DX + 品質** | コンパイルテストスキップ 0 | 残 11 件 |

### 完了済みフェーズ

| フェーズ | 内容 | コミット |
|---------|------|---------|
| R-1 | コンポーネント責務境界の正常化（any_narrowing 移動、re-export 除去、テストギャップ解消） | `1329bd7` |
| C-4 | TypeRegistry 型登録基盤（call_signatures、TsTypeRef 解決、DRY 統合） | `d88d75e` |
| T-1 | テスト基盤の構造的欠陥修正（unsupported スナップショット化、orphan 解消、RAII 化） | `7b8e754` |
| T-2 | コンパイルテスト品質改善（#![allow] 分解、ミュータビリティ推論改善、新規バグ追跡） | `773a4c6` |
| S1 | サイレント意味変更3件修正（f64 guard化、optional chaining safe access、prelude型名衝突防止） | 未push |

### Phase T: E2E テスト基盤改修（進行中）

T-1, T-2, S1 完了。残り T-3, T-4。

| サブフェーズ | PRD | 内容 | 状態 |
|-------------|-----|------|------|
| **T-3** | `backlog/t3-snapshot-test-enrichment.md` | 30+ WEAK TEST fixture の内容拡充 | 未着手 |
| **T-4** | `backlog/t4-e2e-coverage-expansion.md` | E2E 未テスト機能への新規スクリプト追加 | 未着手 |

**未達成の完了基準**:
- WEAK TEST 判定が 0 件（30+ 件 → 全解消）
- E2E カバレッジ: スナップショット fixture の 50% 以上（現状 ~25%）

### Phase R-2: TypeDef の TS 型メタデータ分離（I-312）

TypeDef のフィールド型を TS 型のまま保持する設計に変更し、registry の責務を「純粋な型メタデータ収集」に正す。C-5 の匿名構造体生成のアプローチに影響するため、C-5 設計前に確定させる。

### Phase C-5: 匿名構造体 + 残存パターン

R-2 完了後の設計に基づき実施。I-301（~7件）、I-306（~1件）。

### Phase D: コンパイル品質

| 対象 | 内容 | 効果 |
|------|------|------|
| I-273 | trait/struct 混同修正 | ディレクトリコンパイル 156→157+/158 |
| I-310 | HashMap computed access | コンパイルエラー削減 |
| I-217+I-265 | filter/find の参照型 + Option 二重ラップ | コンパイルテストスキップ解消 |
| I-237+I-238 | toFixed 変換 + Display 自動生成 | コンパイルテストスキップ解消 |
| I-319 | 配列インデックスの .get() 化（~97箇所 S1→S2 昇格） | runtime panic 排除 |

### Phase E: DX + 生成コード品質

| 対象 | 内容 | 効果 |
|------|------|------|
| I-30 | Cargo.toml 依存追加 | I-183, I-34 のゲート解除 |
| I-182 | Hono クリーンファイルのコンパイルテスト CI 化 | 回帰検出自動化 |
| I-282+I-283 | デフォルトパラメータの DRY 化 + unwrap_or_else | 生成コード品質 |

### 長期ビジョン

| マイルストーン | 指標 |
|---------------|------|
| 変換率 80% | クリーン 126/158（現在 110） |
| コンパイル率 80% | ファイルコンパイル 126/158（現在 109） |
| コンパイルテストスキップ 0 | 全 fixture がコンパイル通過 |

---

## 引継ぎ事項

設計判断: [doc/design-decisions.md](doc/design-decisions.md)。調査レポート: `report/`。

### コンパイルテストのスキップ（11 件）

| テスト名 | 原因 | 概要 |
|----------|------|------|
| `indexed-access-type` | — | `Env` 型未定義（マルチファイルテストでカバー） |
| `trait-coercion` | I-201 | `null as any` → `None` が `Box<dyn Trait>` に代入不可 |
| `union-fallback` | I-202 | `Box<dyn Fn>` を含む enum に derive 不適合 |
| `any-type-narrowing` | I-209 | `serde_json::Value` → enum 型の自動変換 |
| `type-narrowing` | I-237+I-238 | `toFixed` 未変換 + `Display` 未生成 |
| `array-builtin-methods` | I-217+I-265 | filter/find の参照型 + Option 二重ラップ |
| `instanceof-builtin` | I-270c | メソッド impl 不在 |
| `external-type-struct` | I-270 | builtins なし環境で外部型 struct 未生成 |
| `ternary-union` | I-11 | 分岐値の enum variant ラッピング未実装 |
| `vec-method-expected-type` | I-289 | ビルトイン前提 |
| `intersection-empty-object` | I-314 | 未使用型パラメータ T (E0091) |

builtins あり（10 件）: 上記から `vec-method-expected-type` を除く。
