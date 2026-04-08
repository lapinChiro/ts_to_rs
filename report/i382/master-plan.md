# I-382 完全解消マスタープラン

## 最終ゴール

`src/pipeline/external_struct_generator/mod.rs::generate_stub_structs` を **完全削除** し、Pass 5c を「synthetic_items が参照する user 定義型に対する `use crate::<path>::Type;` 生成」のみに置き換える。

完了の必要十分条件: probe ログで `empty stub fallback` が 0 件、`generate_stub_structs` の grep ヒット 0 件、Hono ベンチ regression 0 件。

## 事前検証で判明した root cause 分布 (Hono 158 fixture, 2026-04-07 計測)

| カテゴリ | 件数 | 例 | 対応 PRD |
|---|---|---|---|
| (a) DOM/Web API builtin | 16 | `HTMLCanvasElement`, `Window`, `BufferSource` 等 | PRD-β |
| (b) 型パラメータ leak | 9 | `E`, `M`, `P`, `S`, `U`, `TNext`, `TResult`, `TResult1`, `TResult2` | PRD-α |
| (c) 不明識別子 | 4 | `OutputType`, `Status`, `__type`, `symbol` | PRD-γ |
| (d) user 定義型 | ≥1 | `HTTPException` ほか (T0.4 で全件特定) | PRD-δ (= I-382 本体) |

## 進行順序の判断 (Option Y 採用)

PRD-A と PRD-B は当初並列実行可能と評価したが、PRD-A 実装で以下が変化するため **直列順序 T2.A → T1.B → T2.B** を採用:

1. Cluster 2 の件数 (現状 73 件) が PRD-A 実装で減る可能性 — DOM 系 union が error 化されて消えると、その内部の user 型参照も連鎖消滅する
2. anonymous synthetic 名が generic 化される (`MOrVecM` → `MOrVecM<M>` 等) — PRD-B のテスト fixture / Impact Area がこれに依存
3. `push_type_param_scope` が replace → append-merge に変更 — PRD-B が同 API を使う場合の前提が変わる
4. `Item::Enum.type_params` が non-empty になり、PRD-B の import 解決ロジックが「型引数を import 対象から除外する」考慮を必要とする

CLAUDE.md「最も理想的でクリーンな実装」「アドホック対応禁止」原則に照らし、PRD spec を実測ベースで起票する Option Y を選択。`plan.md` にも同方針を記載。

## 進捗ステータス

