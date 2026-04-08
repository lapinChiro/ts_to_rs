# I-382 マスタープラン

**目標**: `src/pipeline/external_struct_generator/mod.rs::generate_stub_structs` を
完全削除し、Pass 5c を「synthetic_items が参照する user 定義型に対する `use crate::<path>::Type;`
生成」のみに置き換える。

**最上位原則**: `.claude/rules/ideal-implementation-primacy.md` に従い、ベンチ数値ではなく
「理想的な TS→Rust トランスパイラ」を判断基準とする。

> 完了済タスクの履歴と背景は [`history.md`](./history.md) 参照。本ファイルは現状と今後の計画のみを扱う。

---

## 現状 (2026-04-08)

### 達成済の土台

- **Cluster 1a (型パラメータ leak) 11/11 解消** — T2.A 完了
- Hono 158 fixture での dangling refs: **23 件** に減少 (当初 34 → 23)
- `cargo test --lib`: 2228 passed, 0 failed
- Hono ベンチ regression 0 維持

### 残存 dangling refs (23 件)

| Cluster | 件数 | 識別子例 | 想定 root cause |
|---|---|---|---|
| Cluster 1b (DOM) | 16 | HTMLCanvasElement, Window, ImageBitmap, ... | **未検証** → INV-1 |
| Cluster 1c (unknown) | 2 | `__type`, `symbol` | **未検証** → INV-2 |
| Cluster 2 (user-defined) | 1+ | HTTPException, ... | **73 件まで拡大見込** (T0.4 時点計測) → INV-3 |

### 重大な認識: 今セッションの修正は patch である

T2.A-i / T2.A-ii / T2.A-iv の修正はすべて「scope push の補完」という症状対処で、
**単一の IR 設計欠陥** に対する patch である:

> **`RustType::Named { name }` が「type variable」と「named type」を区別しない**

詳細な因果関係と理想的な解決策 (`RustType::TypeVar` 変種導入) は後述。

---

## 計画フェーズ構成

旧 Option Y 計画 (T2.A → T2.A2 → T1.B → T2.B) は、TypeVar refactoring の発見により
再構成する。新計画は **調査 → 設計 → 理想実装 → I-382 本体** の 4 フェーズ。

### Phase A: 調査 (Investigation Debt 解消) ✅ **完了 (2026-04-08)**

**成果**: 9 件全 INV を fact ベースで解消。詳細は
[`phase-a-findings.md`](./phase-a-findings.md)。主要結論:

- TypeVar 導入の primary 変更点は `type_converter/mod.rs::convert_ts_type` 1 箇所
- `RustType::Named` 構築 251 件中、書換対象は ~30 件 (type_params.iter() 由来)
- 独立 PRD 候補: **PRD-β** (`TypeDef::ExternalUnsupported` variant、17 件解消)、
  **PRD-γ** (`__type` marker 是正、1 件)、**PRD-δ** (Pass 5c 再設計 = Phase D 本体)
- Phase C 並行可能な独立タスクは PRD-β/γ のみ

**目的**: 理想実装の影響範囲を絞り込めるレベルまで不確定要素を潰す。
`todo-prioritization.md` Step 0 の調査債務解消に相当。

| ID | 調査項目 | 方法 |
|---|---|---|
| **INV-1** | DOM 型 (Cluster 1b) の root cause 検証 | probe で `collect_undefined_type_references` の filter 前後を対比、web_api.json に当該型が登録されているか確認 |
| **INV-2** | `__type`, `symbol` の発生経路特定 | probe + grep で synthetic_items 内の該当参照を列挙、origin を trace |
| **INV-3** | user 定義型参照の真の件数と分布計測 | `defined_elsewhere_names` を一時的に空 set にして probe 再実行、全 user 型参照を列挙 |
| **INV-4** | `SSGParamsMiddleware` → `Fn` flatten 経路の特定 | convert_ts_type 内で interface call signature を flatten する site を trace (T2.A-iv の interim patch 削除条件) |
| **INV-5** | `RustType::Named` 構築サイトの全列挙 | grep で `RustType::Named {` を全件抽出し分類 (TypeVar refactoring の影響範囲) |
| **INV-6** | `push_type_param_scope` / `type_param_constraints` 参照サイトの全列挙 | 同上 |
| **INV-7** | `monomorphize_type_params` / `apply_substitutions_to_items` の semantics | 実装 read + テストケース特定 |
| **INV-8** | TypeResolver / Transformer / type_collector / registry の責務分界 | pipeline/mod.rs / 各モジュール doc comment を read |
| **INV-9** | utility type (Omit / Pick / Record / conditional) の展開完全性 | mapped_type.rs / intersections.rs / type_aliases.rs を read + probe |

**完了条件**: 上記 INV 全件に **fact ベースの回答** が存在し、Phase B の PRD spec が
assumption なしで書ける状態。

### Phase B: 理想実装の設計 (PRD 起票)

**目的**: Phase A の fact に基づき、TypeVar refactoring の PRD を起票する。

