# I-382 Phase A 調査債務解消レポート (2026-04-08)

Phase A (Investigation Debt 解消) を 4 本の並列 subagent 調査で完了。
以下は assumption を fact に置換した結果。Phase B (TypeVar refactoring PRD) は
本レポートのみを参照元に spec を書ける状態にある。

---

## INV-1: DOM 型 (Cluster 1b, 16 件) 残存の root cause

> **⚠ 訂正 (2026-04-10, D-2 実装時に判明)**: 以下の結論は不正確であった。
> 実測では 22 件中 **20 件は JSON にトップレベル定義がなく registry 未登録**
> (`is_external` = false) が正確な root cause。2 件 (HeadersInit, RequestInfo) は
> `alias` kind で JSON に存在するが `convert_external_typedef` が None を返すため未登録。
> 「external 認識は成功するが TypeDef variant が Struct でない」は誤り。
> root cause は `tools/extract-types/src/filter.ts` の `EXCLUDE_PATTERNS` が
> DOM 型を推移的依存追跡から除外しつつ、除外型への参照を JSON に残すフィルタ不整合。
> I-391 で修正済。

- `collect_undefined_type_references`
  (`src/pipeline/external_struct_generator/mod.rs:64-72`) は
  2 段フィルタ (除外条件 L90-143 + `registry.is_external(name)` L70)
- ~~DOM 型は `src/builtin_types/web_api.json` に登録済~~ → 実際は JSON に未登録
- ~~**root cause**: external 認識は成功するが、当該 TypeDef variant が
  `Struct` ではなく `Function` / `Enum` のため
  `generate_external_struct` が `None` を返し、empty stub fallback で
  `pub struct HTMLCanvasElement;` が生成される~~ → 訂正済 (上記参照)
- **Phase B 含意**: ~~`TypeDef::ExternalUnsupported` variant 導入~~ →
  実際の修正: `filter.ts` 参照整合性チェック + root types 追加 + alias 変換 (I-391)

## INV-2: `__type` / `symbol` 発生経路

- 両者とも `src/builtin_types/ecmascript.json` に登録された TS
  internal marker / primitive
- `__type`: TS anonymous function type marker を TypeCollector / TypeConverter が
  raw 文字列として `RustType::Named { name: "__type" }` に構築
- `symbol`: `number | string | symbol` union 展開時に primitive が
  `RustType::Named { name: "symbol" }` で焼き込まれる
- 参照元はすべて anonymous union synthetic
  (`HeadersOrVecTupleStringStringOr__type` 等) のフィールド型
- **Phase B 含意**: `__type` は TypeCollector で function type 展開へ是正
  (PRD-γ 候補)。`symbol` は PRD-β に統合

## INV-3: user 定義型参照の真の分布

- 既存レポート `user-defined-refs.md` で `defined_elsewhere_names` フィルタを
  外した計測値 = **73 件** (うち 1 件は generic type alias `H`、他 72 件は
  `MergeSchemaPath` / `HTTPExceptionFunction` / `Context` 等 Hono 内定義)
- 全件 anonymous synthetic の field 型
- 配置パターン: A = shared_types.rs 集約、B = 同一 file inline、
  C = 異 file inline (要 import)
- **Phase B 含意**: Phase D で Pass 5c を「user 型 import 生成」に置換する際、
  ModuleGraph::module_path で定義元 resolve し placement に応じた import を
  生成する必要がある

## INV-4: `SSGParamsMiddleware` → `Fn` flatten 経路

- **主経路**: `src/pipeline/type_converter/interfaces.rs:158-161`
  (`convert_interface_as_fn_type`) で call signature overload から
  `max_by_key(params.len())` により 1 つだけ選択、他破棄
- `src/pipeline/type_converter/interfaces.rs:228-235` で
  `Item::TypeAlias { ty: RustType::Fn { .. } }` を構築しインタフェース構造を消去
- **副経路**: `src/pipeline/type_resolver/helpers.rs:318-356`
  (`resolve_fn_type_info`) で expected_types 内の `Named { name }` を
  registry lookup 後、`select_overload` で再度 Fn に flatten
- 失われる情報: (1) 未選択 overload の param/return、(2) 非採用 overload の
  type_params、(3) generic binding 由来 (`<E extends Env>` が消える)
- **T2.A-iv interim patch 削除条件**: `RustType::TypeVar` 導入により
  expected_type flatten 時点で free type param を明示標識できれば、
  `collect_free_type_vars` (helpers.rs:50-111) は不要化

## INV-5: `RustType::Named {` 構築サイト全件

- 総件数: **581** (実装 251 + テスト 330)
- 分類 (実装):
  - (a) named type 確定: ~120
  - (b) 型変数になり得る: ~30
  - (c) builtin/std 型リテラル: ~150
  - (d) パターン/その他: ~180
- 代表 (b) = `transformer/classes/helpers.rs:34 name: p.name.clone()`
  (type_params.iter() 由来) — ここが TypeVar 置換 primary ターゲット
- 代表 (c) = `transformer/type_position.rs:37 "Box"`,
  `transformer/expressions/member_access.rs:21 "usize"` 等 —
  TypeVar 導入時も Named のまま据え置くべき (別途 `StdType` variant 化も検討)

## INV-6: type_param scope API 参照サイト

| API | 実装 | テスト | 役割 |
|---|---|---|---|
| `push_type_param_scope` | 18 | 9 | WRITE (push) |
| `restore_type_param_scope` | 20 | 4 | WRITE (pop) |
| `enter_type_param_scope` | 5 | 1 | COMBINED guard |
| `type_param_constraints` | 40 | 6 | READ/WRITE field |
| `is_in_type_param_scope` | 0 | 14 | READ query (テスト専用) |

