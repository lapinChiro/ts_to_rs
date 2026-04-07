# ts_to_rs 開発計画

## 次のアクション

**次のアクション**: **I-378** → **I-376** の順。

I-377（Batch 11c-fix-2-b）完了時の self-review で、I-375 が解消した `Expr::FnCall::name: String`
と同型の broken window が 5 サイトに残存していることを発見した（`Expr::Ident` に
display-formatted path 文字列 `"Direction::Up"` 等を encode する pipeline-integrity 違反）。
これを **I-378** として独立 PRD 化し、I-376 の前に挟む。

**実行順序**: ~~I-375~~ → ~~I-377~~ → ~~行数超過ファイルのケア~~ → **I-378** → **I-376**

行数超過を I-378 より前に行う理由:
- 現状 5 ファイルが 1000 行超過（`./scripts/check-file-lines.sh` で検知済み）
- I-378 は `Expr::Path` 新 variant 導入と downstream 書き換えで `src/ir/mod.rs`（現 1105 行）
  を更に拡大させるため、先に分割しておくことで I-378 の影響範囲がクリーンに保たれる
- 行数超過ファイル群の責務分割は **凝集度向上の機会**（テスト fixture / 巨大ファイルの
  論理セクション分離）であり、後続 PRD のレビュー品質を底上げする

I-378 を I-376 より前に行う理由:
- I-378 は I-377 の直接の継続（同一クラスの broken window 撲滅）であり、原理的な
  pipeline-integrity 違反を残したまま L4/L3 の他作業に進むのは妥協
- I-378 完了により Pattern::Literal の不変条件「値リテラルのみ」が構造的に強制可能になる
- I-376 は pipeline 層（`pipeline/mod.rs` Pass 4/5 plumbing）と直交しており、IR 層の
  整理が完了してから着手する方が rework ゼロ

