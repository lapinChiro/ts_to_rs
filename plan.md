# ts_to_rs 開発計画

## 次のアクション

**次のアクション**: Batch 11c-fix-2（**最優先**） — 本来 Batch 11c-fix で構造解消すべき残課題 I-375 / I-376 / I-377 を次セッションで完全に解消する。詳細は `TODO` 参照。これらは Batch 11c-fix の self-review で発見したが、本セッションでは scope 拡大による回帰リスク累積を避けるため別バッチに分離した

### 次バッチの根拠

Batch 11c-fix-2 は Batch 11c-fix の **直接の継続** であり、以下を理由に最優先で実施する:

1. **I-375 (FnCall 構造化)** は Batch 11c-fix で導入した uppercase head ヒューリスティック（`src/pipeline/external_struct_generator/mod.rs:516`）と同根の workaround `RUST_BUILTIN_TYPES` への `Some/None/Ok/Err` ハードコード（同 `:21`）を構造的に解消するもの。これらは「Rust 命名規約をコントラクトとして受け入れれば correct」だが、TS で lowercase クラス名を使った場合 false negative の可能性がある。pipeline-integrity ルール「IR に display-formatted 文字列を保存禁止」を完全遵守するための残課題。
2. **I-376 (per-file 外部型 stub の構造的重複)** は Batch 11c-fix の `is_definition_item` dedup（`src/pipeline/placement.rs:225`）が「出力時 patch」として残っている根本原因。pipeline 段階で構造的に dedup すれば patch 不要になる。
3. **I-377 (visitor pattern 化)** は Batch 11c-fix で大量に追加した手書き walker（`collect_type_refs_from_item / _stmt / _expr / _rust_type / _verbatim_pattern / _match_arm / _type_params / _method`）の長期保守性問題。新 IR variant 追加時の更新漏れリスクを compile-time 検出だけに頼る現状を改める。

これら 3 件を Batch 11c-fix-2 として **直近 1 セッション内に完了** させること。後続の L3 バッチ（11b 以降）はそれまで保留する。

その後の次バッチ未定（L3 残: 11b, 12, 13, 15-23）

---

## 現在のフェーズ: コンパイル品質 + 設計基盤

フェーズ移行基準: **S1 バグ 0 + ディレクトリコンパイルエラー 0**
現状: S1 残 0 件、ディレクトリコンパイル残 1 ファイル（157/158）

### バッチ実行計画

優先度は `todo-prioritization.md` の L1〜L4 レベルで決定。L1 → L2 → L3 → L4 の順。
同一レベル内はレバレッジ → 拡大速度 → 修正コストの順。
詳細分析: `report/batch-prioritization-2026-04-05.md`

#### L1: 信頼性基盤

S1 バグ 0 件達成。

#### L2: 設計基盤

| Batch | イシュー | 根本原因 |
|-------|---------|---------|
| ~~9~~ | ~~I-282~~ | ~~デフォルトパラメータ lazy eval 設計不足~~ **完了** |
| ~~10~~ | ~~I-299+I-273~~ | ~~型パラメータ制約のモノモーフィゼーション~~ **完了** |

#### L3: 拡大する技術的負債