| Phase / Task | 状態 | 完了日 | 備考 |
|---|---|---|---|
| Phase 0: 徹底調査 | **done** | 2026-04-07 | report/i382/phase0-synthesis.md |
| T0.1: 型パラメータ leak | done | 2026-04-07 | report/i382/type-param-leak.md, 11 件特定 |
| T0.2: 不明識別子 | done | 2026-04-07 | report/i382/unknown-identifiers.md, OutputType/Status→PRD-α, symbol→PRD-β, __type→PRD-γ |
| T0.3: DOM 型 | done | 2026-04-07 | report/i382/dom-types.md, 案 i 推奨 |
| T0.4: user 定義型 | done | 2026-04-07 | report/i382/user-defined-refs.md, 73 件確定 |
| T0.5: 統合 | done | 2026-04-07 | report/i382/phase0-synthesis.md, クラスタ 1a/1b/1c/2 確定 |
| Phase 0.6: Impact Area pre-review | done | 2026-04-07 | resolve_type_ref が共通根本原因と判明、PRD 構成 4→2 に統合 |
| Phase 1a: PRD-A 起票 | done | 2026-04-07 | backlog/I-383-resolve-type-ref-three-tier.md |
| T1.A: PRD-A (= I-383, resolve_type_ref 3 階層化) | done | 2026-04-07 | 完成。ただし T6 (Step 3) は別 PRD-A-2 に分離 |
| Phase 1a-2: PRD-A-2 起票 | **done** | 2026-04-07 | backlog/I-386-resolve-type-ref-step3-and-test-fixture-cleanup.md |
| T1.A2: PRD-A-2 (= I-386, Step 3 + 73 件 bug-affirming test 根絶) | done | 2026-04-07 | 検証エビデンス E1-E6 で発見の真偽証明済み |
| Phase 1b: PRD-B 起票 | **delayed (Option Y)** | - | PRD-A + PRD-A-2 実装後に実測ベースで起票 |
| T1.B: PRD-B (= I-382 本体, synthetic→user import 生成) | blocked by T2.A2 | - | T2.A + T2.A2 完了後の probe 再計測結果を前提 |
| Phase 2: PRD 実装 (Option Y 順序) | in progress | - | T2.A → T2.A2 → T1.B → T2.B |
| T2.A: PRD-A 実装 | **in progress** | - | T1-T5 / T7-T9 / T8' 完了。Cluster 1a 4/11 解消。残 7 件は下記 3 サブクラスタに分解 |
| T2.A-i: 外部 builtin JSON loader の scope push | **next** | - | TNext / TResult / TResult1 / TResult2 (4 件) |
| T2.A-ii: interface call signature overload merge | not started | - | E (FnSSGParamsOrSSGParams) (1 件) |
| T2.A-iii: utility type / typedef resolve scope 引き継ぎ | not started | - | P, S (2 件) — 追加 probe 調査要 |
| T2.A2: PRD-A-2 実装 (Step 3 + 73 件 fixture 修正) | not started | - | T2.A 完了後 |
| T1.B: PRD-B 起票 (T2.A + T2.A2 後) | not started | - | PRD-A + PRD-A-2 完了後の probe 実測ベース |
| T2.B: PRD-B 実装 | not started | - | T1.B 完了後 |
| T2.checkpoint: 中間検証 | not started | - | probe 全件 0 確認 |
| Phase 3: I-382 本体実装 | not started | - | - |
| T3.1: import 生成ロジック追加 | not started | - | - |
| T3.2: `generate_stub_structs` 削除 | not started | - | - |
| T3.3: regression テスト追加 | not started | - | - |
| Phase 4: クロージング | not started | - | - |
| T4.1: ドキュメント整理 | not started | - | - |
| T4.2: 最終 quality check | not started | - | - |

---

## Phase 0: 徹底調査

### T0.1: (b) 型パラメータ leak の発生箇所特定

**作業内容**:

1. probe 用 instrumentation を `external_struct_generator::generate_stub_structs` に再投入し、空 stub fallback を発生させた IR ノードのスタックトレース相当 (発生時の `items` の内容、参照元 file path) を出力できる形式に拡張
2. 9 件 (`E`, `M`, `P`, `S`, `U`, `TNext`, `TResult`, `TResult1`, `TResult2`) を probe 出力から特定し、それぞれが含まれていた Hono fixture を確定
3. 各 fixture の TS source を読み、対応する型パラメータ宣言箇所を特定 (generic function / class の type_params)
4. transformer / type_collector / generic 解決経路で `RustType::Named { name: <その文字> }` を construct する call site を grep で全列挙
5. 各 call site で「型パラメータ scope に該当する文字があるか」を判定するロジックの欠落を特定
6. 単一根本原因 (例: 1 関数の bug) か 複数根本原因 (sub-cluster) かを判定
7. probe instrumentation を撤去

**完了条件**:
- `report/i382/type-param-leak.md` が以下を全て含む状態で存在:
  - 9 件それぞれの (発生 fixture path, 発生 TS 行, 推定 root cause file:line) の表
  - 単一根本原因か複数か明記
  - 単一根本原因の場合: 修正方針案 (どの関数を変更するか)
  - 複数根本原因の場合: sub-cluster 分類と各々の方針案
- master-plan.md の進捗テーブルを `done` に更新

**Depends on**: なし

---

### T0.2: (c) 不明識別子 4 件の正体特定

**作業内容**:

1. `OutputType`, `Status`, `__type`, `symbol` の 4 件について、probe 出力と Hono ソース grep を組み合わせて発生 fixture を特定
2. 各識別子について TS の正体を判定:
   - `symbol`: TS primitive `symbol` 由来か、それとも user 識別子か (大文字/小文字に注目)
   - `__type`: TS compiler の anonymous function type marker の leak か、user 識別子か
   - `OutputType`: user 定義 type / class / lib.dom / npm package のいずれか (Hono 全 ts ファイル grep で出現箇所確認)
   - `Status`: 同上