根拠:
1. **I-375 先行（L2 優先 + IR 形状固定）**: IR 破壊的変更を最初に打って `Expr::FnCall` の最終形状（`CallTarget` enum）を確定させることで、後続の I-377 visitor 実装が最終形状に対して 1 回で済む（rework ゼロ）。また priority L2（correctness）を L3 より先行させる `todo-prioritization.md` 原則を遵守
2. **I-377 中間（IR 層の大手術集約 + MatchPattern 構造化）**: 安定した IR 形状に対して `IrVisitor` trait を 1 回で導入。`external_struct_generator` の `collect_type_refs_*` と `ir/substitute.rs` を visitor ベースに書き換え。

   **I-377 スコープ拡張（I-375 Discovery で確定）**: I-377 は IrVisitor 導入に加え、以下を **同一 PRD 内で完了** させる必要がある:
   - `MatchPattern::EnumVariant { path: String }` を構造化 variant（例: `{ enum_name: String, variant: String, fields: Vec<MatchPattern> }`）に分解
   - `MatchPattern::Verbatim(String)` / `Stmt::IfLet::pattern: String` / `Stmt::WhileLet::pattern: String` / `Expr::IfLet::pattern: String` / `Expr::Matches::pattern: String` の pattern 文字列を構造化 IR に置き換え（Rust pattern grammar の IR 化）

   **理由**: I-377 の目的は「walker / substitute が IR を **構造的** に走査できる基盤の確立」である。`MatchPattern::EnumVariant::path: String` と `Verbatim(String)` が残存すると、IrVisitor の `visit_match_pattern` は内部で文字列 parser を呼ぶか、uppercase head ヒューリスティックを維持するしかなく、**「構造化 walker 基盤」という I-377 の目的が達成されない**（broken window 残存）。従って pattern 文字列の完全構造化は I-377 の **前提条件** として同一 PRD に含める。

   **I-375 に含めない理由**: I-375 の責務は `Expr::FnCall` の call semantics（何を呼ぶか）の構造化であり、IR Expr サブシステムに閉じる。MatchPattern の構造化は IR pattern grammar サブシステムの課題であり、凝集度・責務分離の観点から別 PRD に帰属させる方が合理的。同時改修は構築サイト変更のリスクが累積する。

   **影響範囲追加**:
   - `src/ir/mod.rs`: `MatchPattern`, `Stmt::IfLet`, `Stmt::WhileLet`, `Expr::IfLet`, `Expr::Matches` の pattern field 型変更
   - MatchPattern 構築サイト: transformer 配下（要 grep 実測）
   - Generator: pattern rendering ロジック
   - I-375 で構造化しなかった `collect_type_refs_from_verbatim_pattern` / `collect_type_refs_from_match_arm` の uppercase 判定コードを削除し、IrVisitor ベースに統合

   **I-375 からの申し送り事項（I-377 で必ず解消すべき 3 項目）**:

   **A. `RUST_BUILTIN_TYPES` からの `Some / None / Ok / Err` 削除**

   I-375 実装中に当該 4 エントリを削除したところ、`tests/integration_test.rs` の `test_type_narrowing` / `test_async_await` / `test_error_handling` / `test_narrowing_truthy_instanceof` の 4 件が回帰した。原因: `if let Some(y) = y { ... }` のような pattern が `Stmt::IfLet::pattern: String = "Some(y)"` として IR に保存されており、`collect_type_refs_from_verbatim_pattern` の uppercase-head ヒューリスティックが `"Some"` を refs に登録し、それが builtin フィルタで除外されなくなったため `pub struct Some { }` が stub 生成された。

   **分析結果**: この問題は **I-375 単独では解決不能**。理由:
   - pattern 文字列が String である限り、walker は文字列解析に頼らざるを得ない
   - 文字列解析は必ず uppercase-head ヒューリスティックか、Some/None/Ok/Err のハードコード除外の**どちらか**を必要とする
   - 両方を排除するには pattern を構造化 IR に置き換える必要があり、これが I-377 のスコープそのもの

   I-375 の PRD Completion Criterion #4 は「MatchPattern 構造化と同時に達成する前提」で書かれていたが、スコープ分離の都合上 I-377 に実際の削除作業を委譲する。暫定対応として `RUST_BUILTIN_TYPES` に `Some / None / Ok / Err` を**明示コメント付きで復元済**（`src/pipeline/external_struct_generator/mod.rs:15-36`）。

   **I-377 での必須アクション**:
   1. `MatchPattern::EnumVariant { path: String }` を `{ enum_name: String, variant: String, fields: Vec<MatchPattern> }` 等の構造化 variant に置換
   2. `Stmt::IfLet::pattern: String` / `Stmt::WhileLet::pattern: String` / `Expr::IfLet::pattern: String` / `Expr::Matches::pattern: String` の pattern 文字列を構造化 IR (例: `Pattern` enum) に置換
   3. `collect_type_refs_from_verbatim_pattern` と `collect_type_refs_from_match_arm` の uppercase-head 判定コードを完全削除
   4. **上記 3 が完了してから** `RUST_BUILTIN_TYPES` から `"Some", "None", "Ok", "Err"` の 4 エントリを削除
   5. `integration_test.rs` の 4 件が削除後も pass することを確認

   **A を忘れると**「IR に display-formatted 文字列を保存禁止」「ビルトイン variant のハードコード除外禁止」という I-375/I-377 の根本目的が未達成のまま残る。I-377 の Completion Criteria に明示的に組み込むこと。

   **B. `convert_call_expr` Ident callee の `type_ref` と `sanitize_rust_type_name` の不整合**

   `src/transformer/expressions/calls.rs:106-113` で、Ident callee が `TypeDef::Struct / TypeDef::Enum` として reg 登録されている場合に `type_ref: Some(fn_name.clone())` を設定するが、この `fn_name` は **sanitize 前の TS 識別子**。TS `interface Self { (x: number): string }` のような callable interface の場合、生成 Rust 構造体は `Self_` (I-374 で sanitize) だが `type_ref` には `"Self"` が記録される。walker は `"Self"` を refs に登録し、生成 Rust 側の `Self_` 構造体とミスマッチになる latent バグ。

   現状の Hono ベンチでは顕在化しないが、クリーンな実装の観点では修正必須。I-374（Rust 予約語 sanitize）と併せて解消すべき。I-377 スコープには直接含めないが、I-374 実施時に `convert_call_expr` の `fn_name` も `sanitize_rust_type_name` を通す修正を忘れないこと。

   **C. I-375 統合テストが walker 直接検証になっていない**

   `tests/lowercase_class_reference_test.rs` は「class myClass + new myClass(1) を transpile して出力に `struct myClass` と `myClass::new(` が含まれる」を検証するが、これは Transformer が class declaration を直接 struct に emit するため walker の参照捕捉ロジックを直接検証していない。I-377 で walker を visitor pattern 化する際、walker の `type_ref: Some("myClass")` 走査が正しく動作することを **walker 単体テストで直接検証** すること（`test_walker_lowercase_class_name_registered_via_type_ref` 等）。I-375 実装では Priority B テストとして追加済。
