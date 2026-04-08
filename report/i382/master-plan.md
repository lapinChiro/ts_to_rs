# I-382 マスタープラン

**目標**: `src/pipeline/external_struct_generator/mod.rs::generate_stub_structs` を
完全削除し、Pass 5c を「synthetic_items が参照する user 定義型に対する `use crate::<path>::Type;`
生成」のみに置き換える。

**最上位原則**: `.claude/rules/ideal-implementation-primacy.md` に従い、ベンチ数値ではなく
「理想的な TS→Rust トランスパイラ」を判断基準とする。

> 完了済タスクの履歴と背景は [`history.md`](./history.md) 参照。本ファイルは現状と今後の計画のみを扱う。

---

## 現状 (2026-04-08, I-387 Phase C 実装中)

### 達成済の土台

- Phase A (INV-1〜9 調査債務) ✅ 完了
- Phase B (PRD I-387 起票 + Design Integrity Review) ✅ 完了
- Phase C (I-387 実装) 🔄 T1〜T6 + T8〜T12 完了、T7/T13/T14 残
- **IR 設計欠陥の構造的解消**: `RustType::Named` から `TypeVar` / `Primitive` /
  `StdCollection` を分離し、production の Named は user 定義型のみを表す状態を達成
- `cargo test --lib`: **2259 passed, 0 failed** (+31 新規)
- Hono ベンチ regression 0 維持 (clean 114/158, errors 54, compile dir 99.4%)
- Phase A の Cluster 1a 11 件解消は維持

### 残存 dangling refs (Phase D スコープ)

Phase A 調査時点の 23 件内訳 (Phase C 内では再計測未実施、T14 で検証予定):

| Cluster | 件数 | 識別子例 | Phase D スコープ |
|---|---|---|---|
| Cluster 1b (DOM) | 16 | HTMLCanvasElement, Window, ImageBitmap, ... | **PRD-β** (`TypeDef::ExternalUnsupported` variant) |
| Cluster 1c (unknown) | 2 | `__type`, `symbol` | **PRD-γ** (`__type` 是正) |
| Cluster 2 (user-defined) | 73 (filter なし計測) | HTTPException, Context, ... | **PRD-δ** (Pass 5c 再設計 = I-382 本体) |

### IR 設計欠陥 → 構造的解決

> **旧**: `RustType::Named { name }` が「type variable」「user type」「std type name」を区別しない
>
> **新 (I-387)**: `TypeVar { name }` + `Primitive(PrimitiveIntKind)` +
> `StdCollection { kind, args }` + `Named` (user 専用) に構造化分離

これにより Phase A 調査時点の「interim patch 3 件 (T2.A-i/ii/iv)」は以下のように処理された:

