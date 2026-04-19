# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-19)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 112/158 (70.9%) |
| Hono bench errors | 62 |
| cargo test (lib) | 2591 pass |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 97 pass |
| clippy | 0 warnings |
| fmt | 0 diffs |

### 直近の完了作業

完了 PRD は「1〜3 行サマリ + PRD ファイル/git history への link」で記載。実装詳細と設計判断の
引継ぎは以下の専用ドキュメントで管理:

- **設計判断アーカイブ**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
  (label hygiene / optional param / switch emission 等の convention/idiom)
- **PRD history**: `backlog/` (close 後 git history に archive) + git log

| PRD | 日付 | サマリ |
|-----|------|--------|
| **I-153 + I-154 batch** | 2026-04-19 | switch case body 内 nested bare `break` silent redirect の structural 解消 + 4 internal label (`switch/try_block/do_while/do_while_loop`) を `__ts_` prefix に統一 + 3-entry lint + A-fix (`ast::Stmt::Block` support)。empirical verify: TSX stdout `50/550/55` = Rust stdout `50/550/55` (pre-fix Rust=`0/550/55`)。追加 test: walker unit 19 / block 3 / lint 4 / per-cell E2E i153 13 + i154 3。詳細: [`backlog/I-153-switch-nested-break-label-hygiene.md`](backlog/I-153-switch-nested-break-label-hygiene.md)、report: [`report/i153-switch-nested-break-empirical.md`](report/i153-switch-nested-break-empirical.md) |
| **INV-Step4-1** | 2026-04-19 | I-142 Step 4 C-2 empirical 再分類: rustc E0308 検知の L3 Tier 2 compile error と確認 (L1 silent ではない)。I-144 CFG narrowing で structural 解消予定。report: [`report/i142-step4-inv1-closure-compile.md`](report/i142-step4-inv1-closure-compile.md) |
| **Phase A Step 4: I-023 + I-021** | 2026-04-17 | `async-await` / `discriminated-union` fixture unskip。try body `!`-typed detection + DU walker single source of truth 化 + scope-aware shadowing。Follow-up I-149〜I-157 を TODO に登録 |
| **I-SDCDF (Spec-Driven Conversion Dev Framework)** | 2026-04-17 | implementation-first → specification-first への process 転換。Beta 昇格、全 matrix-driven PRD に必須適用。rule: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md)、reference: `doc/grammar/` |
| **I-050-a: primitive Lit → Value coercion** | 2026-04-17 | SDCDF Pilot、Spec gap = 0 達成 |
| **I-142: `??=` NullishAssign Ident LHS** | 2026-04-15 | shadow-let + fusion + `get_or_insert_with`。Step 4 follow-up は [`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md) |
| **Phase A Step 3: Box wrap + implicit None** | 2026-04-17 | I-020 部分解消 + I-025 解消、`void-type` unskip |
| **I-142-b+c: FieldAccess/Index `??=`** | 2026-04-17 | `if is_none/get_or_insert_with` + HashMap `entry/or_insert_with` emission |
| **以前の完了** | — | I-022 (`??`), I-138 (Vec index Option), I-040 (optional param), I-392 (callable interface) |

### 次の作業 (empirical 再評価 2026-04-19、spec-first workflow 適用)

**優先順位は `.claude/rules/todo-prioritization.md` (L1 > L2 > L3 > L4) および
`.claude/rules/ideal-implementation-primacy.md` (silent semantic change を最優先) に従う。**

**Tier 0 (L1 silent) 該当なし** (I-153 完了により解消)。次の最優先は L2 structural foundation の I-144。

| 優先度 | レベル | PRD | 内容 | 根拠 |
|--------|-------|-----|------|------|
| 1 | **L2 Struct** | **I-144** umbrella | control-flow narrowing analyzer (I-024 complex / I-025 Option return / I-142 Cell #14 / I-142 Step 4 C-1+C-2 吸収) | C-1 scanner false-positive / C-2 closure body shadow-let 不整合は CFG narrowing で structural 解消。既存 `NarrowingEvent` infra (`pipeline/type_resolution.rs:42-56`) 拡張。scope ~800-1000 行 |
| 2 | L3 | **Phase A Step 5** (I-026 / I-029 / I-030) | 型 assertion / null as any / any-narrowing enum 変換 | `type-assertion`, `trait-coercion`, `any-type-narrowing` unskip (3 fixture 直接削減) |
| 3 | L3 | I-142 Step 4 C-5〜C-9 残余 | I-144 に吸収されない小規模 follow-up (`doc/handoff/I-142-step4-followup.md` 参照) | C-9 INV-Step4-2 は git bisect 要、user 操作待ち |
| 4 | L3 | **I-158** | Non-loop labeled stmt (`L: { ... }` / `L: switch(...)`) support | TS valid syntax の gap。I-153 完了により emission model 安定、依存解消済 |
| 5 | L3 | **I-159** | 内部 emission 変数の user namespace 衝突 (I-154 の variable 版) | `_try_result` / `_fall` / `_try_break` 等を `__ts_` prefix に統一 + 変数宣言 lint |
| 6 | L3 | I-143 meta-PRD | `??` 演算子の問題空間完全マトリクス + 8 未解決セル | I-143-a〜h 未着手。I-144 後の topology で一部 (I-143-b any ?? T) は I-050 依存 |
| 7 | L3 | I-140 | TypeDef::Alias variant 追加 | `type MaybeStr = string \| undefined` alias 経由 Option 認識失敗 |
| 8 | L3 | I-150 | `resolve_new_expr` 未登録 class args visit | no-builtin 経路 compile error (empirical 確認済) |
| 9 | L3 | I-050-b | Ident → Value coercion | TypeResolver expr_type と IR 型乖離解消が前提 |
| 10 | L4 | I-145 | `tests/compile-check/src/lib.rs` gitignore 化 | 毎 commit artifact diff |
| 11 | L4 | I-160 | Walker defense-in-depth (Expr-embedded Stmt::Break) | 現時点 reachability なし |

**注**: 本テーブルは着手順。各 PRD で `prd-template` skill + `.claude/rules/problem-space-analysis.md`
+ `.claude/rules/spec-first-prd.md` を適用する。

### Batching 検討 (2026-04-19)

- I-144 + I-142 Step 4 C-1〜C-4+D-1: ✅ batch 推奨 (I-144 CFG narrowing が scanner を structural に置換)
- I-158 + I-159: batch 検討 (namespace hygiene 系、I-154 と同系。ただし I-158 が I-153 emission model と interaction するため I-158 先行推奨)
- I-143 + I-050-b: batch 検討 (I-143-b は I-050 Any coercion 依存)
- I-140 + I-134: type alias 関連、DRY 可能性
- **I-050 umbrella** (`backlog/I-050-any-coercion-umbrella.md`) は design 母体として存続

### INV 状態

- INV-Step4-1: ✅ 完了 (`report/i142-step4-inv1-closure-compile.md`)
- INV-Step4-2: ⏸ user git 操作待ち (commit bisection 要)
- I-153 問題空間: ✅ 完了 (`report/i153-switch-nested-break-empirical.md`)

---

## 次のタスクの先行調査まとめ

「次の作業」テーブル priority 1 (I-144) の着手前調査事項。詳細な問題空間と修正方針は
PRD 起票時に SDCDF spec stage で確定する。

### I-144 umbrella: control-flow narrowing analyzer

**Empirical 根拠**: [`report/i142-step4-inv1-closure-compile.md`](report/i142-step4-inv1-closure-compile.md)
- I-142 Step 4 C-2 は rustc E0308 検知の Tier 2 compile error (L1 silent ではない)
- Shadow-let が TypeResolver scope に反映されない不整合が root cause
- 既存 `NarrowingEvent` infra を拡張して structural 解消

**吸収対象** (いずれも I-144 完了で解消):

- I-024 (`if (x)` complex truthy narrowing)
- I-025 (Option return 暗黙 None の complex case)
- I-142 Cell #14 (narrowing-reset: linear `x = null`)
- I-142 Step 4 C-1 (compound op false-positive reset)
- I-142 Step 4 C-2 (closure body reassign shadow-let 不整合)
- I-142 Step 4 C-3 + C-4 (scanner test coverage、廃止により moot)
- I-142 Step 4 D-1 (scanner call site DRY、廃止により moot)

**問題空間 (SDCDF spec stage で確定要)**:

1. `??=` 後 narrow event + narrow-reset detection (linear / if body / loop body)
2. Closure capture boundary: outer var mutation 検出 → shadow-let 採否判定
3. `typeof` / `instanceof` guards (既存 `narrowing.rs` 実装拡張)
4. switch case narrowing (I-148 shadowing infra 流用可能)
5. `if (x)` truthy narrowing (LHS 型別: Option / Any / String / Array / Number)

**実装規模**: ~800-1000 行。`pipeline/narrowing_analyzer.rs` (新規) + 既存
`type_resolver/narrowing.rs` 拡張 + `NarrowingEvent` variant 追加 (Reset / ClosureCapture)。

### I-142 Step 4 残余 (優先度 3、C-5〜C-9)

I-144 に吸収されない小規模 follow-up。[`doc/handoff/I-142-step4-followup.md`](doc/handoff/I-142-step4-followup.md) 参照。
I-144 完了後に 1 session で纏めて処理可能。

### I-158 / I-159 (優先度 4-5、I-153 完了後 derivable)

I-153 PRD から derive された hygiene 系 follow-up。詳細は TODO 参照。

### I-142 Cell 分類 (lock-in 状態)

```
I-142 Cell 分類
  ├── #1〜#4, #7, #8, #11, #13  — Step 1 で structural 解消
  ├── #6, #10, #12              — Step 2 で structural 解消
  ├── #5, #9                    — I-050 依存、compile-error lock-in test
  ├── #14 (narrowing-reset)     — I-144 依存、lock-in test
  └── FieldAccess / Index       — I-142-b+c で解消済 (2026-04-17)
```

---

## 設計判断の引継ぎ

後続 PRD 向けの設計判断アーカイブは **[`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)** に集約。

