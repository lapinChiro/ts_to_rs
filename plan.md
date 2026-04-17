# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-17)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 112/158 (70.9%) |
| Hono bench errors | 62 |
| cargo test (lib) | 2566 pass |
| cargo test (integration) | 122 pass |
| cargo test (compile) | 3 pass (async-await + discriminated-union unskip) |
| cargo test (E2E) | 95 pass |
| clippy | 0 warnings |
| fmt | 0 diffs |

### 直近の完了作業

**Phase A Step 4: I-023 + I-021** (2026-04-17, closed)

`async-await` と `discriminated-union` fixture の skip 解消。2 つの独立した root cause を
structural に解決:

- **I-023 (try/catch unreachable)**: `convert_try_stmt` に `'try_block` labeled block の
  `!`-type 検出を追加。Try body が常に return し throw / outer break / outer continue
  rewrite を伴わない場合、`_try_result` / labeled block / `if let Err` / `unreachable!()`
  の machinery を全て drop し try body を inline emit。`unreachable_code` lint violation
  を構造的に排除。併せて `TryBodyRewrite::rewrite` を `Stmt::Match` / `IfLet` / `WhileLet` /
  `LabeledBlock (non-try_block)` まで exhaustive に recurse させ (Critical review で発見した
  hidden throw silent-drop 問題の根本修正)、`ends_with_return` も `Stmt::Match` (全 arm 終端)
  と `Stmt::IfLet { else_body: Some }` を認識するよう拡張。
- **I-021 (DU field binding)**: `resolve_expr_inner::Tpl` に `tpl.exprs` recursion を追加
  して inner expression の `expr_types` を populate。加えて `du_analysis.rs` の walker を
  **全 AST variant exhaustive** に拡張し、Transformer 側 (`switch.rs`) の重複 walker を
  削除して single source of truth に統合。Array / Object / Unary / Await / OptChain /
  TsAs 系 / Seq / TaggedTpl / New / While / For / Switch / Try / Throw / Labeled まで
  カバーする matrix を lock-in テスト。副次改善として unit variant を
  `Pattern::UnitStruct` で emit (idiomatic な `Status::Active` 出力)。

成果物:
- 16 cell 以上のマトリクス (expression context × DU variant × statement context)
- DU walker に scope-aware shadowing tracking を追加 (I-148 同 PRD 内で解消、Tier 1 silent 排除)
- 34 新規 unit test (du_analysis: 15 + 8 variant coverage + 6 shadowing lock-in + 5 follow-up) + 5 新規 unit test (try/catch noreturn + nested throw regression)
- E2E fixture 拡張 (`tests/e2e/scripts/discriminated_union.ts` に template literal context + shadow 回避再代入、`tests/e2e/scripts/async_await.ts` に try/catch noreturn + nested throw regression)
- `TryBodyRewrite::throw_count: usize` → `has_throw: bool` 変更 (boolean blindness inverse の解消、3 field 対称化)
- PRD: `backlog/phase-a-step-4-du-and-try-catch.md` (完了後 delete、git history に archive)

Follow-up TODO (本 PRD 完了時に新規登録、各項目 empirical trace / 再現 TS / 修正方針まで記載済):