3. 各々の現在の変換経路をトレースし root cause を特定:
   - parser → transformer のどこで `RustType::Named { name }` に化けるか
   - 本来あるべき変換 (=理想的な変換先) を判定
4. 各 root cause が PRD-α (T0.1) と同根か独立かを判定

**完了条件**:
- `report/i382/unknown-identifiers.md` が以下を含む:
  - 4 件それぞれの (TS 上の正体, 発生 fixture, 発生経路 file:line, 必要な対応方針) の表
  - PRD-α と統合可能か独立 PRD が必要かの判定
- master-plan.md 進捗を `done` に更新

**Depends on**: なし

---

### T0.3: (a) Web API/DOM 型 16 件の参照源と理想的処理方針

**作業内容**:

1. 16 件 (`HTMLCanvasElement`, `WebGLRenderingContext`, `Window`, `ImageBitmap`, `RTCDataChannel`, `ServiceWorker`, `VideoFrame`, `AudioData`, `CanvasGradient`, `CanvasPattern`, `BufferSource`, `HeadersInit`, `RequestInfo`, `MediaSourceHandle`, `TemplateStringsArray`, `SVGImageElement`) について Hono 全 ts ファイルを grep し、参照箇所を全件列挙
2. 参照文脈を分類:
   - (i) 関数引数 / 戻り値の型注釈
   - (ii) union 型の constituent
   - (iii) generic 引数
   - (iv) JSDoc / type alias 経由
3. それぞれの理想的処理方針候補を評価:
   - **案 i (lib.dom 型 registry + 明示エラー)**: TypeRegistry に lib.dom 由来の型集合を組み込み、参照を「外部型・変換不可」として明示エラー化。conversion-correctness-priority Tier 3。silent stub と異なり可視化される
   - **案 ii (Rust 等価マッピング)**: 各型に Rust 対応物 (例: `BufferSource` → `Vec<u8>`)。type-fallback-safety で safe か検証必要
   - **案 iii (Any/`serde_json::Value`)**: 全てを Any に。type-fallback-safety で UNSAFE になる可能性が高い
4. conversion-feasibility / type-fallback-safety / conversion-correctness-priority に照らして案ごとの安全性を判定
5. 各型に対する推奨案を確定 (16 件で異なる案でも可)

**完了条件**:
- `report/i382/dom-types.md` が以下を含む:
  - 16 件それぞれの (参照 fixture path:line, 参照文脈分類, 推奨案, 推奨理由, ベンチ影響予測) の表
  - 全体方針 (新 lib.dom registry を導入するか、ad hoc に case-by-case 対応するか) の確定と理由
- master-plan.md 進捗を `done` に更新

**Depends on**: なし

---

### T0.4: (d) user 定義型参照の網羅マッピング

**作業内容**:

1. probe を再投入し、`defined_elsewhere_names` を一時的に空 set にして再実行 — これにより現状 exclusion で握り潰されている user 定義型 stub も全て可視化
2. 結果から、空 stub 化された全 user 定義型を列挙
3. 各 user 定義型について:
   - 定義 file (Hono 内のどの ts ファイルに定義された type/interface/class か)
   - 参照する synthetic_item (どの synthetic_item の field/variant が参照するか)
   - `ModuleGraph::module_path(file)` で解決される Rust モジュールパス
   - 必要な `use` 文 (`use crate::<path>::<Type>;`)
4. inline 配置と shared 配置で必要な import 文の差異を整理:
   - synthetic が inline 配置される場合 (= 1 ファイルからのみ参照): その file が user 定義 file と同一なら import 不要、異なるなら inline 先に import 必要
   - synthetic が shared 配置される場合: shared_types.rs に import 必要
5. ModuleGraph API で全件解決可能かの判定 (解決不能ケースがあれば追加調査タスクを提案)
6. probe instrumentation を撤去

**完了条件**:
- `report/i382/user-defined-refs.md` が以下を含む:
  - user 定義型参照の全リスト (件数 N の確定)
  - 各々の (型名, 定義 file, 参照 synthetic, 解決パス, 必要 use 文) のマッピング表
  - ModuleGraph で解決不能なケースの有無 (あれば対応方針)
  - inline / shared それぞれの import 注入先設計の概要
- master-plan.md 進捗を `done` に更新

**Depends on**: なし

---