含まれる topic (要約):

- **Type scope 管理**: `push_type_param_scope` の設計理由
- **Primitive type 9 variant YAGNI 例外**
- **Switch emission と label hygiene (I-153/I-154)**: `__ts_` prefix convention、walker 設計、conditional wrap、Block flatten、is_literal_match_pattern 微変化
- **Optional param 収束設計 (I-040)**: `wrap_if_optional` 単一ヘルパー、全 10 emission 経路
- **Conversion helpers (RC-2)**: remapped methods / `produces_option_result` / strictNullChecks / FieldAccess parens
- **Error handling emission**: TryBodyRewrite exhaustive capture / I-023 short-circuit / 協調 / union return 実行順序 (RC-13)
- **DU analysis (Phase A Step 4)**: walker single source of truth / Tpl children visit
- **Lock-in テスト (削除禁止)**: 保護対象テスト一覧
- **残存 broken window**: Item::StructInit 等

新規 PRD 着手時は関連 section を事前レビュー。実装が設計判断と乖離していたら該当 section を
最新化 (削除は禁止 — 過去の設計判断は reference として保持)。

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。
skip 解消後は新たな skip 追加を原則禁止とし、回帰検出を自動化する。

**完了済み:**

- Step 0: `basic-types` unskip
- Step 1 (RC-13): `union-fallback`, `ternary`, `ternary-union` unskip + `external-type-struct` (with-builtins) unskip
- Step 2: `array-builtin-methods` unskip + `closures` の I-011 filter 参照セマンティクス解消
- **Pre-Step-3**: I-138 (Vec index Option) + I-022 (`??`) + I-142 (`??=` Ident LHS) — Tier 1 silent bug を pre-Step として解消、`nullish-coalescing` fixture unskip
- **Step 3** (2026-04-17): I-020 部分 + I-025、`void-type` unskip
- **Step 4** (2026-04-17): I-023 + I-021、`async-await` + `discriminated-union` unskip
- **I-153 + I-154 batch** (2026-04-19): switch case body silent redirect + label hygiene structural fix + A-fix (Block stmt support)