| TODO | 分類 | Tier / 優先度 | 概要 |
|------|------|--------------|------|
| I-149 | review insight / pre-existing latent | L4 | async error propagation PRD (I-049/I-078/I-127) 完了時に I-023 short-circuit 再監査必須 |
| I-150 | review insight / pre-existing | L3 / Tier 2 | `resolve_new_expr` が未登録 class の args を visit せず、DU field access inside `new Error(...)` without builtins で compile error (empirical 再現済) |
| I-151 | review insight / design integrity | L4 | `try_convert_tagged_enum_switch::is_unit_variant` の `unwrap_or(false)` fallback は registry inconsistency に brittle、safer は `return Ok(None)` |
| I-152 | review insight / design integrity | L4 | `pub(crate) mod du_analysis` が pipeline boundary を弱化。walker を `ast_utils` neutral module に移設 or re-export に絞るべき |
| I-153 | review insight / pre-existing Tier 1 | L1 | switch case body の nested bare `break` が outer loop を誤 break する silent (`'switch:` labeled block は Rust 側で bare break target にならないため)、switch emission の pre-rewrite で structural fix すべき。問題空間 matrix 設計が必要 |
| I-154 | review insight / rarity | L4 | `'try_block` 固定 label が user labeled block と衝突し得る hygiene 欠落、`__ts_try_block` 等に変更 |
| I-155 | review insight / defense-in-depth | L4 | `TryBodyRewrite::rewrite` が body-bearing expression (Block/Match/If in Stmt::Expr/TailExpr/Return/Let::init) 内の throw を見ない。現時点 reachability なしだが将来 regression source |
| I-156 | SDCDF 完了条件残 / oracle grounding 欠 | L3 | Phase A Step 4 PRD の "per-cell E2E fixture" 要件が未対応 (既存 fixture 拡張で代替)。16 cell 以上の DU context + 6 shadowing cell について `tests/e2e/scripts/phase-a-step-4/<cell-id>.ts` を作成し runtime stdout 一致 oracle を確立すべき |
| I-157 | review insight / defense-in-depth | L4 | `Stmt::Match` の exhaustiveness が IR 型で表現されず、`ends_with_return` が implicit assumption で判断。`has_wildcard: bool` tag 等の型強化を検討 |

**I-SDCDF: Spec-Driven Conversion Development Framework** (2026-04-17, closed)

PRD の開発プロセスを implementation-first → specification-first に転換する
framework を Phase 1-4 で完走。Pilot (I-050-a) で **Spec gap = 0** を達成し、
rule を Beta 昇格。今後の全 matrix-driven PRD に必須適用。

成果物:
- `.claude/rules/spec-first-prd.md` (Beta) — 2-stage workflow rule
- `doc/grammar/{ast-variants,rust-type-variants,emission-contexts}.md` (Beta) — reference docs
- `scripts/observe-tsc.sh`, `scripts/record-cell-oracle.sh` — helper scripts
- `/prd-template` skill + `/check_job` command — spec-first 統合
- `tests/e2e/scripts/<prd-id>/<cell-id>.ts` — per-cell E2E layout + parametric runner
- 計画詳細: plan.prd.md (v4 承認済、framework 導入完了により git history に archive)

**I-050-a: Any coercion primitive Lit → Value** (2026-04-17, closed)

SDCDF Pilot。primitive Lit (Str/Num/Bool) → `serde_json::Value::from()` coercion を
let-init + return context で実装。6 cell E2E green。Ident coercion は I-050-b に
scope-out (TypeResolver の IR 型乖離問題)。

**I-142: `??=` NullishAssign Ident LHS** (closed、Step 1-3 完了)

shadow-let + fusion + expression-context `get_or_insert_with` による structural rewrite。
残 defect (C-1〜C-9 + D-1) は [引継ぎドキュメント](doc/handoff/I-142-step4-followup.md) に移管。

**Phase A Step 3: Box wrap + implicit None** (2026-04-17, closed)

I-020 (closure tail → `Box::new(...)` wrap) + I-025 (Option return → implicit `None`) を実装。
`void-type` fixture unskip。`closures` は I-048 (所有権推論: move/FnMut) が残存、
`keyword-types` は I-146 (`return undefined` on void fn) が残存。4 cell E2E green。

**I-142-b+c: FieldAccess/Index `??=`** (2026-04-17, closed)

FieldAccess (`obj.field ??= d`) と Index (`cache[key] ??= d`) の `??=` を structural に
実装。FieldAccess は `if is_none / get_or_insert_with` emission、Index は HashMap
`entry().or_insert_with()` emission。TypeResolver に Member ??= 型解決を拡張。
3 cell E2E green。Hono 3 件は TypeResolver の private field / globalThis 解決限界で未解消
(後続調査)。

**以前の完了**: I-022 (`??` 演算子)、I-138 (Vec index Option)、I-040 (optional param 統一)

### 次の作業 (spec-first workflow 適用)