- **T2.A-iv の heuristic 部分** (`collect_free_type_vars`): 完全削除 → `collect_type_vars`
  TypeVar walker で置換 (PRD Goal #7 達成)
- **T2.A-i / T2.A-ii の scope push**: **correct lexical scope management として残置** と
  判定。post-I-387 でも `convert_ts_type` / `convert_external_type` が scope を参照して
  TypeVar routing するため、scope 自体は削除不可。コメントから "INTERIM" 注釈を撤去し
  「I-387 lexical scope semantics」に relabel
- `RUST_BUILTIN_TYPES` 定数: 完全削除 (Named が user type のみになり文字列フィルタ不要)

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

### Phase C: I-387 実装 (TDD) 🔄 **実装中 (2026-04-08)**

**目的**: PRD I-387 を TDD で実装し、`RustType` を構造化して interim heuristic を削除。

**進捗**:

| タスク | 内容 | 状態 |
|---|---|---|
| T1 | `TypeVar` / `Primitive` / `StdCollection` variant 追加 + 6 テスト | ✅ |
| T2 | substitute に TypeVar branch 追加 + 5 テスト (legacy Named{"T"} 後方互換は残置) | ✅ |
| T3 | generator に新 variant 生成 + 10 テスト (Semantic Safety 等価性 3 件含む) | ✅ |
| T4a | `primitive_int_kind_from_name` / `std_collection_kind_from_name` ヘルパー + 5 テスト | ✅ |
| T4b | TypeVar routing + 下流両対応化 (type_resolver) + 2 テスト | ✅ |
| T4c | Primitive/StdCollection routing + 下流両対応化 (transformer) + 3 テスト | ✅ |
| T4d | BigInt / Record / Map / Set の構造化 routing | ✅ |
| T5 | (c1) 既存 variant 巻戻し — 3 production sites | ✅ |
| T6 | (c2) Primitive/StdCollection 構築サイト置換 | ✅ |
| T7 | (b) TypeVar 構築サイト置換 | 🔄 production 完了、**test fixtures 残** |
| T8 | T2.A-i 処理 — scope push は lexical scope として残置、heuristic は walker で置換 | ✅ |
| T9 | T2.A-ii 処理 — enter_type_param_scope を relabel | ✅ |
| T10 | `collect_free_type_vars` 削除 + `collect_type_vars` walker 導入 | ✅ |
| T11 | `extract_used_type_params` を walker-only 実装に | ✅ |
| T12 | 下流 pattern match 更新 (T4b/T4c に統合済) | ✅ |
| T13 | plan.md / master-plan.md / history.md 更新 | 🔄 本回実施中 |
| T14 | /quality-check + Hono bench 最終確認 | ⏳ 未実施 |

**完了条件**:
- ✅ `cargo test --lib` 全 pass (2259 件)
- ✅ Hono ベンチ regression 0
- ✅ `collect_free_type_vars` / `RUST_BUILTIN_TYPES` の heuristic 削除 (PRD Goal #7)
- 🔄 substitute の legacy Named{"T"} 後方互換ブランチ削除 (T7 残)
- 🔄 test fixtures の `Named{"T"}` → `TypeVar{"T"}` 一括置換 (T7 残)
- ⏳ `/quality-check` 通過確認 (T14)
- ⏳ session-todos.md の該当 TODO 削除 (T-2, T-5, T-6, T-7, T-8)

### T8 設計判断の変更 (重要)

PRD 起票時点では「T2.A-i / T2.A-ii の `push_type_param_scope` 呼び出しを**完全削除**」と
想定していたが、実装調査の結果、以下の architectural insight を得て方針変更した:

- `convert_external_type` (外部 JSON ローダ) と `convert_ts_type` (SWC AST コンバータ) は
  互いに独立した 2 つの変換経路で、`convert_ts_type` の TypeVar routing を後者が直接
  流用することはできない
- `convert_external_type::Named` も scope を参照して TypeVar routing する必要があり、
  scope 自体は「lexical scope management」として残すのが構造的に正しい
- 「interim」だったのは scope を介してフィルタ判定していた
  `extract_used_type_params` の heuristic 部分であり、それは walker-only 実装で完全置換

この判断は「scope push を残すと interim の条件を満たさないのでは」という懸念に対する
回答として master-plan に明記する。walker 化で heuristic が構造的に除去された時点で、
scope push は **correct design** となり interim ではなくなる。

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

## 直近アクション (次セッション開始時)

**Phase C 残作業を優先順に実施**:

### 1. T7 残り: substitute 後方互換削除 + test fixtures 一括更新

**着手手順**:
1. `src/ir/substitute.rs::fold_rust_type` の 2 本目 `if let RustType::Named { name, type_args } = &ty` ブランチ (L43-52 付近、後方互換用) を削除
2. `cargo test --lib` 実行 → 壊れる test fixtures をリストアップ
3. `bulk-edit-safety.md` 準拠で grep ベースのドライラン → 置換対象確認:
   - `src/registry/tests/generics.rs` 内の `RustType::Named { name: "T"/"E"/... }` 等
   - `src/generator/tests.rs:111, 401-407, 781-782, 820-821` 等
   - `src/ir/fold_tests.rs:102-103, 183-184, 385-386, 389-390`
   - `src/ts_type_info/resolve/typedef.rs` の `apply_substitutions_*` テスト (L781-921)
4. `Named{"T", type_args: vec![]}` → `TypeVar{"T"}` へ一括置換 (型引数なしの単一名のみ)
5. `cargo test --lib` 全 pass、`cargo clippy` 0 warning 確認

**注意**: substitute の後方互換ブランチは現在 2 セクション (L34-49) に残存:
```rust
if let RustType::TypeVar { name } = &ty { ... }  // 本体
if let RustType::Named { name, type_args } = &ty { ... }  // ← これを削除
```

### 2. T14: 最終 quality check

1. `/quality-check` (cargo fix → fmt → clippy → test)
2. `./scripts/hono-bench.sh` 再実行 (clean files / error instances / compile rates)
3. Phase A の probe 再投入で dangling refs 件数再計測 (Cluster 1a 継続 0 / 全体 23 維持を確認)

### 3. T13 仕上げ: ドキュメント最終化

1. `report/i382/history.md` に Phase C 完了エントリ追加
2. `report/i382/session-todos.md` から I-387 で解消された項目 (T-2, T-5, T-6, T-7, T-8) を削除
3. `backlog/I-387-rust-type-structural-refinement.md` を archive (完了処理)

### 4. PRD-β / PRD-γ / PRD-δ 起票 (Phase D 準備)

1. **PRD-β**: `TypeDef::ExternalUnsupported` variant 導入 (DOM 型 16 件 + symbol 1 件解消)
2. **PRD-γ**: `__type` marker → function type 是正 (1 件解消)
3. **PRD-δ**: Pass 5c 再設計 = `generate_stub_structs` 削除 + user 型 import 生成 (I-382 本体)

---

## セッション中断時点のスナップショット (2026-04-08)

### 検証結果

- `cargo test --lib`: **2259 passed / 0 failed**
- `cargo clippy --all-targets --all-features`: **0 warning**
- Hono bench: **clean 114/158, errors 54, compile (dir) 99.4% — regression 0**
- PRD Goal #4 / #7 達成 (interim heuristic 削除、`RUST_BUILTIN_TYPES` 削除)

### Interim patch 最終状態

旧 "interim" 管理表は I-387 完了により責務転換:

| 元項目 | 現在の状態 |
|---|---|
| `convert_external_typedef` の `push_type_param_scope` (T2.A-i) | **残置** (correct lexical scope management に re-label)、heuristic 部分は walker で置換済 |
| `enter_type_param_scope` (T2.A-ii) | **残置** (同上)、doc comment を I-387 semantics に更新済 |
| `collect_free_type_vars` (T2.A-iv) | **削除済** → `collect_type_vars` TypeVar walker |
| `RUST_BUILTIN_TYPES` 文字列フィルタ | **削除済** (IR 構造化で不要) |
| `tools/extract-types/src/extractor.ts::convertType` intersection → `any` (T-1) | 残存、Phase D 以降の別 PRD で対応 |

### 残 interim (Phase D 以降)

| Patch 箇所 | 削除条件 | 対応 PRD |
|---|---|---|
| `tools/extract-types/src/extractor.ts::convertType` intersection → `any` (T-1) | `ExternalType::Intersection` variant 導入後 | Phase D 以降の別 PRD |

---

## 参照

- 完了履歴: [`history.md`](./history.md)
- Phase 0 調査レポート群: `phase0-synthesis.md`, `type-param-leak.md`, `dom-types.md`, `user-defined-refs.md`, `unknown-identifiers.md`
- セッション発見 TODO: [`session-todos.md`](./session-todos.md)
- 最上位原則: `.claude/rules/ideal-implementation-primacy.md`
- 優先度ルール: `.claude/rules/todo-prioritization.md`