**永続 skip (設計制約 4件):**

- `callable-interface-generic-arity-mismatch` — 意図的 error-case (INV-4)
- `indexed-access-type` — マルチファイル用 (`test_multi_file_fixtures_compile` でカバー)
- `vec-method-expected-type` — no-builtins mode 限定の設計制約
- `external-type-struct` — no-builtins mode 限定の設計制約 (with-builtins 側は Step 1 で解消済)

**effective residual (10 fixture):**

trait-coercion, any-type-narrowing, type-narrowing, instanceof-builtin,
intersection-empty-object, closures, functions, keyword-types, string-methods, type-assertion

#### 次の Step

```
~~(L1 silent) I-153 + I-154 batch~~ — ✅ 完了 (2026-04-19)
  ↓
(L2 struct) I-144 (CF narrowing) ← I-142 Step 4 C-1/C-2/C-3/C-4/D-1 吸収
  ↓
Step 5 (type conversion + null)       I-142 Step 4 C-5〜C-9 残余処理 (並行可能)
  ↓ I-158 / I-159 (hygiene follow-ups、並行可能)
Step 6 (string + intersection)        type-narrowing は Step 1 + 6 で完全解消
  ↓
Step 7 (builtin impl)
```

#### Step 5-7 の予定 (未着手)