| 優先度 | PRD | 内容 | 根拠 |
|--------|-----|------|------|
| 1 | I-144 | control-flow narrowing analyzer | I-024 complex / D-1 DRY / I-142 Cell #14 を構造的に解消。~800-1000 行、直接 fixture unskip は 0 だが将来の narrowing 基盤 |
| 2 | I-050-b | Ident → Value coercion | TypeResolver 精度向上が前提 |
| 3 | Phase A Step 5 | I-026 / I-029 / I-030 型変換 + null セマンティクス | `type-assertion`, `trait-coercion`, `any-type-narrowing` unskip |
I-142 残 defect (C-1〜C-9 + D-1) は新 framework 適用後に個別 sub-PRD として処理する。

---

## 設計判断の引継ぎ (後続 PRD 向け)

### `push_type_param_scope` は correct design であり interim ではない

PRD 起票時は `push_type_param_scope` を完全削除する想定だったが、実装調査で方針変更:

- `convert_external_type` (外部 JSON ローダ) と `convert_ts_type` (SWC AST コンバータ) は
  独立した 2 つの変換経路。`convert_ts_type` の TypeVar routing を後者が直接流用できない
- `convert_external_type::Named` も scope を参照して TypeVar routing する必要があり、
  scope 自体は「lexical scope management」として残すのが構造的に正しい
- 「interim」だったのは scope を介してフィルタ判定していた `extract_used_type_params` の
  heuristic 部分であり、それは walker-only 実装 (`collect_type_vars`) で完全置換済

**引継ぎ**: scope push を見て「interim 残存では?」と思った場合、上記の判断に立ち戻ること。

### `PrimitiveType` 9 variant の YAGNI 例外

`src/ir/expr.rs::PrimitiveType` は 9 variant 定義で、production で使われるのは `F64` のみ
(`f64::NAN` / `f64::INFINITY`)。「9 variant 維持」を採用した理由: (1) 基盤型としての概念的完全性、
(2) 将来 `i32::MAX` 等で再追加する総コストが現状維持より高い、(3) variant 網羅テストで
dead_code lint 発火しない。

**引継ぎ**: 後続 PRD で primitive associated const を使う際、既存 variant をそのまま利用すべき。

### `switch.rs::is_literal_match_pattern` の意味論微変化

判定基準を `name.contains("::")` 文字列マッチから `Expr::EnumVariant` 構造マッチに変更。
`case Math.PI:` / `case f64::NAN:` のような (TS で稀な) ケースは guarded match に展開される。
Hono 後退ゼロ確認済。

**引継ぎ**: 将来 `case` で primitive const / std const を使う TS fixture を追加する場合、
`is_literal_match_pattern` に `Expr::PrimitiveAssocConst { .. } | Expr::StdConst(_) => true`
追加を検討。ただし `f64` 値の pattern matching は Rust で unstable のため guarded match が安全。

### lock-in テスト (削除禁止)

`tests/enum_value_path_test.rs` / `tests/math_const_test.rs` / `tests/nan_infinity_test.rs`
は `Expr::EnumVariant` / `PrimitiveAssocConst` / `StdConst` 構造化の lock-in テスト。
**削除・スキップ禁止**。

### 残存 broken window

- **`Item::StructInit::name: String`** に display-formatted `"Enum::Variant"` 形式が格納される
  (`transformer/expressions/data_literals.rs:90`)。`StructInit` IR に
  `enum_ty: Option<UserTypeRef>` を追加して構造化すべき（TODO I-074）。

### Step 2 (RC-2) で確立した設計方針

#### 1. remapped methods は TS signature 依存の arg 変換を回避する

`methods::is_remapped_method(name)` を共有判定として持ち、`map_method_call` が書き換える
メソッド（`startsWith`, `endsWith`, `filter`, `find`, `slice`, `substring`, ...）の呼び出しでは:

- 転送器側 (`convert_call_expr`): `method_sig` を `None` にして param_types 由来の
  fill-None / Box::new / trait coercion を抑制
- TypeResolver 側 (`set_call_arg_expected_types`): 末尾 optional 引数を drop してから
  expected type を伝播（required 引数の Fn 型伝播は維持）

