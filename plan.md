# ts_to_rs 開発計画

## 現在のベースライン（2026-03-31 C-4完了後）

| 指標 | 値 |
|------|-----|
| Hono クリーン | 110/158 (69.6%) |
| エラーインスタンス | 58 |
| コンパイル(file) | 109/158 (69.0%) |
| コンパイル(dir) | 156/158 (98.7%) |
| テスト数 | 1515 |
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
| **現在: 変換率改善** | エラー < 60 件 | 58 件 ✅ 達成。C-5 でさらに改善 |
| **次: コンパイル品質** | ディレクトリコンパイルエラー 0 | 残 2 ファイル（I-273） |
| **その後: DX + 品質** | コンパイルテストスキップ 0 | 残 11 件 |

### ~~Phase R-1: コンポーネント責務境界の正常化~~ ✅ 完了

- `any_narrowing.rs` を `transformer/` → `pipeline/` に移動、逆方向依存解消
- any-narrowing enum 登録を pipeline に一本化（registry 側の重複処理削除）
- `transformer/types/` re-export 層除去、233 テストを `pipeline/type_converter/tests/` に移動
- テストギャップ G1-G23 を解消（+40 テスト追加）
- `register_extra_enums` の DRY 化、doc comment 修正、テストファイル凝集度改善

### ~~Phase C-4: TypeRegistry 型登録基盤~~ ✅ 完了

- `TypeDef::Struct` に `call_signatures` フィールド追加
- パラメータ抽出ヘルパー共通化（`extract_ts_fn_param`, `extract_pat_param`）— DRY 違反 3 件解消
- callable interface の call/construct signature 収集 + `is_callable_only` DRY 化
- `resolve_fn_type_info` が callable interface の return type を返すよう拡張
- `collect_type_alias_fields` に TsTypeRef ブランチ追加（`type X = Partial<T>` 対応）
- `resolve_type_params_impl` に `"::"` 複合名解決追加（I-308）
- rest パラメータ収集修正（I-259）、arrow デフォルトパラメータ対応
- OBJECT_LITERAL_NO_TYPE 29→25（-4件）、エラーインスタンス 61→58（-3件）
- テスト +21（1494→1515）、snapshot テスト +2（85→87）

### Phase T: E2E テスト基盤改修（最優先）

レビュー報告書（`report/integration-test-review-2026-03-31.md`, `report/e2e-test-infrastructure-review-2026-03-31.md`）で発見された構造的欠陥とカバレッジ不足を体系的に修正する。

| サブフェーズ | PRD | 内容 | 前提 |
|-------------|-----|------|------|
| **T-1** | `backlog/t1-test-infrastructure-foundation.md` | collecting モードの unsupported 検証、orphan 処理、DRY 化、一時ファイル安全化 | なし |
| **T-2** | `backlog/t2-compile-test-quality.md` | `#![allow]` 範囲縮小、warning 検出、compile skip リストの理由文書化 | T-1 |
| **T-3** | `backlog/t3-snapshot-test-enrichment.md` | 30+ WEAK TEST fixture の内容拡充、テスト名と内容の乖離修正 | T-1 |
| **T-4** | `backlog/t4-e2e-coverage-expansion.md` | E2E 未テスト機能への新規スクリプト追加、既存スクリプト強化 | T-1 |

**フェーズ T 完了基準**:
- collecting/builtins モードの全テストで unsupported がスナップショット化されている
- orphan fixture が 0 件
- WEAK TEST 判定が 0 件（30+ 件 → 全解消）
- E2E カバレッジ: スナップショット fixture の 50% 以上に対応する E2E テストが存在（現状 ~25%）
- コンパイルテスト skip リスト全項目に TODO ID が紐付いている
- `unused_mut` と `unreachable_code` がコンパイルテストで検出可能
- レビューで発見された新規バグ（S1: 3件、SD: 1件）が TODO に追記されている

### Phase R-2: TypeDef の TS 型メタデータ分離（I-312）

C-4 で `call_signatures` を TypeDef に追加した後、TypeDef 全体の設計を再検討する。TypeDef のフィールド型を TS 型のまま保持する設計に変更し、registry の責務を「純粋な型メタデータ収集」に正す。C-5 の匿名構造体生成のアプローチに影響するため、C-5 設計前に確定させる。

### Phase C-5: 匿名構造体 + 残存パターン

R-2 完了後の設計に基づき実施。現時点の見込み:

| 対象 | 内容 | 効果 |
|------|------|------|
| I-301 | 型注釈なしオブジェクトリテラル → 匿名構造体自動生成 | ~7件（C-4 の結果で変動） |
| I-306 | `.map()` callback への戻り値型伝播 | ~1件 |

### Phase D: コンパイル品質

| 対象 | 内容 | 効果 |
|------|------|------|
| I-273 | trait/struct 混同修正（`extends` の変換精度） | ディレクトリコンパイル 156→157+/158 |
| I-310 | HashMap computed access の transformer 変換 | コンパイルエラー削減 |
| I-217+I-265 | filter/find の参照型 + Option 二重ラップ | コンパイルテストスキップ解消 |
| I-237+I-238 | toFixed 変換 + Display 自動生成 | コンパイルテストスキップ解消 |
| I-311 | 型引数推論結果の引数 expected type フィードバック | 型推論精度向上 |

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
| `intersection-empty-object` | — | 未使用型パラメータ T (E0091) |

builtins あり（10 件）: 上記から `vec-method-expected-type` を除く。