- push サイト主要: external_types (2), type_converter (5),
  transformer (3), type_resolver (2), ts_type_info/typedef (2)
- scope 管理はすでに fully integrated — TypeVar 導入時は既存 push/pop を
  維持したまま `convert_ts_type` の Named 分岐点のみ置換可能

## INV-7: monomorphize / apply_substitutions semantics

- `monomorphize_type_params`
  (`src/ts_type_info/resolve/typedef.rs:348-389`):
  非 trait bound 制約を持つ型パラメータを削除し substitution map を iterative
  に生成 (chained constraint 対応)。`(Vec<TypeParam>, HashMap<String, RustType>)`
  を返す
- `apply_substitutions_to_items`
  (`src/pipeline/synthetic_registry/mod.rs:477-487`):
  monomorphization 結果を registered synthetic items 全体に `substitute(subs)`
  walk で適用。`resolve_typedef` (typedef.rs:72) で TypeDef 単位で 1 度だけ呼ぶ
- テスト: `typedef.rs:680-920` に 8 ケース
- **fact**: monomorphization は TypeDef 単位で isolated、cross-typedef
  substitution は存在しない (scope push/restore で隔離)

## INV-8: モジュール責務分界

| Module | 責務 | TypeVar 変更対象度 |
|---|---|---|
| **TypeRegistry** (`src/registry/`) | ユーザー型前方参照・interface/enum/function metadata | low |
| **TypeConverter** (`src/pipeline/type_converter/`) | TS 型 → Rust 型変換、mapped type 簡略化 | **primary** |
| **TypeResolver** (`src/pipeline/type_resolver/`) | 式型計測、narrowing、type_param_constraints 管理 | **secondary** |
| **SyntheticTypeRegistry** (`src/pipeline/synthetic_registry/`) | union/inline struct/intersection enum dedup + type_param_scope | **tertiary** (scope 仕様確認のみ) |
| **Transformer** (`src/transformer/`) | TS decl → IR Item 変換 | low (Named 構築の置換のみ) |

- 責務重複なし、各層 distinct
- TypeVar 導入 primary = `convert_ts_type` (`type_converter/mod.rs`) の
  Named 分岐点 1 箇所
- 削除対象 = `collect_free_type_vars` (interim patch)

## INV-9: utility type 展開完全性

- **supported**: identity mapped `{ [K in keyof T]: T[K] }` → `T`
  (`src/registry/intersections.rs:63-130 try_simplify_identity_mapped_type`)、
  generic type alias + monomorphization (`type_aliases.rs:5-72`)、
  intersection struct field/method merge (`intersections.rs:147-216`)
- **partial**: conditional type は true branch 代用 + TODO コメント
  (`type_aliases.rs:14-62`)
- **unsupported**: `Omit` / `Pick` / `Record` 等は `TsTypeRef("Omit")` として
  registry lookup → unresolved → dangling ref になる経路が存在
- **fact**: utility type は INV-3 の 73 件と一部重複する可能性あり。ただし
  TypeVar refactoring とは直交する (別 PRD 候補)
- TODO 箇所: `type_aliases.rs:48` conditional fallback,
  `intersections.rs:62` mapped general case

---

## Phase B 移行時の確定事項

1. **primary 変更点** = `src/pipeline/type_converter/mod.rs::convert_ts_type` の
   `TsTypeRef → RustType::Named` 分岐で、`type_param_scope` を参照して
   TypeVar / Named に二分岐
2. **Named 構築サイト 251 件中、書き換え対象は ~30 件 (分類 b)**。
   builtin/std 型 (分類 c, ~150 件) および named type 確定 (分類 a, ~120 件) は
   据え置き
3. **削除対象 interim patch**:
   - `src/external_types/mod.rs::convert_external_typedef` の push_type_param_scope (T2.A-i)
   - `src/pipeline/type_resolver/helpers.rs::enter_type_param_scope` 周辺 (T2.A-ii)
   - `src/pipeline/type_resolver/helpers.rs:50-111 collect_free_type_vars` (T2.A-iv)
4. **独立 PRD 候補** (TypeVar refactoring と並行):
   - **PRD-β**: `TypeDef::ExternalUnsupported` variant 導入
     (INV-1 DOM 型 16 件 + INV-2 symbol 1 件、合計 17 件解消)
   - **PRD-γ**: `__type` marker → function type 是正 (INV-2, 1 件)
   - **PRD-δ**: Pass 5c 再設計 = user 型 import 生成 (INV-3, 73 件 / Phase D 本体)
5. **責務分界確定** = TypeVar 導入は TypeConverter primary / TypeResolver secondary /
   SyntheticTypeRegistry tertiary。Transformer / TypeRegistry は副次的

## バッチ化可能性の検討

- TypeVar refactoring (Phase C) と PRD-β / PRD-γ は**直交**: 前者は
  `RustType::Named` 内部構造変更、後者は `TypeDef` variant 追加。
  同時実施は context 圧迫のため非推奨
- PRD-δ (Pass 5c 再設計) は Phase C 完了を前提とする (free var heuristic 除去後の
  synthetic_items が入力になる)
- **結論**: Phase B→C→D の順序維持が最適。Phase B 着手時に PRD-β/γ の起票を
  合わせて行い、Phase C と並行実装可能な独立タスクとして扱う