これにより、TS の `Array.filter(predicate, thisArg?)` のような signature が Rust の
`Iterator::filter(closure)` に書き換わる際に、`Some(arg)` ラップや末尾 `None` 挿入が
発生しなくなる。

**引継ぎ**: `map_method_call` に新しい remap ケースを追加する際は必ず
`REMAPPED_METHODS` const にも同名を追記する。単体テスト
`test_remapped_methods_match_map_method_call_arms` と
`test_non_remapped_common_methods_passthrough` が両方向の整合性を検証するため、
片方だけ更新するとビルドが失敗する。

#### 2. 構造的 wrap-skip: `produces_option_result`

`convert_expr_with_expected` の `Option<T>` wrap 判定に構造的 fallback を追加。
内部式が `Iterator::find(predicate)` / `Vec::pop()` の method call（Rust 契約で
常に `Option<T>` を by-value 返す）なら TypeResolver が Unknown を返した場合でも
ラップをスキップする。`transpile_collecting` (builtins なし) で
`const doubled = nums.map(...)` の型が unknown になり、`doubled.find(...)` の
返り値型解決が連鎖破綻するケースに対する最小の安全対策。

**引継ぎ**: 将来拡張する場合、`Option<&T>` を返すメソッド（`Vec::first`/`last`/
`get`、`HashMap::get` 等）は追加してはならない（expected `Option<T>` との型整合性が
異なり、silent に wrap-skip するとコンパイルエラーではなく意味論ずれを招く）。
bool 返しや element by-value 返しのメソッドも追加不可。

#### 3. extract-types tool の strictNullChecks 必須化

`tools/extract-types/src/index.ts` の 3 つの program 構築で `strictNullChecks: true`
を固定。非strict では `T | undefined` が `T` に潰される（`Array.find` の `S | undefined`
返り値、`message?: string` の optional param 等）。`isOptional` 判定は
`paramDecl.questionToken` を優先（`param.flags & SymbolFlags.Optional` が callable
signature parameter で false を返すため）。

**引継ぎ**: builtin JSON を再生成する際は必ず strictNullChecks 有効で実行。
`ParamDef.optional = true` かつ `type: T | undefined` は二重ラップ（`Option<Option<T>>`）
を招くため、`extractSignature` で optional 検出時は `stripUndefined` を適用する。

#### 4. FieldAccess receiver の括弧付与

`generator::expressions::needs_parens_as_receiver` に `Expr::Deref` / `Expr::Ref` を
追加。Rust では `.` が `*`/`&` より強く結合するため、`(*x).field` を明示括弧なしに
書くと `*(x.field)` に誤解釈される。

**引継ぎ**: IR で `FieldAccess { object: <prefix op> }` を構築する際は、generator が
括弧を補うことを前提にしてよい（transformer で手動ラップ不要）。

### I-040 で確立した optional param 収束設計

#### 0. Option wrap の原則 (全コードベースで遵守)

`RustType::Option<T>` を新規に生成する際、raw な
`RustType::Option(Box::new(...))` を避け、必ず以下いずれかのヘルパーを使う:

- 条件分岐あり: `.wrap_if_optional(optional)` (optional=false なら passthrough、optional=true なら idempotent wrap)
- 無条件で wrap: `.wrap_optional()` (idempotent — 既に Option なら変更なし)

これによりネスト nullable / 複合 optional セマンティクス (`x?: T | null`, `Partial<T>` 適用済
Option field) における `Option<Option<T>>` silent double-wrap を構造的に防ぐ。

#### 0.5. TypeResolver scope は IR と整合しなければならない (incident-driven)

`extract_param_name_and_type` (関数/arrow の Fn 型登録) と `visit_param_pat`
(本体 scope 登録) は IR 側 (`convert_param` / `wrap_param_with_default`) と同じ
optional ラップ規則を適用する必要がある:

- `x?: T` (optional, no default) → IR: `Option<T>`、TypeResolver: `Option<T>` (両者一致)
- `x: T = value` (default-only) → IR: `Option<T>` (caller 視点)、TypeResolver の
  scope: `T` (本体は `let x = x.unwrap_or(...)` 後に T として参照される)