**Step 5: 型変換 + null セマンティクス** — Tier 2、型変換パイプライン

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-026 | 型 assertion 変換 | `as unknown as T` の中間 `unknown` を消去して直接キャスト |
| I-029 | null/any 変換 | `null as any` → `None` が `Box<dyn Trait>` 文脈で型不一致 |
| I-030 | `build_any_enum_variants()` (`any_narrowing.rs:85`) | any-narrowing enum の値代入で型強制 |

- unskip: `type-assertion`, `trait-coercion`, `any-type-narrowing`

**Step 6: string メソッド + intersection** — Tier 2、独立した小修正群

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-033 | `methods.rs` | `charAt` → `chars().nth()`, `repeat` → `.repeat()` マッピング追加 |
| I-034 | `methods.rs` | `toFixed(n)` → `format!("{:.N}", v)` 変換 |
| I-028 | `intersections.rs:132-145` | mapped type の非 identity 値型で型パラメータ T が消失 (E0091) |

- unskip: `string-methods`, `intersection-empty-object`, `type-narrowing`

**Step 7: ビルトイン型 impl 生成** — Tier 2、大規模

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-071 | `external_struct_generator/` + generator | ビルトイン型（Date, RegExp 等）の impl ブロック生成 |

- unskip: `instanceof-builtin`

#### fixture × Step 解消マトリクス

| fixture | 解消 Step / 依存 | メモ |
|---------|-----------------|------|
| ~~basic-types~~ | ~~Step 0~~ | — |
| ~~union-fallback~~ / ~~ternary~~ / ~~ternary-union~~ | ~~Step 1~~ | — |
| ~~external-type-struct (with-builtins)~~ | ~~Step 1~~ | — |
| ~~array-builtin-methods~~ | ~~Step 2~~ | — |
| ~~void-type~~ | ~~Step 3~~ | — |
| ~~async-await~~ / ~~discriminated-union~~ | ~~Step 4~~ | — |
| ~~nullish-coalescing~~ | ~~pre-Step-3 (I-022 + I-142)~~ | — |
| closures | I-048 (所有権推論) | I-020 Box wrap 解消済、残: move/FnMut |
| keyword-types | I-146 | I-025 implicit None 解消済、残: `return undefined` on void |
| functions | I-319 (Vec index move) | I-020 Box wrap 解消済 |
| type-assertion / trait-coercion / any-type-narrowing | Step 5 | — |
| string-methods / intersection-empty-object | Step 6 | — |
| type-narrowing | Step 6 | Step 1 (I-007) 依存済 |
| instanceof-builtin | Step 7 | — |
| vec-method-expected-type | — | 設計制約 (永続 skip) |
| external-type-struct (no-builtins) | — | 設計制約 (永続 skip) |

### Phase B: RC-11 expected type 伝播 (OBJECT_LITERAL_NO_TYPE 28件)

Phase A 完了後、Hono ベンチマーク最大カテゴリ（全エラーの 45%）に着手。
I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback) を対象とする。
(件数: 2026-04-17 bench 実測 62 errors 中 28 件)

---

## リファレンス

- 最上位原則: [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md)
- 優先度ルール: [`.claude/rules/todo-prioritization.md`](.claude/rules/todo-prioritization.md)
- TODO 記載標準: [`.claude/rules/todo-entry-standards.md`](.claude/rules/todo-entry-standards.md)
- PRD workflow: [`.claude/rules/spec-first-prd.md`](.claude/rules/spec-first-prd.md) + [`.claude/rules/problem-space-analysis.md`](.claude/rules/problem-space-analysis.md)
- 設計整合性: [`.claude/rules/design-integrity.md`](.claude/rules/design-integrity.md) + [`.claude/rules/prd-design-review.md`](.claude/rules/prd-design-review.md)
- **設計判断 archive**: [`doc/handoff/design-decisions.md`](doc/handoff/design-decisions.md)
- PRD handoff: `doc/handoff/*.md` (I-142 Step 4 follow-up 等)
- Grammar reference: `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md`
- TODO 全体: [`TODO`](TODO)
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
- 実装調査 report: `report/*.md`