| タスク | 内容 |
|---|---|
| B-1 | PRD 起票: `RustType::TypeVar` 導入 + 関連 API 再設計 |
| B-2 | 凝集度 / 責務分離 / DRY の Design Integrity Review |
| B-3 | Semantic Safety Analysis (`type-fallback-safety.md` 準拠) |
| B-4 | Impact Area Code Review (INV-5/6/7/8 の fact を反映) |

**想定される PRD 内容**:
- `RustType::TypeVar { name: String }` variant を追加
- `convert_ts_type` で scope 参照し TypeVar / Named を分岐
- `Item::Enum.type_params` を scope ベースから member 内 TypeVar collection ベースに変更
- `synthetic_registry` から `push_type_param_scope` を削除 (scope 不要化)
- `collect_free_type_vars` (interim patch) を削除し TypeVar walker で置換
- `RUST_BUILTIN_TYPES` フィルタ依存を除去 (TypeVar/Named の区別で不要化)

**副次効果 (以下の既知 TODO が構造的に解消される)**:
- T-2 (sibling constraint), T-5 (dedup first-write-wins), T-6 (expected_types 欠損),
  T-7 (builtin 型表現不統一), T-8 (free var 判定 heuristic)
- `session-todos.md` の 6 件中 5 件

### Phase C: TypeVar refactoring 実装 (TDD)

**目的**: PRD-TypeVar を TDD で実装し、T2.A-i/ii/iv の interim patch を構造的に置換。

**完了条件**:
- `cargo test --lib` 全 pass
- Hono ベンチ regression 0 (**指標であり目標ではない**)
- T2.A-i/ii/iv の interim patch コード削除 + コメント `// INTERIM:` 撤去
- session-todos.md の該当 TODO (T-2, T-5, T-6, T-7, T-8) 削除

### Phase D: I-382 本体実装

**目的**: Phase C で整備された foundation 上で、元の I-382 目標を達成する。

| タスク | 内容 |
|---|---|
| D-1 | PRD-A-2 (= I-386, 73 件 fixture 整理) 実装 |
| D-2 | PRD-B 起票 — synthetic_items → user 型 import 生成設計 |
| D-3 | PRD-B 実装 — Pass 5c の import 生成ロジック |
| D-4 | `generate_stub_structs` 完全削除 + regression test 追加 |
| D-5 | ドキュメント整理、最終 quality check |

**最終完了条件**: probe で dangling refs = 0、`generate_stub_structs` grep ヒット = 0、
Hono ベンチ regression 0。

---

## 直近アクション

**今着手すべきは Phase A** (Investigation Debt 解消)。

### 着手順序

1. **INV-5 / INV-6** を並列で grep 実施 (ファイル行数のみの軽い調査、所要時間小)
2. **INV-1 / INV-2 / INV-3** を probe 再投入で一括取得
3. **INV-4** を trace で実施 (T2.A-iv の interim patch 削除前提)
4. **INV-7 / INV-8 / INV-9** を read 中心で実施

### 推奨実行モード

Phase A は 9 件の INV 項目があり、並列化可能な項目が多いため、**`Explore` または
`Plan` subagent を使った並列調査** を推奨する。1 項目ずつ手動 read すると context
消費が大きく、Phase B で assumption ベースに戻るリスクがある。

subagent 利用のフォーマット候補:
- INV-1/2/3: probe 実行 + 結果分析を 1 subagent
- INV-4: trace 追跡を 1 subagent
- INV-5/6: grep 集計を 1 subagent
- INV-7/8/9: コード read を 1-2 subagent

---

## Interim Patch 管理

`ideal-implementation-primacy.md` に基づき、本プロジェクト中で適用中の interim patch
を明示管理する。各 patch は Phase C で削除される。

| Patch 箇所 | 削除条件 | 削除担当 Phase |
|---|---|---|
| `src/external_types/mod.rs::convert_external_typedef` の `push_type_param_scope` (T2.A-i) | `RustType::TypeVar` 導入後、convert_ts_type で自動解決 | Phase C |
| `src/pipeline/type_resolver/helpers.rs::enter_type_param_scope` (T2.A-ii) | 同上 | Phase C |
| `src/pipeline/type_resolver/helpers.rs::collect_free_type_vars` (T2.A-iv) | 同上 | Phase C |
| `tools/extract-types/src/extractor.ts::convertType` intersection → `any` (T-1) | `ExternalType::intersection` variant 導入後 | Phase D 以降の別 PRD |

---

## 参照

- 完了履歴: [`history.md`](./history.md)
- Phase 0 調査レポート群: `phase0-synthesis.md`, `type-param-leak.md`, `dom-types.md`, `user-defined-refs.md`, `unknown-identifiers.md`
- セッション発見 TODO: [`session-todos.md`](./session-todos.md)
- 最上位原則: `.claude/rules/ideal-implementation-primacy.md`
- 優先度ルール: `.claude/rules/todo-prioritization.md`