- `x?: T = value` (両方) → IR: `Option<T>`、TypeResolver の Fn 型: `Option<T>`、
  scope: `T` (default が unwrap)

過去 TypeResolver は optional フラグを完全に無視していたため、本体の `if (x)`
が `if let Some(x) = x` に narrowing されず Rust compile error を生んでいた
(`functions` fixture)。I-040 fix で解消。

#### 1. `RustType::wrap_if_optional` 単一ヘルパー

`src/ir/types.rs` の `RustType::wrap_if_optional(self, optional: bool)` が「TS `?:` optional
→ Rust `Option<T>`」の**唯一の収束点**。新しい param-emission site を追加する際は必ず
本ヘルパー経由で optional を適用すること。直接 `RustType::Option(Box::new(ty))` を書くと
二重ラップ抑止 (`wrap_optional` の idempotency) が働かず、silent semantic bug の risk。

全 10 経路:
1. `convert_method_signature` (interface method) — `interfaces.rs:466`
2. `convert_callable_interface_as_trait` (callable interface) — `interfaces.rs:141`
3. `convert_ident_to_param` (class method / ctor) — `classes/members.rs:453`
4. `convert_fn_type_to_rust` (embedded fn type) — `utilities.rs:127`
5. `try_convert_function_type_alias` (fn type alias) — `type_aliases.rs:370`
6. `resolve_param_def` (registry MethodSignature params) — `typedef.rs:531`
7. `resolve_method_info` (anonymous type literal method) — `intersection.rs:506`
8. `convert_param` (free fn / arrow / fn expr) — `functions/params.rs:28`
9. `convert_external_params` (builtin JSON loader) — `external_types/mod.rs:469`
10. `resolve_ts_type TsTypeInfo::Function` (fn type reachable via TsTypeInfo) — `resolve/mod.rs:76`

#### 2. TsTypeInfo::Function は `Vec<TsParamInfo>` で optional を保持する

`extract_fn_params` は `Vec<TsParamInfo>` 返し。optional flag を下流の `resolve_ts_type` まで
伝播する。過去は `Vec<TsTypeInfo>` で optional が落ちていた (I-040 で修正)。新しく
`TsTypeInfo::Function` を構築するコードは必ず `TsParamInfo { optional }` を含めること。

#### 3. callee の param_types 解決は Ident / Named alias 両対応

`convert_call_expr` の Ident callee path は以下 3 経路で param_types を解決する:

1. `reg().get(&fn_name)` が `TypeDef::Function` → 直接 params 取得 (global fn)
2. `get_expr_type(callee)` が `RustType::Fn { params }` → params を ParamDef に wrap (inline fn type param)
3. `get_expr_type(callee)` が `RustType::Named { name }` で registry の `TypeDef::Function` → params 取得 (fn type alias 経由)

新しい fn 型 variant を追加する際は本 3 経路を参照し、`convert_call_args_inner` の fill-None が働くことを
integration test で確認する。`resolve_call_expr` は callee を `resolve_expr` で visit して
expr_types[callee_span] を populate するため、Ident callee でも `get_expr_type` が機能する。

### Phase A Step 4 で確立した設計方針

#### 1. DU field access walker は single source of truth (`du_analysis::collect_du_field_accesses_from_stmts`)

`src/pipeline/type_resolver/du_analysis.rs` の `collect_du_field_accesses_from_stmts` が
switch 内 `obj.field` 形式のアクセス収集の唯一の entry point。TypeResolver (`detect_du_switch_bindings`
での `DuFieldBinding` 登録) と Transformer (`switch.rs::try_convert_tagged_enum_switch` の
`needed_fields` 計算) の両方が同一関数を call する。`doc/grammar/ast-variants.md` の Tier 1 Expr /
Stmt 全 variant を exhaustive に match し、Arrow/Fn body のみ I-048 scope-out として意図的にスキップ
(追加時は variant 網羅を保つこと)。