| Batch | イシュー | 根本原因 | レバレッジ |
|-------|---------|---------|-----------|
| ~~11a~~ | ~~I-368+I-369~~ | ~~OutputWriter types.rs 衝突 + ビルトイン型モノモーフィゼーション~~ | **完了** dir 156→157 |
| ~~11c~~ | ~~I-371~~ | ~~合成型の単一正準配置（同一ファイル重複 + クロスファイル冗長性）~~ | **完了** E0428+E0119 17→0、shared_imports 生成 |
| ~~11c-fix~~ | ~~I-371 self-review 修正~~ | ~~substring scan / 重複ロジック / API 非対称 / テスト不足 等 12 問題~~ | **完了** IR ベース placement、`RustType::QSelf` 構造化、fn body IR walker、`UndefinedRefScope` 共通骨格、type_params constraint walking、verbatim pattern walking、自動テスト +104 件 |
| **11c-fix-2** | **I-375 + I-376 + I-377** | **Batch 11c-fix で導入した uppercase ヒューリスティック / 出力時 dedup patch / 手書き walker の構造解消（本来 11c-fix で行うべきだった残課題）** | **最優先** Batch 11c-fix の継続。次セッションで完了させる |
| 11b | I-300+I-301+I-306 | OBJECT_LITERAL_NO_TYPE（25件） | 最大エラーカテゴリ削減 |
| 12 | I-311+I-344 | 型引数推論フィードバック欠如 | I-344 自動解消 + generic 精度 |
| 13 | I-11+I-238+I-202 | union/enum 生成品質 | skip: ternary, ternary-union 他 |
| ~~14~~ | ~~I-361+I-257~~ | ~~デストラクチャ変数型付き登録~~ | **完了** |
| 15 | I-340 | Generic Clone bound 未付与 | generic コード増に比例 |
| 16 | I-360+I-331 | Option\<T\> narrowing + 暗黙 None | skip: functions 部分 |
| 17 | I-321 | クロージャ Box::new ラップ漏れ | skip: closures, functions 部分 |
| 18 | I-217+I-265 | iterator クロージャ所有権 | skip: array-builtin-methods |
| 19 | I-336+I-337 | abstract class 変換パス欠陥 | 安定（拡大しない） |
| 20 | I-329+I-237 | string メソッド変換 | skip: string-methods |
| 21 | I-313 | 三項演算子 callee パターン | CALL_TARGET 4件 |
| 22 | I-30 | Cargo.toml 依存追加 | I-183, I-34 のゲート |
| 23 | I-182 | コンパイルテスト CI 化 | 回帰検出自動化 |

#### L4: 局所的問題

バッチ化は L3 完了後に実施。根本原因クラスタ単位で順次対応。
主要候補: I-322, I-326, I-330, I-332, I-314, I-201, I-209, I-310, I-345, I-342, I-260 他

### 完了済みバッチ

`git log` で詳細参照: Batch 1〜3b, R-1, C-4, T-1〜T-4, S1, D-1, 4a〜5b, 10b, 6, 6b, 7, 8, 14, 8b, 9, 10, 11a, 11c, 11c-fix

---

## ベースライン（2026-04-05 計測）

| 指標 | Batch 8 時点 | Batch 10 時点 | Batch 11a 時点 | Batch 11c 時点 |
|------|-------------|--------------|---------------|---------------|
| Hono クリーン | 112/158 (70.9%) ※Hono upstream 変更 | 114/158 (72.2%) | 114/158 (72.2%) | 114/158 (72.2%) |
| エラーインスタンス | 56 ※CALL_TARGET +2 (upstream) | 54 | 54 | 54 |
| コンパイル(file) | 111/158 (70.3%) | 113/158 (71.5%) | 113/158 (71.5%) | 113/158 (71.5%) |
| コンパイル(dir) | 156/158 (98.7%) | 156/158 (98.7%) | 157/158 (99.4%) | 157/158 (99.4%) |
| dir compile エラー (E04xx/E01xx) | — | — | 17 (E0428×5 + E0119×12) | 14 (E0405/E0107/E0072 のみ) |
| テスト数 | 2048 | 2143 | 2150 | 2156 |
| コンパイルテストスキップ | 23 / 22（builtins なし / あり） | 22 / 21 | 22 / 21 | 22 / 21 |

### 長期ビジョン

| マイルストーン | 指標 |
|---------------|------|
| 変換率 80% | クリーン 126/158（現在 112） |
| コンパイル率 80% | ファイルコンパイル 126/158（現在 111） |
| コンパイルテストスキップ 0 | 全 fixture がコンパイル通過（現在 23 件） |

---

## リファレンス

- 調査レポート: `report/`
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
- 優先度分析: `report/batch-prioritization-2026-04-05.md`