### T0.5: 統合と PRD 化計画確定

**作業内容**:

1. T0.1〜T0.4 の成果物を統合し、根本原因クラスタを最終確定
2. 各クラスタを「単一バグ修正で済む」「設計が必要 (= 独立 PRD 化)」に分類
3. サブ PRD の依存順序を再検証 — Phase 0 結果で当初の α→β→γ→δ 順が変わる可能性を評価
4. 各サブ PRD の Goal を「probe で X カテゴリ 0 件」という測定可能基準で確定
5. master-plan.md の Phase 1〜4 を Phase 0 結果で更新 (PRD 数や順序が変わる場合)

**完了条件**:
- master-plan.md にサブ PRD 構成 (PRD 数、依存グラフ、各 Goal) が確定し記載されている
- TODO に新規 issue (I-383, I-384, ...) として登録
- master-plan.md 進捗を `done` に更新

**Depends on**: T0.1, T0.2, T0.3, T0.4

---

## Phase 1: サブ PRD 作成

### T1.α / T1.β / T1.γ / T1.δ: 各サブ PRD の作成

**作業内容**:

各サブ PRD について `/prd-template` 手順を厳格適用:
1. Discovery (clarification questions)
2. Impact Area Code Review (production + test coverage)
3. PRD Drafting (Background / Goal / Scope / Design / Design Integrity Review / Semantic Safety Analysis / Task List / Test Plan / Completion Criteria)

**完了条件**:
- 各 PRD ファイルが `backlog/I-XXX-<title>.md` に存在
- `/prd-template` 必須セクション全て記載
- master-plan.md 進捗を `done` に更新

**Depends on**: T0.5

---

## Phase 2: サブ PRD 実装

### T2.α / T2.β / T2.γ: 各サブ PRD 実装

**作業内容**: TDD で各 PRD を完遂

**完了条件 (各)**:
- 対象カテゴリが probe で 0 件
- /quality-check 通過 (clippy / fmt / test 全 pass)
- Hono ベンチ regression 0 件
- 該当 PRD の completion criteria 全て満足
- master-plan.md 進捗を `done` に更新

**Depends on**: T1.α (= T2.α), T2.α (= T2.β), T2.β (= T2.γ)

### T2.checkpoint: 中間検証

**作業内容**:

1. probe を再投入して全カテゴリ集計
2. (a)+(b)+(c) = 0 件、残るのは (d) user 定義型のみ (T0.4 で確定した数) であることを確認
3. 0 件でない場合: 見落とし root cause を Phase 0 に差し戻し追加調査
4. probe instrumentation を撤去

**完了条件**:
- probe 出力で `empty stub fallback` 行が user 定義型のみ (T0.4 でリスト化したもの) に一致
- master-plan.md 進捗を `done` に更新

**Depends on**: T2.γ

---

## Phase 3: I-382 本体実装

### T3.1: import 生成ロジック追加

**作業内容**:

1. PRD-δ の設計に従い、Pass 5c (または `OutputWriter::resolve_synthetic_placement`) に新ロジック追加:
   - synthetic_items が参照する全 type name を `TypeRefCollector` で収集
   - `ModuleGraph::module_path` で各 user 定義型の所属モジュール解決
   - shared_types.rs / 各 inline 配置先に `use crate::<path>::Type;` を注入
2. 既存 `build_shared_imports` (synthetic 型自身の shared import) と新ロジック (user 型の import) の責務分離を保つ
3. TDD: 正常系 / 多階層 path / inline と shared の混在 / 同名衝突 / synthetic が user file と同一配置 のテストを追加

**完了条件**:
- 新規テスト全 pass
- 既存テスト全 pass
- `HTTPException` 等の user 定義型が import 経由で解決されコンパイル可能
- master-plan.md 進捗を `done` に更新

**Depends on**: T2.checkpoint

### T3.2: `generate_stub_structs` 完全削除

**作業内容**:

1. `external_struct_generator/mod.rs` から `generate_stub_structs` 関数 / `defined_elsewhere_names` 引数を削除
2. `pipeline/mod.rs::collect_user_defined_type_names` を T3.1 の import 解決ロジックに統合 / 削除
3. 関連テスト (`tests/undefined_refs_tests.rs::test_generate_stub_structs_*`) を削除または import test に置換
4. Pass 5c に「dangling reference 残存検出 → panic」を追加 (regression detector)
5. doc コメント / 関連コメントから「stub」「band-aid」「I-382」言及を整理