**引継ぎ**: 新規 AST variant 追加時は本 walker にも arm 追加必須 (walker が exhaustive match のため
build fail でリマインダーが出る)。同 walker の scope-aware shadowing tracking (`walk_stmts` +
`stmt_declares_name` + `pat_binds_name` + `for_head_binds_name`) は `obj_var` 同名の binding 導入で
descendant 収集を抑止する構造。新しい binding 導入 construct (TS 仕様拡張) が増えたら本 tracking にも
反映必須。

#### 2. `resolve_expr_inner::Tpl` / `TaggedTpl` は children を必ず visit する

`src/pipeline/type_resolver/expressions.rs` の `Tpl` arm は `tpl.exprs` を全て `resolve_expr`
で visit して inner expression の `expr_types` entry を populate する。これにより downstream
(`is_du_field_binding` check 等) が inner の Ident 型を lookup 可能になる。`TaggedTpl` も同様に
tag + tpl.exprs を visit (本体の return 型は Unknown)。

**引継ぎ**: Expression で body-bearing な variant (Block / Match / If / IfLet) を新規追加する際は、
children visit の完全性を verify する (span-based lookup が silent に fail しないため)。

#### 3. `TryBodyRewrite::rewrite` は break-to-try_block の source を全て exhaustive に capture する

`src/transformer/statements/error_handling.rs` の try body rewriter は 3 種類の break source を capture:
(a) `Stmt::Return(Some(Err(...)))` → throw rewrite (`has_throw` flag 立て + `_try_result = Err; break 'try_block`)、
(b) bare `Stmt::Break { None }` at loop_depth == 0 → `needs_break_flag` + flag 立て、
(c) bare `Stmt::Continue { None }` at loop_depth == 0 → `needs_continue_flag` + flag 立て。

`Stmt::If` / `ForIn` / `While` / `Loop` / `IfLet` / `WhileLet` / `Match (arm bodies)` /
`LabeledBlock (label != "try_block")` 全てに再帰し、hidden throw/break が skip されないようにする
(Phase A Step 4 deep /check_job で検出した Critical bug の根本 fix)。

**引継ぎ**: IR `Stmt` に body-bearing variant を追加する場合は `TryBodyRewrite::rewrite` の recurse
先を必ず更新。また `ends_with_return` も対応 variant を認識させる (現状は Return / If(both) / IfLet(both) /
Match(all arms))。

#### 4. I-023 short-circuit は labeled block が `!`-typed と判定できる時のみ発動

`convert_try_stmt` 内の `if try_ends_with_return && !has_break_to_try_block` 条件は、**labeled
block が Rust 型 `!` になる場合のみ** machinery (`_try_result`/`LabeledBlock`/`if let Err`/
`unreachable!()`) を全て drop する。throw があれば block 型は `()` になるため machinery 必要、
I-023 short-circuit は抑止される (has_throw が true のため)。

**引継ぎ**: async error propagation PRD (I-049/I-078/I-127 系) が導入されたら、async 文脈の
catch body drop は semantic 失われるため I-149 に従い再設計必須。

### union return wrapping の実行順序 (RC-13 PRD で確立)

`convert_fn_decl` 内の処理順序は以下でなければならない:

1. **Union return wrapping** — return/tail 式を enum variant constructor でラップ
2. **has_throw wrapping** — return 式を `Ok()` でラップし、return_type を `Result` に変更
3. **`convert_last_return_to_tail`** — 最後の return を tail 式に変換

理由: (1) `wrap_returns_in_ok` は `Stmt::TailExpr` を処理しないため 3 の後に実行不可。
(2) has_throw が return_type を `Result<T, String>` に変更すると union 型 `T` が隠蔽され
union wrap 判定が失敗するため 2 の前に実行必須。(3) throw 由来の `Err(...)` return は
SWC leaf collection に対応がないため `wrap_body_returns` でスキップする。

---

## 次のタスク

「現在の状態」セクションの「次の作業」テーブル参照。

### I-142 の依存 / lock-in 状態