3. **I-376 最後（独立 pipeline 層）**: IR/walker 層と完全に直交（`pipeline/mod.rs` Pass 4/5 plumbing のみ）。最後に配置することで層ごとに review を分離可能

6 順列分析の結論: #1 `I-375 → I-377 → I-376` が rework ゼロで総コスト最小。逆順（I-377 先行）は visitor の `walk_expr::FnCall` 分岐を I-375 で再編集する rework +1 が発生

### 次バッチの根拠

Batch 11c-fix-2 は Batch 11c-fix の **直接の継続** であり、以下を理由に最優先で実施する:

1. ~~**I-375 (FnCall 構造化)**~~ **完了**（Batch 11c-fix-2-a）
2. ~~**I-377 (visitor pattern 化)**~~ **完了**（Batch 11c-fix-2-b、self-review で I-378 を派生）
3. ~~**行数超過ファイルのケア (11c-fix-2-line)**~~ **完了**（I-378 前に `ir/mod.rs` 等の分割を実施済）
4. **I-378 (Expr::Path 構造化)**: I-377 self-review で発見した path-in-Expr::Ident broken
   window（5 サイト）。I-375 が FnCall::name で解消したのと同型。pipeline-integrity ルール
   「IR に display-formatted 文字列を保存禁止」の完全遵守のための残課題
5. **I-376 (per-file 外部型 stub の構造的重複)** は Batch 11c-fix の `is_definition_item`
   dedup（`src/pipeline/placement.rs:225`）が「出力時 patch」として残っている根本原因。
   pipeline 段階で構造的に dedup すれば patch 不要になる

残 2 件（I-378 → I-376）を Batch 11c-fix-2 の継続として実施する。
後続の L3 バッチ（11b 以降）はそれまで保留する。

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
| ~~11c-fix-2-a~~ | ~~I-375~~ | ~~`Expr::FnCall::name` の意味論的多義性（CallTarget で構造化）~~ | **完了** `CallTarget { Path { segments, type_ref } \| Super }` 2 variant 構造化、walker の uppercase-head ヒューリスティック廃止、lowercase class 統合テスト追加、Hono 後退ゼロ |
| ~~11c-fix-2-b~~ | ~~I-377~~ | ~~walker / substitute の IrVisitor 化 + `MatchPattern` / verbatim pattern 文字列の構造化~~ | **完了** `Pattern` enum + `IrVisitor` / `IrFolder` trait 導入、`MatchPattern` 削除、5 stmt/expr の `pattern: String` を構造化、walker `TypeRefCollector` 化、`RUST_BUILTIN_TYPES` からの Some/None/Ok/Err 除去（I-375 申し送り完遂）、substitute.rs の IrFolder 化、散発再帰 detector の IrVisitor 化、Hono 後退ゼロ、テスト 2124→2175（+51） |
| ~~11c-fix-2-line~~ | ~~行数超過ファイルのケア~~ | ~~5 ファイル（external_struct_generator/tests.rs 2489、output_writer.rs 1135、calls.rs 1111、ir/mod.rs 1105、ir/tests/mod.rs 1031）の責務分割~~ | **完了** D1: `camel_to_snake` test 重複解消。S1: `ir/mod.rs` を `types/naming/item/stmt/expr` に分割。S2: `ir/tests/mod.rs` を 5 カテゴリ分割。S3: `external_struct_generator/tests.rs` を 7 カテゴリ分割 + `tests/` ディレクトリ化。S4: `calls.rs` を `basic/console_log/rest_params/type_ref` に分割 + `calls/` ディレクトリ化。S5: `output_writer.rs` を `mod.rs` (entry) / `placement.rs` / `mod_rs_emit.rs` に責務分離。Hono 後退ゼロ、テスト 2171 維持 |
| 11c-fix-2-d | I-378 | `Expr::Path` 構造化による path-in-Ident broken window 撲滅（I-377 self-review で発見） | **次** |
| 11c-fix-2-c | I-376 | per-file 外部型 stub の構造的重複（pipeline 段階 dedup） | I-378 の後 |
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

`git log` で詳細参照: Batch 1〜3b, R-1, C-4, T-1〜T-4, S1, D-1, 4a〜5b, 10b, 6, 6b, 7, 8, 14, 8b, 9, 10, 11a, 11c, 11c-fix, 11c-fix-2-a (I-375), 11c-fix-2-b (I-377), 11c-fix-2-line

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
