# 設計判断の引継ぎ

後続 PRD 向けの設計判断アーカイブ。過去 PRD で確立した convention / idiom / 既知
broken window を「将来の実装者が参照すべき reference」として保持する。

各 section は発見時点の背景と現時点 (記載時点) の状態を記録。参照時は git log で
最新化の有無を確認すること。

## 参照ルール / スキル

- `.claude/rules/ideal-implementation-primacy.md` — 最上位原則
- `.claude/rules/design-integrity.md` — 設計整合性チェックリスト
- `.claude/rules/prd-design-review.md` — PRD 設計レビュー手順
- `.claude/rules/problem-space-analysis.md` — 問題空間網羅化
- `.claude/rules/spec-first-prd.md` — SDCDF 2-stage workflow
- `.claude/rules/pipeline-integrity.md` — pipeline 境界保持
- `.claude/rules/type-fallback-safety.md` — 型 fallback 安全性分析

## 目次

1. [Type scope 管理](#type-scope-管理)
2. [Primitive type 9 variant YAGNI 例外](#primitive-type-9-variant-yagni-例外)
3. [Switch emission と label hygiene (I-153/I-154)](#switch-emission-と-label-hygiene-i-153i-154)
4. [Optional param 収束設計 (I-040)](#optional-param-収束設計-i-040)
5. [Conversion helpers (RC-2)](#conversion-helpers-rc-2)
6. [Error handling emission](#error-handling-emission)
7. [DU analysis (Phase A Step 4)](#du-analysis-phase-a-step-4)
8. [Control-flow narrowing analyzer (I-144)](#control-flow-narrowing-analyzer-i-144)
9. [Lock-in テスト (削除禁止)](#lock-in-テスト-削除禁止)
10. [残存 broken window](#残存-broken-window)

---

## Type scope 管理

### `push_type_param_scope` は correct design であり interim ではない

PRD 起票時は `push_type_param_scope` を完全削除する想定だったが、実装調査で方針変更:

- `convert_external_type` (外部 JSON ローダ) と `convert_ts_type` (SWC AST コンバータ) は
  独立した 2 つの変換経路。`convert_ts_type` の TypeVar routing を後者が直接流用できない
- `convert_external_type::Named` も scope を参照して TypeVar routing する必要があり、
  scope 自体は「lexical scope management」として残すのが構造的に正しい
- 「interim」だったのは scope を介してフィルタ判定していた `extract_used_type_params` の
  heuristic 部分であり、それは walker-only 実装 (`collect_type_vars`) で完全置換済

**引継ぎ**: scope push を見て「interim 残存では?」と思った場合、上記の判断に立ち戻ること。

---

## Primitive type 9 variant YAGNI 例外

`src/ir/expr.rs::PrimitiveType` は 9 variant 定義で、production で使われるのは `F64` のみ
(`f64::NAN` / `f64::INFINITY`)。「9 variant 維持」を採用した理由: (1) 基盤型としての概念的完全性、
(2) 将来 `i32::MAX` 等で再追加する総コストが現状維持より高い、(3) variant 網羅テストで
dead_code lint 発火しない。

**引継ぎ**: 後続 PRD で primitive associated const を使う際、既存 variant をそのまま利用すべき。

---

## Switch emission と label hygiene (I-153/I-154)

### 1. `__ts_` prefix namespace reservation

ts_to_rs が emission する全 internal label は `__ts_` prefix で統一:

| Label | 位置 | 用途 |
|-------|------|------|
| `'__ts_switch` | `src/transformer/statements/switch.rs` | switch case body 内 nested break の target (conditional wrap 発動時のみ emit) |
| `'__ts_try_block` | `src/transformer/statements/error_handling.rs:125` | try body の throw / break / continue rewrite 先 |
| `'__ts_do_while` | `src/transformer/statements/loops.rs:360/382` | do-while body 内 continue の rewrite 先 (needs_labeled_block 発動時) |
| `'__ts_do_while_loop` | `src/transformer/statements/loops.rs:346` | do-while の outer Loop label fallback (user label なしの時) |

User の `__ts_*` prefix 使用は 3 entry point で lint reject (`check_ts_internal_label_namespace`
@ `src/transformer/statements/mod.rs`):
- `convert_stmt::ast::Stmt::Labeled` (宣言)
- `convert_stmt::ast::Stmt::Break` (labeled break 参照)
- `convert_stmt::ast::Stmt::Continue` (labeled continue 参照)
- defense-in-depth: `convert_labeled_stmt` (loops.rs) 内にも同じ check

**引継ぎ**: 新規 internal label 追加時は必ず `__ts_` prefix を使用する。user label との
collision は lint で構造的に block される。SWC parser が未定義 label への break を accept
する挙動 (tsx は reject) にも対応済。変数名 hygiene は別 concern (I-159)。

### 2. `rewrite_nested_bare_break_in_stmts` walker 設計パターン

`src/transformer/statements/switch.rs` の walker は **14 IR Stmt variant を exhaustive match**
で 4 カテゴリに分類:

- **Descent** (same-switch scope): `If.{then,else}_body` / `IfLet.{then,else}_body` / `Match.arms[*].body`
- **Skip** (inner emission 所掌尊重): `Stmt::LabeledBlock { .. }` (全 label 無条件)
- **Non-descent** (loop 境界、inner break は inner loop target): `While / WhileLet / ForIn / Loop`
- **Leaf**: `Let / Break (labeled or value-bearing) / Continue / Return / Expr / TailExpr`

**引継ぎ**:
- 新規 IR `Stmt` variant 追加時、walker に arm 追加必須 (exhaustive match で build fail)
- `Match.arms[*].body` descent は **explicit loop** (no `.any()`)、short-circuit 禁止 —
  複数 arm すべてに rewrite 適用要
- `Stmt::LabeledBlock` の skip は内部 label (`__ts_*`) のみでなく user label (I-158 後の
  `Stmt::LabeledBlock { label: user_L, body }` 想定) にも適用 — inner scope の break
  ownership を尊重する design
- **future regression 警告**: `Stmt::Break` が Expr (`Expr::Block/If/IfLet/Match`) 内に
  埋め込まれる emission が追加された場合、walker の拡張が必要 (I-160 参照)

### 3. Conditional LabeledBlock wrap (unused_labels warning 回避)

5 switch emission path のうち fallthrough path 以外の 4 path は **rewrite が発生した時のみ**
`'__ts_switch:` labeled block で wrap:

```rust
fn wrap_match_with_switch_label_if_needed(arms, match_expr) -> Vec<Stmt> {
    let rewritten = /* walk arms */;
    if rewritten { LabeledBlock wrap } else { raw Match }
}
```

Conditional 判定により Rust の `unused_labels` warning を回避。fallthrough path は既存の
`'__ts_switch:` emission を維持。

**引継ぎ**: 新規 switch emission path を追加する場合は `wrap_match_with_switch_label_if_needed`
経由で一貫性維持。

### 4. `ast::Stmt::Block` flatten による lexical scope 等価保持 (A-fix)

`src/transformer/statements/mod.rs::convert_stmt` に `ast::Stmt::Block(block) =>
convert_stmt_list(&block.stmts, return_type)` arm を追加。TS の `{ ... }` block stmt は
親 scope に flatten される。Rust 側では親 context (match arm / fn body / if body 等) が
既に `{ }` block scope を提供するため、valid TS の範囲で semantic 等価。

**注意**: `is_case_terminated` (switch.rs) に `ast::Stmt::Block` peek-through 追加必須 —
`case 1: { return 1; }` が case-terminated 判定されないと fallthrough path 誤選択。

**引継ぎ**: TS の block scope に `const/let` を跨いで参照する ill-formed コード (tsc error)
は本 flatten 下で silent compile 成功する可能性あり。valid TS 前提、invalid TS は scope 外。

### 5. `switch.rs::is_literal_match_pattern` の意味論微変化

判定基準を `name.contains("::")` 文字列マッチから `Expr::EnumVariant` 構造マッチに変更。
`case Math.PI:` / `case f64::NAN:` のような (TS で稀な) ケースは guarded match に展開される。
Hono 後退ゼロ確認済。

**引継ぎ**: 将来 `case` で primitive const / std const を使う TS fixture を追加する場合、
`is_literal_match_pattern` に `Expr::PrimitiveAssocConst { .. } | Expr::StdConst(_) => true`
追加を検討。ただし `f64` 値の pattern matching は Rust で unstable のため guarded match が安全。

---

## Optional param 収束設計 (I-040)

### 0. Option wrap の原則 (全コードベースで遵守)

`RustType::Option<T>` を新規に生成する際、raw な
`RustType::Option(Box::new(...))` を避け、必ず以下いずれかのヘルパーを使う:

- 条件分岐あり: `.wrap_if_optional(optional)` (optional=false なら passthrough、optional=true なら idempotent wrap)
- 無条件で wrap: `.wrap_optional()` (idempotent — 既に Option なら変更なし)

これによりネスト nullable / 複合 optional セマンティクス (`x?: T | null`, `Partial<T>` 適用済
Option field) における `Option<Option<T>>` silent double-wrap を構造的に防ぐ。

### 0.5. TypeResolver scope は IR と整合しなければならない (incident-driven)

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

### 1. `RustType::wrap_if_optional` 単一ヘルパー

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

### 2. TsTypeInfo::Function は `Vec<TsParamInfo>` で optional を保持する

`extract_fn_params` は `Vec<TsParamInfo>` 返し。optional flag を下流の `resolve_ts_type` まで
伝播する。過去は `Vec<TsTypeInfo>` で optional が落ちていた (I-040 で修正)。新しく
`TsTypeInfo::Function` を構築するコードは必ず `TsParamInfo { optional }` を含めること。

### 3. callee の param_types 解決は Ident / Named alias 両対応

`convert_call_expr` の Ident callee path は以下 3 経路で param_types を解決する:

1. `reg().get(&fn_name)` が `TypeDef::Function` → 直接 params 取得 (global fn)
2. `get_expr_type(callee)` が `RustType::Fn { params }` → params を ParamDef に wrap (inline fn type param)
3. `get_expr_type(callee)` が `RustType::Named { name }` で registry の `TypeDef::Function` → params 取得 (fn type alias 経由)

新しい fn 型 variant を追加する際は本 3 経路を参照し、`convert_call_args_inner` の fill-None が働くことを
integration test で確認する。`resolve_call_expr` は callee を `resolve_expr` で visit して
expr_types[callee_span] を populate するため、Ident callee でも `get_expr_type` が機能する。

---

## Conversion helpers (RC-2)

### 1. remapped methods は TS signature 依存の arg 変換を回避する

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

### 2. 構造的 wrap-skip: `produces_option_result`

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

### 3. extract-types tool の strictNullChecks 必須化

`tools/extract-types/src/index.ts` の 3 つの program 構築で `strictNullChecks: true`
を固定。非strict では `T | undefined` が `T` に潰される（`Array.find` の `S | undefined`
返り値、`message?: string` の optional param 等）。`isOptional` 判定は
`paramDecl.questionToken` を優先（`param.flags & SymbolFlags.Optional` が callable
signature parameter で false を返すため）。

**引継ぎ**: builtin JSON を再生成する際は必ず strictNullChecks 有効で実行。
`ParamDef.optional = true` かつ `type: T | undefined` は二重ラップ（`Option<Option<T>>`）
を招くため、`extractSignature` で optional 検出時は `stripUndefined` を適用する。

### 4. FieldAccess receiver の括弧付与

`generator::expressions::needs_parens_as_receiver` に `Expr::Deref` / `Expr::Ref` を
追加。Rust では `.` が `*`/`&` より強く結合するため、`(*x).field` を明示括弧なしに
書くと `*(x.field)` に誤解釈される。

**引継ぎ**: IR で `FieldAccess { object: <prefix op> }` を構築する際は、generator が
括弧を補うことを前提にしてよい（transformer で手動ラップ不要）。

---

## Error handling emission

### 1. `TryBodyRewrite::rewrite` は break-to-try_block の source を exhaustive に capture する

`src/transformer/statements/error_handling.rs` の try body rewriter は 3 種類の break source を capture:

(a) `Stmt::Return(Some(Err(...)))` → throw rewrite (`has_throw` flag 立て + `_try_result = Err; break '__ts_try_block`)
(b) bare `Stmt::Break { None }` at loop_depth == 0 → `needs_break_flag` + flag 立て
(c) bare `Stmt::Continue { None }` at loop_depth == 0 → `needs_continue_flag` + flag 立て

`Stmt::If` / `ForIn` / `While` / `Loop` / `IfLet` / `WhileLet` / `Match (arm bodies)` /
`LabeledBlock (label != "__ts_try_block")` 全てに再帰し、hidden throw/break が skip されない
ようにする (Phase A Step 4 deep /check_job で検出した Critical bug の根本 fix)。

**引継ぎ**: IR `Stmt` に body-bearing variant を追加する場合は `TryBodyRewrite::rewrite` の
recurse 先を必ず更新。また `ends_with_return` も対応 variant を認識させる (現状は Return /
If(both) / IfLet(both) / Match(all arms))。

### 2. I-023 short-circuit は labeled block が `!`-typed と判定できる時のみ発動

`convert_try_stmt` 内の `if try_ends_with_return && !has_break_to_try_block` 条件は、**labeled
block が Rust 型 `!` になる場合のみ** machinery (`_try_result`/`LabeledBlock`/`if let Err`/
`unreachable!()`) を全て drop する。throw があれば block 型は `()` になるため machinery 必要、
I-023 short-circuit は抑止される (has_throw が true のため)。

**引継ぎ**: async error propagation PRD (I-049/I-078/I-127 系) が導入されたら、async 文脈の
catch body drop は semantic 失われるため I-149 に従い再設計必須。

### 3. TryBodyRewrite と I-153 walker の cooperation

TryBodyRewrite は try body 内の bare break を `_try_break = true; break '__ts_try_block` に
rewrite し、labeled block 脱出後に `if _try_break { break; }` を emit する。この emit される
bare break は **case body sibling scope** にあり、I-153 walker の descent 対象
(`Stmt::If.then_body`)。walker が自動的に `break '__ts_switch` に rewrite し、try body からの
switch break を正しく伝播。

`label == "__ts_try_block"` self-skip (error_handling.rs:436) は TryBodyRewrite が自身の
nested try labeled block を re-rewrite しないための guard。I-154 rename 時に更新済。

**引継ぎ**: TryBodyRewrite の label 名を更新する際は error_handling.rs:436 の self-skip
check も同期更新。

### 4. union return wrapping の実行順序 (RC-13)

`convert_fn_decl` 内の処理順序は以下でなければならない:

1. **Union return wrapping** — return/tail 式を enum variant constructor でラップ
2. **has_throw wrapping** — return 式を `Ok()` でラップし、return_type を `Result` に変更
3. **`convert_last_return_to_tail`** — 最後の return を tail 式に変換

理由:

- `wrap_returns_in_ok` は `Stmt::TailExpr` を処理しないため 3 の後に実行不可
- has_throw が return_type を `Result<T, String>` に変更すると union 型 `T` が隠蔽され
  union wrap 判定が失敗するため 2 の前に実行必須
- throw 由来の `Err(...)` return は SWC leaf collection に対応がないため `wrap_body_returns`
  でスキップする

---

## DU analysis (Phase A Step 4)

### 1. DU field access walker は single source of truth

`src/pipeline/type_resolver/du_analysis.rs` の `collect_du_field_accesses_from_stmts` が
switch 内 `obj.field` 形式のアクセス収集の唯一の entry point。TypeResolver
(`detect_du_switch_bindings` での `DuFieldBinding` 登録) と Transformer
(`switch.rs::try_convert_tagged_enum_switch` の `needed_fields` 計算) の両方が同一関数を call
する。`doc/grammar/ast-variants.md` の Tier 1 Expr / Stmt 全 variant を exhaustive に match し、
Arrow/Fn body のみ I-048 scope-out として意図的にスキップ (追加時は variant 網羅を保つこと)。

**引継ぎ**: 新規 AST variant 追加時は本 walker にも arm 追加必須 (walker が exhaustive match の
ため build fail でリマインダーが出る)。同 walker の scope-aware shadowing tracking
(`walk_stmts` + `stmt_declares_name` + `pat_binds_name` + `for_head_binds_name`) は `obj_var`
同名の binding 導入で descendant 収集を抑止する構造。新しい binding 導入 construct (TS 仕様拡張)
が増えたら本 tracking にも反映必須。

### 2. `resolve_expr_inner::Tpl` / `TaggedTpl` は children を必ず visit する

`src/pipeline/type_resolver/expressions.rs` の `Tpl` arm は `tpl.exprs` を全て `resolve_expr`
で visit して inner expression の `expr_types` entry を populate する。これにより downstream
(`is_du_field_binding` check 等) が inner の Ident 型を lookup 可能になる。`TaggedTpl` も同様に
tag + tpl.exprs を visit (本体の return 型は Unknown)。

**引継ぎ**: Expression で body-bearing な variant (Block / Match / If / IfLet) を新規追加する際は、
children visit の完全性を verify する (span-based lookup が silent に fail しないため)。

---

## Lock-in テスト (削除禁止)

以下のテストは特定の構造化 IR / emission の lock-in として機能する。削除・スキップ禁止。

- `tests/enum_value_path_test.rs` — `Expr::EnumVariant` 構造化の lock-in
- `tests/math_const_test.rs` — `PrimitiveAssocConst` 構造化の lock-in
- `tests/nan_infinity_test.rs` — `StdConst` 構造化の lock-in

I-153 + I-154 batch で追加された lock-in (いずれも削除・スキップ禁止):

- `src/transformer/statements/tests/switch.rs::i153_walker_tests::*` — walker の 14 IR Stmt variant exhaustive descent/skip/non-descent/leaf policy を 19 case で lock
- `src/transformer/statements/tests/loops.rs::i154_*` — `__ts_` prefix label lint を 3 entry point で lock
- `src/transformer/statements/tests/control_flow.rs::test_convert_block_stmt_*` — A-fix (Block flatten) と case body 内 nested break 保存を lock
- `tests/e2e/scripts/i153/*.ts` — 13 per-cell runtime oracle
- `tests/e2e/scripts/i154/*.ts` — 3 user label hygiene oracle

---

## Control-flow narrowing analyzer (I-144)

I-144 (2026-04-19 起票、2026-04-21 完了) は TypeScript の flow-sensitive type narrowing を
Rust に写像する CFG-based analyzer を確立。以下は後続 PRD が narrow 領域に触れる際に参照
すべき設計判断 archive。

### 1. 2 channel architecture: `NarrowEvent` vs `EmissionHint` vs `du_analysis`

Problem Space T 次元の narrow trigger を **3 つの architecture に意図的に分離**:

| Trigger dimension | Architecture | 出力 |
|-------------------|--------------|------|
| T1 typeof / T2 instanceof / T3 null check / T4 truthy / T7 OptChain / T9 negation / T11 early-return | `NarrowEvent::Narrow` via `pipeline/narrowing_analyzer/guards.rs` | TypeResolver scope-based override |
| T6 `??=` narrow emission | `EmissionHint::{ShadowLet, GetOrInsertWith}` via `pipeline/narrowing_analyzer/classifier.rs` | Transformer per-stmt dispatch |
| T8 DU switch case | `pipeline/type_resolver/du_analysis.rs` | Tag-based match pattern (既存、I-144 で migrate せず) |

**根拠**: narrow 検出メカニズム (guard → scope) と、emission 選択 (stmt-specific hint) と、
DU tag-based pattern (match 生成) は本質的に異なる責務。単一 channel に統合すると
pipeline-integrity が崩れる (FileTypeResolution は immutable data のみ保持すべきで、
emission 選択ロジックは Transformer 所有)。

**帰結** (IMPL-1〜4 fix、T6-6 2026-04-21): `NarrowEvent` 関連 enum / struct は
actually-populated な variant / field のみ保持する **YAGNI 厳守** 方針を確定:

- **`PrimaryTrigger`** 5 variants: TypeofGuard / InstanceofGuard / NullCheck /
  Truthy / OptChainInvariant (T6 `??=` narrowing / T8 DU switch は別 architecture
  で処理されるため enum variant なし、dead 化を防止)
- **`EmissionHint`** 2 variants: ShadowLet / GetOrInsertWith (`??=` 2-way dispatch
  のみ; E2b/E3/E4/E5/E6/E7/E8/E9/E10 は Transformer 側 helpers / pattern visitors で
  直接 emit され、analyzer hint を経由しない)
- **`RcContext`** enum **削除** (T6-2 当初は Sub-matrix 5 RC 次元のコード化として
  導入したが production では ExpectT 単一値のみ使用。enum parameter 自体が
  over-designed で YAGNI 違反、`coerce_default.rs` の API を RC 別 dedicated builder
  (`build_option_coerce_to_t` RC1 + `build_option_coerce_to_string` RC6) に整理)
- **`NarrowEvent::ClosureCapture.outer_narrow: RustType`** field **削除** (Phase 3b
  emission policy 予約だったが Transformer 消費 0、現在の narrow suppression path は
  enclosing_fn_body span filter のみで十分)

Problem Space dimension T / E / RC は PRD matrix として記録・完全性確認するが、
「dimension の全 cell を enum variant として定義する」ことは **YAGNI に反する**。
対応 enum は actually-dispatched な cell のみ実装し、未実装 cell は TODO / PRD で
track する。将来 architecture が拡張される時点で variant / field を追加する。

### 2. `NarrowTypeContext` trait による registry access 抽象化

`narrowing_analyzer/type_context.rs` に `NarrowTypeContext` trait を新設。
`lookup_var` / `synthetic_enum_variants` / `register_sub_union` / `push_narrow_event`
の 4 method で TypeResolver 実装を抽象化し、trait boundary 専用 unit test を実装。

**根拠**: T5 で `type_resolver/narrowing.rs` (524 行) を削除し `narrowing_analyzer/guards.rs` に
集約する際、narrow guard 検出と registry access の結合度を下げるため trait 境界で分離。
MockNarrowTypeContext 経由で registry-less に narrow 検出ロジックを unit test 可能にした。

### 3. `NarrowEvent` 3-variant enum + 2-layer `NarrowTrigger`

`NarrowEvent::{Narrow, Reset, ClosureCapture}` + `NarrowTrigger::{Primary(PrimaryTrigger),
EarlyReturnComplement(PrimaryTrigger)}` の 2-layer 型で nested `EarlyReturnComplement` を
**型レベルで構造排除**。

**根拠**: T4 migration 時に `NarrowTrigger::EarlyReturnComplement(NarrowTrigger)` 形式を
採用すると、`EarlyReturnComplement(EarlyReturnComplement(Truthy))` 等の不正 nested を
型システムが許容してしまう。`PrimaryTrigger` を wrap する単層にすることで single-level
complement という TS semantics を型レベルで強制。

### 4. `coerce_default` table: narrow-stale emission

`transformer/helpers/coerce_default.rs` に JS coerce_default table を実装。
narrow 変数が closure reassign 等で stale 化した時、RC1 arithmetic (`x + 1`) と
RC6 string concat (`"v=" + x`) で runtime null を Rust で再現する:

| RC | Inner type | null coerce | Rust emission |
|----|-----------|-------------|---------------|
| RC1 arithmetic | F64 | `0.0` | `x.unwrap_or(0.0)` |
| RC6 string concat / interp | F64 | `"null"` | `x.map(\|v\| v.to_string()).unwrap_or_else(\|\| "null".to_string())` |

**scope 限定の根拠 (YAGNI)**: T6-2 の empirical probe (Hono benchmark + i144 cell matrix) で
RC1 arith F64 / RC6 concat F64 以外の (type, RC) cell が narrow stale context で登場する
case が無かったため初期実装を 2 cell に限定。将来別 (type, RC) で発火する case が出た
時点で cell 追加 (その時点で (F64, RC4 Boolean) / (F64, RC5 Match disc) 等に拡張予定)。

### 5. Closure reassign Policy A: FnMut + explicit block scope fallback

Closure が外側 narrow 変数を reassign するケース (C-2a/b/c) の Rust emission は
**Policy A (FnMut + `let mut`)** を default に採用 (PRD v2.1 Phase 3b)。

```rust
// Policy A default emission (Hono + matrix cell に対し sufficient):
let mut x: Option<T> = Some(...);
let mut reset = || { x = None; };
reset();                  // NLL で borrow scope 短縮、後続 read 可能
x.unwrap_or(default) ...
```

**Escape 検出アルゴリズム** (`narrowing_analyzer/closure_captures.rs`): closure 変数が
(1) return される、(2) 親関数 callee に渡される、(3) struct field / array element に
代入される、(4) async / promise context に渡される — いずれかで Policy B
(`Rc<RefCell<Option<T>>>`) に切替予定。現時点では Policy A で全 matrix cell green
のため、Policy B emission は unimplemented (T6 scope 外、escape 検出で fallback が必要に
なった時点で追加)。

### 6. Per-cell E2E fixture + `observation ✓ ≠ Rust emission ✓` framework

`.claude/rules/spec-first-prd.md` に **Dual verdict (TS / Rust)** 条項として structural
追加 (I-144 T1 で empirical に発見した framework-level 学びの structural 定着)。

I-144 matrix の ✗ cell (9 種) と ✓ regression cell (8 種) は全て
`tests/e2e/scripts/i144/cell-*.ts` 配下に 1 fixture 1 cell で配置、
`test_e2e_cell_i144_*` 関数群で per-cell runtime stdout 一致を assert。T1 E2E fixture
作成を「Rust emission の empirical probe」と位置付け、**tsc observation ✓ + Rust emission
✗ の cell** を T1 時点で判定する責務を fixture 作成側に持たせる。

**帰結**: I-144 T1 で R4 (`&&=` narrow preserve) / F6 (try body narrow preserve) は
observation ✓ だったが Rust emission で E0308 / try body 崩壊判明 → I-161 / I-149 別 PRD
scope に re-classification。本 PRD は observation + empirical probe の 2 層で spec を
ground する structural 規範となった。

### 7. `transformer/mod.rs` の cohesion 分割 (T6-6 IMPL-6)

`transformer/mod.rs` は I-144 以前から 1086 LOC の broken window (threshold 1000 超過)
で、T6-1 で `build_option_get_or_insert_with` (+30 LOC) を追加した結果 1117 LOC に達した。
T6-6 で 3 つの cohesive sub-module に分割して 718 LOC (-399 LOC) に減量:

- **`transformer/helpers/option_builders.rs`** — Option-shape IR builders
  3 種 (`build_option_unwrap_with_default` / `_get_or_insert_with` /
  `_or_option`) + unit tests。共通 "eager/lazy dispatch on
  [`Expr::is_copy_literal`]" pattern を持つ。既存 `helpers/coerce_default` /
  `helpers/truthy` と並置して cross-module helpers を一元化。
- **`transformer/injections.rs`** — post-IR-generation helper injections
  (`js_typeof` fn + `use regex::Regex;` import) + 2 `IrVisitor` detector
  (`RuntimeTypeofDetector` / `RegexDetector`) + tests。全 post-process
  injection を `Transformer::transform_module` 終端の 2 line 呼び出しに集約。
- **`transformer/ts_enum.rs`** — `convert_ts_enum` + variant value
  formatter (`format_bin_expr` / `format_simple_expr`)。`TsEnumDecl` →
  `IR::Item::Enum` 変換の完結 unit で、formatter は enum 内 private 限定。

**根拠**: `Transformer` struct / core lifecycle / 公開 API entry point は
`mod.rs` の責務だが、Option shape builders・IR visitor ベース detector・
TsEnum 専用 formatter は orthogonal な concern で cohesion が別物。
1 ファイルに混在させると「なぜここに?」が増えるため 3 sub-module に分離。
結果として `helpers/` module が transformer 内の「pure IR-construction +
cross-cutting utilities」の single landing place になる (`coerce_default` /
`truthy` / `option_builders` が同居)。

残存 broken window (I-144 non-contribution の pre-existing):
`registry/collection.rs 1524` / `transformer/expressions/methods.rs 1267` /
`registry/tests/build_registry.rs 1123` / `transformer/statements/tests/
control_flow.rs 1095` / `generator/tests.rs 1068` / `ts_type_info/mod.rs
1045` / `transformer/statements/tests/switch.rs 1028` /
`generator/expressions/tests.rs 1019`。各々別 PRD での抜本的 refactor が必要
(`plan.md`「残存 broken window」section 参照)。

### 8. `ir_body_always_exits` を `pub(crate)` に昇格 (T6-5)

`transformer/statements/control_flow.rs::ir_body_always_exits` は T6-5 で
`transformer/functions/helpers.rs::append_implicit_none_if_needed` から呼べるように
`pub(crate)` 昇格。`control_flow` module 自体も `pub(crate)` 化。

**根拠**: Option 返り値関数の「全 path が exit するか」判定はパターンマッチ heuristic
(if-without-else / while / for の 4 variant 限定) では多分岐 if-else (cell-i025 複合
pattern) を cover 不能。CFG reachability 概念を functions module に import することで
構造的に全 fall-through pattern を判定。DRY: `control_flow` が唯一の exit 判定
authority。

### 参照

- PRD archive (closed 2026-04-21, retrievable via git history): `backlog/I-144-control-flow-narrowing-analyzer.md`
- phase 分割 plan (archived with PRD): `plan.t6.md`
- 実装 module: `src/pipeline/narrowing_analyzer/` + `src/transformer/helpers/`
- T1 red-state report: `report/i144-t1-red-state.md`
- Spec observations report: `report/i144-spec-observations.md`

---

## 残存 broken window

### `Item::StructInit::name: String` に display-formatted `"Enum::Variant"` 形式が格納

`transformer/expressions/data_literals.rs:90` で discriminated union の struct variant 変換時に
`format!("{enum_name}::{variant_name}")` で生成。Rust の enum struct-variant 構文として偶然動作
するが pipeline-integrity 違反。`StructInit` IR に `enum_ty: Option<UserTypeRef>` を追加して
構造化すべき (TODO I-074)。

---

## バージョン / 更新履歴

本ドキュメントは design handoff のアーカイブ。各 section の対応 PRD は section 見出しで明記。
内容が実装と乖離した場合は個別 section を最新化する (削除は禁止 — 過去の設計判断は reference
として保持)。