```
I-142 Cell 分類
  ├── #1〜#4, #7, #8, #11, #13  — Step 1 で structural 解消
  ├── #6, #10, #12              — Step 2 で structural 解消
  ├── #5, #9                    — I-050 依存、compile-error lock-in test
  ├── #14 (narrowing-reset)     — I-144 依存、lock-in test
  └── FieldAccess / Index       — I-142-b+c で解消済 (2026-04-17)
```

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。
skip 解消後は新たな skip 追加を原則禁止とし、回帰検出を自動化する。

**完了済み:**
- Step 0: `basic-types` unskip
- Step 1 (RC-13): `union-fallback`, `ternary`, `ternary-union` unskip + `external-type-struct` with-builtins unskip
- Step 2: `array-builtin-methods` unskip + `closures` の I-011 filter 参照セマンティクス解消
- **I-138 (pre-Step-3)**: Vec index read access の Option<T> context 対応 (Tier 1 silent bug 解消)
- **I-022 (pre-Step-3)**: `??` 演算子 LHS 型処理 + chain case 対応 (Tier 1 silent drop 解消 + chain compile error 解消)
- **I-142 (pre-Step-3)**: `??=` (NullishAssign) Ident LHS の structural 解消 — shadow-let + fusion + expression-context `get_or_insert_with(*/clone)` + matrix-driven cells。`nullish-coalescing` fixture skip 除去 (no-builtins + with-builtins)。Step 3 で敵対的自己レビュー (D-1 narrowing-reset 検出 / D-2 RHS matrix 4-class 正規化 / D-3 RHS convert 局所化 / D-4 exhaustive `pick_strategy` + table test / D-5〜D-7 cosmetic) まで完了

**永続 skip (2件):** `callable-interface-generic-arity-mismatch` (意図的 error-case), `indexed-access-type` (マルチファイル用、別テストでカバー)

**残: 12 fixture** (effective 10 + 設計制約 2; + I-144 起票済)

#### 次の Step

```
I-144 (CF narrowing)
  ↓
Step 5 (type conversion + null)      Step 6 (string + intersection)
                                     type-narrowing は Step 1 + 6 で完全解消
  ↓
Step 7 (builtin impl)
```

**Step 3: クロージャ Box 化 + Option 暗黙返却** — **完了** (2026-04-17)

| イシュー | 状態 | 内容 |
|----------|------|------|
| I-020 | **部分解消** | return/tail の closure → `Box::new(...)` wrap (`wrap_closures_in_box` 再帰 walk)。残: let-init 経路 + Option<Fn> inner (I-147) |
| I-025 | **解消** | `append_implicit_none_if_needed`: if/while/for 末尾に implicit `None` |
| I-024 | **基本動作確認済** | `if (x)` truthy narrowing は既に動作。complex case は I-144 |

- unskip: `void-type`
- `closures` は I-048 (所有権推論: move/FnMut) が残存、skip 維持
- `keyword-types` は I-146 (`return undefined` on void fn) が残存、skip 維持
- `functions` は I-319 (Vec index move) が残存、skip 維持

---

**Step 4: 制御フロー + DU** — **完了** (2026-04-17)

| イシュー | 状態 | 内容 |
|----------|------|------|
| I-023 | **解消** | `convert_try_stmt` に `!`-typed labeled block 検出を追加し、try body が常時 return + throw/break/continue なしのケースで machinery を drop して body を inline emit |
| I-021 | **解消** | `resolve_expr_inner::Tpl` に recursion 追加 + DU field access walker を統合して全 AST variant exhaustive に拡張 + unit variant pattern を `Pattern::UnitStruct` 化 |

- unskip: `async-await`, `discriminated-union`
- `functions` は I-319 (Vec index move) が残存、skip 維持

---