**完了条件**:
- `generate_stub_structs` の grep ヒット 0 件
- `defined_elsewhere_names` の grep ヒット 0 件
- Hono ベンチ regression 0 件
- 全 test pass / clippy / fmt 0 warnings
- master-plan.md 進捗を `done` に更新

**Depends on**: T3.1

### T3.3: regression テスト追加

**作業内容**:

1. `synthetic_items` が user 定義型を参照する典型ケース (HTTPException 風) の integration test を `tests/` に追加
2. 削除禁止コメントを付与
3. master-plan.md 「設計判断」セクションに synthetic ↔ user import 規約を追記

**完了条件**:
- 新 integration test が pass
- master-plan.md 進捗を `done` に更新

**Depends on**: T3.2

---

## Phase 4: クロージング

### T4.1: ドキュメント整理

**作業内容**:

1. `plan.md` から「次のアクション: I-382」記述を削除し、次ターゲット (Batch 11b) に更新
2. `TODO` から I-382 / I-383..385 関連 entry を削除し、完了履歴 1 行のみ残す
3. plan.md 「設計判断」セクションに I-382 で確立した規約を追記
4. report/i382/ の各 .md を最新化

**完了条件**:
- plan.md / TODO の I-382 言及が完了履歴のみ
- master-plan.md 進捗を `done` に更新

**Depends on**: T3.3

### T4.2: 最終 quality check

**作業内容**:

1. /quality-check 実行
2. /refactoring-check 実行
3. Hono ベンチ確定値を `bench-history.jsonl` に追記
4. master-plan.md の最終ステータスを完了に更新

**完了条件**:
- 全 phase の進捗テーブルが `done`
- bench-history.jsonl に新 entry
- 0 errors / 0 warnings

**Depends on**: T4.1

---

## 現在のセッション状態 (paused: 2026-04-07)

### T2.A (PRD-A 実装) 詳細進捗

#### 完了済みタスク
- **T1-T2**: `extract_used_type_params` 共通ヘルパー抽出 + `register_union` / `register_struct_dedup` / `register_intersection_enum` の 3 関数で type_param 伝播を統一 (`src/pipeline/synthetic_registry/mod.rs`)
- **T3**: `push_type_param_scope` の append-merge 意味論変更 (ネスト scope 対応)
- **T4**: `is_in_type_param_scope` 公開 API 追加
- **T5**: `external_struct_generator::collect_undefined_refs_inner` の `Item::Enum` type_param 漏れ修正
- **T6**: **分離済み** (PRD-A-2 = I-386 に移管)
- **T7**: `convert_fn_decl` (関数 generic), `convert_arrow_expr_with_return_type` (arrow generic) の `push_type_param_scope` 補完
- **T8**: `extract_class_info` (class generic), `build_method` (method generic) の append-merge push
- **T9**: `convert_method_signature` (interface method generic) の push
- **T8'** (追加タスク, T8 拡張):
  - `MethodSignature` 構造体に `type_params: Vec<TypeParam<T>>` フィールド追加
  - `MethodSignature<T>` に手動 `Default` 実装追加
  - `src/registry/collection.rs::extract_class` で `method.function.type_params` から抽出
  - `src/registry/interfaces.rs::build_method_signature` に `type_params_decl` 引数追加 + caller 3 箇所更新
  - `src/ts_type_info/resolve/typedef.rs::resolve_method_sig` で push/restore + `type_params` の constraint 解決
  - `src/pipeline/type_converter/interfaces.rs::convert_interface_as_fn_type` で call signature generic を抽出し `Item::TypeAlias.type_params` に merge
  - Test fixture 36 箇所を Python スクリプトで一括更新 (`type_params: vec![],` 挿入)
  - production 4 箇所を手動更新 (collection.rs, interfaces.rs, external_types/mod.rs)

#### 現状の確証
- `cargo test --lib`: **2225 passed, 0 failed** (T7 + T8 検証用 integration test 2 件追加)
- `cargo build --release`: 成功
- Hono 158 fixture probe 実測: dangling refs 30 件 (当初 34 件から **4 件削減**)