**Step 5: 型変換 + null セマンティクス** — Tier 2、型変換パイプライン

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| ~~I-022~~ | ~~解消済~~ | ~~`??` 演算子 LHS Option 処理 + chain case (pre-Step-3 で完了)~~ |
| ~~I-142~~ | ~~解消済~~ | ~~`??=` Ident LHS shadow-let rewrite (pre-Step-3 で完了)~~ |
| ~~I-142-b+c~~ | ~~解消済~~ | ~~FieldAccess/Index LHS `??=` (2026-04-17 完了)~~ |
| I-026 | 型 assertion 変換 | `as unknown as T` の中間 `unknown` を消去して直接キャスト |
| I-029 | null/any 変換 | `null as any` → `None` が `Box<dyn Trait>` 文脈で型不一致 |
| I-030 | `build_any_enum_variants()` (`any_narrowing.rs:85`) | any-narrowing enum の値代入で型強制 |

- unskip: ~~`nullish-coalescing` (pre-Step-3 I-022+I-142 で解消済)~~, `type-assertion`, `trait-coercion`, `any-type-narrowing`

---

**Step 6: string メソッド + intersection** — Tier 2、独立した小修正群

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-033 | `methods.rs` | `charAt` → `chars().nth()`, `repeat` → `.repeat()` マッピング追加 |
| I-034 | `methods.rs` | `toFixed(n)` → `format!("{:.N}", v)` 変換 |
| I-028 | `intersections.rs:132-145` | mapped type の非 identity 値型で型パラメータ T が消失 (E0091) |

- unskip: `string-methods`, `intersection-empty-object`
- `type-narrowing` 完全解消（Step 1 の I-007 と合わせて）

---

**Step 7: ビルトイン型 impl 生成** — Tier 2、大規模

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-071 | `external_struct_generator/` + generator | ビルトイン型（Date, RegExp 等）の impl ブロック生成 |

- unskip: `instanceof-builtin`（`String()` コンストラクタ呼び出し問題が別途残る可能性あり）
- `external-type-struct` の no-builtin skip はテスト設計上の制約（with-builtin は Step 1 で解消済み）

---

#### fixture × Step 解消マトリクス

| fixture | 解消 Step | 依存 |
|---------|-----------|------|
| ~~basic-types~~ | ~~Step 0~~ | — |
| ~~union-fallback~~ | ~~Step 1~~ | — |
| ~~ternary~~ | ~~Step 1~~ | — |
| ~~ternary-union~~ | ~~Step 1~~ | — |
| ~~external-type-struct (with-builtins)~~ | ~~Step 1~~ | — |
| ~~array-builtin-methods~~ | ~~Step 2~~ | — |
| closures | I-048 (所有権推論) | I-020 Box wrap 解消済、残: move/FnMut |
| keyword-types | I-146 | I-025 implicit None 解消済、残: `return undefined` on void |
| ~~void-type~~ | ~~Step 3~~ | — |
| functions | I-319 (Vec index move) | I-020 Box wrap 解消済 |
| ~~async-await~~ | ~~Step 4~~ | — |
| ~~discriminated-union~~ | ~~Step 4~~ | — |
| ~~nullish-coalescing~~ | ~~pre-Step-3 (I-022 + I-142)~~ | — |
| type-assertion | Step 5 | — |
| trait-coercion | Step 5 | — |
| any-type-narrowing | Step 5 | — |
| string-methods | Step 6 | — |
| intersection-empty-object | Step 6 | — |
| type-narrowing | Step 6 | Step 1 (I-007) |
| instanceof-builtin | Step 7 | — |
| vec-method-expected-type | — | builtins なし mode で expected 未伝播 (設計制約) |
| external-type-struct (no-builtins) | — | builtins 必要 (設計制約、with-builtins は Step 1 で解消済) |

### Phase B: RC-11 expected type 伝播 (OBJECT_LITERAL_NO_TYPE 27件)

Phase A 完了後、Hono ベンチマーク最大カテゴリ（全エラーの 47%）に着手。
I-004 (imported 関数), I-005 (匿名構造体), I-006 (.map callback) を対象とする。

---

## リファレンス

- 最上位原則: `.claude/rules/ideal-implementation-primacy.md`
- 優先度ルール: `.claude/rules/todo-prioritization.md`
- TODO 記載標準: `.claude/rules/todo-entry-standards.md`
- TODO 全体: `TODO`
- ベンチマーク履歴: `bench-history.jsonl`
- エラー分析: `scripts/inspect-errors.py`