#### Cluster 1a 解消状況 (当初 11 件)
| 識別子 | 状態 | 発生経路 |
|---|---|---|
| `M` | ✅ 解消 | class method (Hono.on) の call signature generic |
| `Status` | ✅ 解消 | `ExtractSchemaForStatusCode<T, Status>` 関数 generic |
| `OutputType` | ✅ 解消 | validator 関数 generic |
| `U` | ✅ 解消 | context.ts の関数 generic |
| `E` | ❌ **残存** | 追跡中 |
| `P` | ❌ **残存** | 追跡中 |
| `S` | ❌ **残存** | 追跡中 |
| `TNext` | ❌ **残存** | Promise.then 関連 |
| `TResult` | ❌ **残存** | aws-lambda `Handler<TEvent, TResult>` 関連 |
| `TResult1` | ❌ **残存** | Promise.then |
| `TResult2` | ❌ **残存** | Promise.then |

---

### 残 7 件の発生経路 — 3 サブクラスタ確定 (2026-04-08 probe 再投入結果)

probe を再投入して Hono 158 fixture で referencer を特定した結果、残 7 件は **独立した 3
サブクラスタ** に分類される。それぞれ修正経路が異なるため、PRD-A 完了には 3 サブタスク
(T2.A-i / T2.A-ii / T2.A-iii) に分解して順次対応する。

#### Sub-cluster T2.A-i: 外部 builtin JSON ローダの type_param scope 欠如 (4 件)

**対象**: `TNext`, `TResult`, `TResult1`, `TResult2`

| 識別子 | referencer (synthetic name) | 由来 |
|---|---|---|
| `TNext` | `TupleOrTupleTNext` | `ecmascript.json` の `Generator/AsyncIterator.next(__0?: [] \| [TNext])` |
| `TResult1` | `PromiseLikeTResult1OrTResult1`, `TResult1OrTResult2` | `ecmascript.json` の `Promise.then<TResult1, TResult2>` |
| `TResult2` | `PromiseLikeTResult2OrTResult2`, `TResult1OrTResult2` | 同上 |
| `TResult` | `PromiseLikeTResultOrTResult`, `TOrTResult` | `ecmascript.json` の `Promise.catch<TResult>` |

**root cause**:
- `src/external_types/mod.rs::convert_external_typedef` がインターフェース型パラメータも、各メソッドシグネチャの method-level generic も `synthetic.push_type_param_scope` を **一切呼ばずに** `convert_external_type` → `convert_union_type` → `synthetic.register_union` を叩く。結果、union 由来の synthetic enum が空の `type_params: vec![]` で生成される
- 副次問題: `tools/extract-types/src/extractor.ts::extractSignature` が `sig.typeParameters` を抽出していないため、`then<TResult1, TResult2>` のような method-level generic は **JSON にも存在しない**。スキーマと抽出器の両方で対応が必要

**修正経路** (clean 対応):
1. `tools/extract-types/src/types.ts::ExternalSignature` に `type_params?: ExternalTypeParam[]` 追加
2. `extractor.ts::extractSignature` が `sig.typeParameters` から抽出
3. `npm run build && node dist/index.js ...` で `web_api.json` / `ecmascript.json` を再生成
4. `src/external_types/mod.rs::ExternalSignature` に `type_params` フィールド追加 (deserialize)
5. `convert_external_typedef` で interface 単位の `push_type_param_scope` を実施
6. `convert_external_signature` で method 単位の `push_type_param_scope` を実施 (interface scope に append-merge)
7. 抽出した method-level type_params は `MethodSignature.type_params` (T8' で追加済み) に格納

#### Sub-cluster T2.A-ii: interface call signature overload merge 漏れ (1 件)

**対象**: `E` (referencer: `FnSSGParamsOrSSGParams`)

**由来**: `helper/ssg/middleware.ts:29-34` の `interface SSGParamsMiddleware` は `<E extends Env = Env>` 付き call signature を **2 つ overload** している。

```ts
interface SSGParamsMiddleware {
  <E extends Env = Env>(generateParams: (c: Context<E>) => SSGParams | Promise<SSGParams>): MiddlewareHandler<E>
  <E extends Env = Env>(params: SSGParams): MiddlewareHandler<E>
}
```

**root cause** (仮説): `convert_interface_as_fn_type` (`interfaces.rs:158`) は `max_by_key(params.len)` で 1 つの call signature だけ選択する。`SSGParamsMiddleware` の場合、選ばれた signature の type_params は scope に push されるが、**未選択 signature が transformer の他経路で別途処理される際に scope が失われる** 可能性がある。または、union 名 `FnSSGParamsOrSSGParams` (`(c: Context<E>) => ...) | SSGParams`) は overload 2 つを「候補集合」として 1 つの union に畳み込む経路を経ており、その畳み込み時に scope が空。

**要追加調査**: probe で `register_union` 呼び出し時の `type_param_scope` を出力する trace を投入し、`FnSSGParamsOrSSGParams` 生成の正確な call site を特定する。

#### Sub-cluster T2.A-iii: utility type / typedef resolve 経路の scope 引き継ぎ漏れ (2 件)

**対象**: `P`, `S`

| 識別子 | referencer | 由来 |
|---|---|---|
| `P` | `AnyOrHashMapCustomHeaderOrRequestHeaderStringOrHashMapPOptionStringOrHashMapStringStringOrHashMapStringStringOrVecStringOrHashMapStringTOrVecT` | Hono の巨大 generic 型 (Context.req, Validator 系) で `HashMap<P, _>` を含む union |
| `S` | `MergeSchemaPathOrS` | `hono-base.ts:217` の `Hono<E, MergeSchemaPath<...> \| S, BasePath, CurrentPath>` (class method 戻り型) |

**root cause** (仮説): T7-T9 で transformer 側の class/method scope push は補完済み。それでも leak しているということは、**registry 経路 (`ts_type_info/resolve/typedef.rs`) または monomorphize_type_params 経路** で同じ class の型を再評価する際に scope が空のまま `convert_ts_type` → `register_union` を叩いている。

**要追加調査**: T2.A-i 完了後に probe trace を再投入し、`P` / `S` を含む union の register call site を特定する。具体的には:
- `register_union` 内で member に `Named { name: "P" \| "S" }` が含まれかつ `type_param_scope` に該当名が **無い** 場合に backtrace 相当を出力
- 出力された経路を辿って scope push 漏れを修正

#### 進行順序

T2.A-i (4 件) → T2.A-ii (1 件) → T2.A-iii (2 件) の順。理由:
1. T2.A-i は最も影響範囲が明確で独立性が高い (外部 JSON のみ修正)
2. T2.A-i 完了後の再 probe で T2.A-ii / iii の状況が変化する可能性 (synthetic dedup の連鎖)
3. T2.A-iii は追加調査が必須なので最後に回す

---

### 旧次セッション手順 (実行済み: Step 1-3 完了 = 上記サブクラスタ分類が成果物)

#### Step 1: probe + tracer 再投入

`src/pipeline/external_struct_generator/mod.rs::generate_stub_structs` に以下を追加:

```rust
for ref_name in &sorted {
    let referencers: Vec<String> = items
        .iter()
        .filter(|it| {
            let mut refs = HashSet::new();
            collect_type_refs_from_item(it, &mut refs);
            refs.contains(ref_name)
        })
        .filter_map(|it| match it {
            Item::Struct { name, .. }
            | Item::Enum { name, .. }
            | Item::Trait { name, .. }
            | Item::TypeAlias { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    eprintln!("[I-383-PROBE] dangling iter={iter} name={ref_name} referencers={referencers:?}");
}
```

`src/pipeline/synthetic_registry/mod.rs::register_union` に以下を追加 (残 7 件のどれかが member に含まれる場合のみ出力):

```rust
let target_names = ["E", "P", "S", "TNext", "TResult", "TResult1", "TResult2"];
if member_types.iter().any(|t| matches!(t, RustType::Named { name, .. } if target_names.contains(&name.as_str()))) {
    eprintln!(
        "[I-383-TRACE] register_union scope={:?} -> tp={:?} members={:?}",
        self.type_param_scope,
        type_params.iter().map(|tp| &tp.name).collect::<Vec<_>>(),
        member_types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>()
    );
}
```

#### Step 2: Hono 再実行して trace を取得

```bash
cargo build --release
/home/kyohei/ts_to_rs/target/release/ts_to_rs /tmp/hono-clean -o /tmp/hono-bench-output 2>/tmp/i383-next-session.log
grep "I-383-PROBE" /tmp/i383-next-session.log | sort -u
grep "I-383-TRACE" /tmp/i383-next-session.log | head -20
```

#### Step 3: 発生源の分類

残 7 件の referencer から、次のどれに該当するかを判定:
1. **class method 経路** (T8 で push したはずだが効いていない) → 別の `local_synthetic` instance 問題?
2. **interface call signature 経路** (T8' で修正したはず) → 何らかの条件で convert_interface_as_fn_type を通らない?
3. **type alias 経路** (type_aliases.rs で既に scope push 済) → 条件分岐の漏れ?
4. **完全に新経路** → 未発見の convert_ts_type 呼出し元

#### Step 4: 仮説検証と修正

分類ごとに個別対応。仮説:

- **`TResult` / `TResult1` / `TResult2`**: `aws-lambda/types.ts` の `type Callback<TResult>` (type alias) 由来。type_aliases.rs は scope push 済なので、ここで leak するのは `Promise<TResult>` 等の型引数 resolution で monomorphize 漏れの可能性
- **`TNext`**: Promise.then の標準 signature に類似。`Promise.then<TResult1, TResult2>(...)` または `AsyncIterator.next<TNext>()` の TypeScript 標準 lib の interface が、`convert_interface_*` 経路を通って type_params を失っている可能性
- **`E`, `P`, `S`**: Hono 内の巨大 generic 関数 (例: `HonoBase.on`) の複数経路で leak している可能性。Transformer 経由と resolve_typedef 経由で複数回 register されており、そのうち片方が scope 漏れ

#### Step 5: 修正後検証

各修正ごとに:
1. `cargo test --lib` 全 pass
2. probe 再実行で対象 identifier が消えることを確認
3. 残件数を `master-plan.md` に更新

**目標**: Cluster 1a **7 件すべて 0 件** に到達 → PRD-A (T2.A) 完了条件達成 → T1.B (PRD-B 起票) に進む

---

### 現在の編集ファイル一覧 (次セッション開始前の状態)

以下のファイルが T2.A の一部として変更済み (git diff で確認可能):
- `src/pipeline/synthetic_registry/mod.rs` — extract_used_type_params + push_type_param_scope append-merge + is_in_type_param_scope
- `src/pipeline/synthetic_registry/tests.rs` — 新規テスト 6 件
- `src/pipeline/external_struct_generator/mod.rs` — Item::Enum 漏れ修正
- `src/pipeline/external_struct_generator/tests/undefined_refs_tests.rs` — 新規テスト 1 件
- `src/transformer/functions/mod.rs` — convert_fn_decl の scope push
- `src/transformer/functions/tests/fn_decl.rs` — 新規テスト 2 件
- `src/transformer/expressions/functions.rs` — convert_arrow_expr の scope push + inner 分離
- `src/transformer/classes/mod.rs` — extract_class_info の class scope push + inner 分離
- `src/transformer/classes/members.rs` — build_method の method scope push + inner 分離
- `src/pipeline/type_converter/interfaces.rs` — convert_method_signature + convert_interface_as_fn_type の scope push + build_method_signature の signature 拡張
- `src/registry/mod.rs` — MethodSignature.type_params フィールド + Default 実装
- `src/registry/collection.rs` — extract_class での method type_params 抽出
- `src/ts_type_info/resolve/typedef.rs` — resolve_method_sig の scope push + type_params 解決
- `src/external_types/mod.rs` — MethodSignature 構築更新
- `src/registry/interfaces.rs` — build_method_signature に type_params_decl 引数追加
- 合計 36 件の test fixture ファイル (MethodSignature 構築箇所の `type_params: vec![],` 追加)

---

## 計画見直しチェックポイント

各 Phase / Task 完了時に以下を確認:

1. **進捗テーブル更新**: 該当行を `done` + 完了日記入
2. **漏れチェック**: 完了時点で発見した新 root cause / 新 sub-task を計画に追記
3. **依存関係再評価**: 後続 task の前提が崩れていないか確認
4. **次タスクの prerequisites 確認**: 開始前条件が満たされているか確認
