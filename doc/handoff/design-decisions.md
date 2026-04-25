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
9. [I-161 + I-171 batch (`&&=`/`||=` desugar + Bang truthy emission)](#i-161--i-171-batch--desugar--bang-truthy-emissionclosed-2026-04-25)
10. [I-178 + I-183 + Rule corpus optimization batch](#i-178--i-183--rule-corpus-optimization-batch-closed-2026-04-25)
11. [Lock-in テスト (削除禁止)](#lock-in-テスト-削除禁止)
12. [残存 broken window](#残存-broken-window)

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

## I-161 + I-171 batch (`&&=`/`||=` desugar + Bang truthy emission、closed 2026-04-25)

`if (x !== null) { x &&= 3; }` 等 narrow-scope `&&=` / `||=` の Tier 2 compile error
(E0308 mismatched types) と `if (!x)` 汎用 Bang truthy emission を structural fix
する batch PRD。8 task (T1-T8) で T2-T7 完了 + T8 close。詳細は git history。

### 1. `truthy_predicate_for_expr` / `falsy_predicate_for_expr` (expr-level API、I-171 T2)

`truthy_predicate(name, ty)` (I-144 T6-3 で導入された Ident 限定 + primitive 限定)
を generalize し、任意の `Expr` operand × 全 `RustType` variant に対応する expr-level
helper を新設。dispatch は `predicate_primitive_with_tmp` (Bool/F64/String/int) /
`predicate_option` (Option<T>) / `const_truthiness_with_side_effect`
(always-truthy types) の 3 路に分割。`Option<synthetic union>` は per-variant
`matches!` chain (Some(U::V(_)) if truthy)、`Option<Named other>` は `is_some()`
shortcut。`is_pure_operand` で Ident/Lit 等は tmp-bind skip、Call/BinaryOp 等は
`TempBinder`-vended `__ts_tmp_op_<n>` で single-evaluation 保証。

**設計判断**: `predicate_primitive_with_tmp` は ref-count-aware (T4 IG-2 fix):
F64 だけが predicate body で 2 回 operand を読む (`<op> != 0.0 && !<op>.is_nan()`)
ため tmp-bind 必須、Bool/String/int は 1 回のみ参照で tmp-bind 不要。snapshot
noise を抑える structural decision。

`is_always_truthy_type(ty, synthetic)` helper は Vec/Fn/Tuple/StdCollection/
DynTrait/Ref/Named-non-synthetic を真と判定、`!<always-truthy>` const-fold +
synthetic union enum 判定 (synthetic registry lookup) で synthetic union を除外。

### 2. `peek_through_type_assertions` (runtime-no-op wrapper 統合、I-171 T2)

TS の `as T` / `!` / `<T>` / `as const` / `(...)` は全て runtime-no-op (型 check 影響のみ)。
`unwrap_parens` (Paren のみ) を generalize した recursive helper。`!(x as T)` /
`if (x!)` / `((x))` 等 wrapper 越しの shape 判定で identical な narrow / emission
を保証。

**設計判断 (T6 P2/P3a)**: control_flow.rs の `try_generate_primitive_truthy_condition`
+ guards.rs の `detect_early_return_narrowing` Bang arm を `unwrap_parens` →
`peek_through_type_assertions` に移行、syntactic variants が emission gap を
作らない invariant を確立。peek-through は runtime-no-op wrapper のみ unwrap、
`Bang` / `Assign` / `Bin` 等 observable な wrapper は保持 (peek-through 後の
shape match で別 dispatch lane が発火)。

### 3. Bang Layer 1 (`convert_unary_expr`) の 5-layer dispatch (I-171 T4)

Bang `!x` 変換を以下 5 layer の優先順位で dispatch (`convert_bang_expr` 関数):

1. **Peek-through** (Paren/TsAs/TsNonNull/TsTypeAssertion/TsConstAssertion 後の
   inner で再 dispatch、`!(x as T)` 等を再帰)
2. **`try_constant_fold_bang`** (`!null`/`!undefined`/`!arrow`/`!fn`/`!always-truthy-ident`
   等を `Expr::BoolLit(b)` に const-fold)
3. **Double-neg `!!<e>`** (= `truthy_predicate_for_expr` ; literal const-fold は
   再帰 `try_constant_fold_bang(inner)` 経由で TypeResolver 非依存に decidable)
3b. **De Morgan on `Bin(LogicalAnd/LogicalOr)`** (`!(a && b)` → `!a || !b`)
3c. **Assign desugar** (`!(x = rhs)` → `{ let tmp: <lhs_ty> = rhs; x = tmp [.clone()]; <falsy(tmp)> }`、
   tmp に LHS 型を annotation することで TypeResolver の expected-type wrap (`Some(...)`)
   と整合、非 Copy LHS は `.clone()` で tmp を保持)
4. **General `falsy_predicate_for_expr`** (5 layer fallback)
5. **Fallback raw `!<operand>`** (Any/TypeVar の compile-time fence、explicit error
   surface)

**設計判断 (IG-3/IG-4/IG-5/IG-6 — deep deep review fix)**: tmp の type annotation
は LHS 型 (RHS 型ではない、TypeResolver の Some-wrap と整合)。非 Copy LHS は
`x = tmp.clone()` で tmp を保持 (`is_copy_type()` 判定)。Layer 3c は AST op check
ではなく **IR shape check** で `Expr::Assign` を識別、`+=`/`-=` 等 arithmetic
compound は normalised IR shape で desugar、`&&=`/`||=`/`??=` は non-Assign IR
(If/Block) を emit するため自然 skip (conditional semantics 保持)。Double-neg
recurse (IG-6) で inner operand が `Assign`/`LogicalAnd`/`LogicalOr` の場合
`convert_bang_expr(inner)` を recurse して outer `Not` で wrap、Layer 3b/3c が
先発火 + outer Not で正しい truthy 意味論を emit。

### 4. Bang Layer 2 (`try_generate_option_truthy_complement_match`) の 3-form 統一 (I-171 T5)

`if (!x) <body> [else <else_body>]` on `Option<T>` を `OptionTruthyShape` enum で
3-form に統一して emit:

1. **EarlyReturn** (else 不在 + then always-exit):
   `let x = match x { Some(x) if truthy => x, _ => { exit_body } };` —
   post-if narrow を outer let rebinding で materialize (T6-3 由来)。
2. **EarlyReturnFromExitWithElse** (else 存在 + then always-exit + else 非 exit):
   `let x = match x { Some(x) if truthy => { else_body; x }, _ => { exit_body } };` —
   post-if reachable only via else 経由 narrow を outer let に rebind
   (T5 SG-T5-DEEP1 retroactive)。
3. **ElseBranch** (else 存在 + 上記以外):
   `match x { Some(x) if truthy => { else_body }, _ => { then_body } }` —
   narrow を `Some(x)` arm にスコープ。

**設計判断**: `OptionTruthyShape` enum で shape を pre-classify、`build_option_truthy_match_arms`
が shape 情報を受け取って primitive arm / synthetic union per-variant arm /
always-truthy single-arm を一元生成 (DRY)。`is_always_truthy_type` で Named
non-synthetic / Vec / Fn / Tuple / StdCollection / DynTrait / Ref に対し
`Some(x) => <body>` 単一 arm without truthy guard を emit、post-narrow access
(`x.field` / `x.method()`) を可能化 (T5 SG-T5-FRESH1 fix)。

`convert_if_stmt` 末尾に **const-fold dead-code elimination** を追加: condition
が `Expr::BoolLit(true)` → then_body 直返却、`Expr::BoolLit(false)` → else_body
or 空 stmt list。Layer 1 const-fold が emit した `BoolLit` を Layer 2 で
if-wrapper ごと除去、PRD 「ideal output」基準に整合 (`if true { ... }` 残骸 /
unreachable post-if コード根絶)。

### 5. T7 cohesion verification (cohesion gap 検出 + I-177-D 起票で委譲、revert 含む)

T7 PRD task は「classifier × emission cohesion 検証」が deliverable。実装 deliverable:

- **5 E2E cells (T7-1〜T7-5)** + **T7-6 unit test** で empirical 検証実施
- **T7-3 cell で architectural cohesion gap を発見**: `FileTypeResolution::narrowed_type(var, position)`
  の closure-reassign suppression scope が enclosing fn body 全体で broad すぎ、
  cons-span 内 (if-body 内、narrow が valid な scope) も含めて narrow を suppress
  → IR shadow form (cons-span 内 x: T) と TypeResolver Option<T> view の不整合 →
  `convert_assign_expr` for `x &&= 3` 等 Option-shape body ops が IR shadow と
  mismatch (E0599 chain)
- **三度の `/check_job` iteration で 4 件 defect 発見** (Truthy 誤発火 / INV-2
  path 3 symmetric 欠落 / sub-case (b) test 不完全 / Scenario A regression):
  workaround patch (predicate form `if x.is_some() { body }`) を試行、各 iteration
  で発見した defect を順次修正
- **Scenario A regression** (`return x` body without `??` × closure-reassign で
  E0308 mismatch) は構造的 trade-off (pre-T7 shadow form は narrow-T-shape ✓ /
  Option-shape ✗、post-T7 patch (predicate form) は narrow-T-shape ✗ /
  Option-shape ✓、どちらの form でも一部 body shape が破綻) で **patch では解消
  不能** と判定
- **`ideal-implementation-primacy.md` の interim patch 条件** (1) PRD 起票 ✓ /
  (2) `// INTERIM:` annotation ✗ / (3) silent semantic change なし ✓ / (4) removal
  criteria ✗ のうち (2)(4) 未充足 + structural fix の patch 降格に該当 → **revert 実施**
- **architectural fix を I-177-D PRD に委譲**: `narrowed_type` suppression scope
  refactor (案 C: cons-span 内 narrow 保持 + post-if scope のみ suppress) で
  IR shadow form と TypeResolver narrow が agree → narrow-T-shape body と
  Option-shape body 両方で works → trade-off 構造解消

**設計判断 (revert の根拠)**: T7 PRD の本来の deliverable は cohesion verification
であり fix は副産物。fix が patch であると判明したら fix を撤去して documented
finding として残すのが正しい。revert で git history が clean に保たれ、
architectural fix を I-177-D で structural に解消することで `ideal-implementation-primacy.md`
完全準拠を達成。

**T7 三度の `/check_job` iteration の framework lesson** (本 PRD scope 外、
TODO `[I-178-5]` Rule 10 + `[I-183]` `/check_job` 4 層化 framework として起票):
defect の連続発見 root cause は (a) 直積 enumeration 不足 (解決軸内の coverage
は意識しているが直交軸を見ていない) + (b) review 深度の iteration 依存 (initial
review が mechanical only)。

### 6. 関連 issue / 後続 PRD

- **I-177**: narrow emission v2 umbrella (mutation propagation defect、T6-3
  inherited) + sub-items A/B/C (typeof/instanceof/OptChain × post-narrow / query
  順序 / symmetric direction) + sub-item D (suppression scope refactor、T7
  architectural fix の本体)
- **I-178-5**: spec-first-prd Checklist Rule 10 (Cross-axis matrix completeness)
- **I-183**: `/check_job` 4 層化 framework rule (mechanical / empirical /
  structural cross-axis / adversarial trade-off)
- **I-179 / I-180 / I-181**: I-171 T4 で発見の orthogonal blocker (synthetic union
  literal coercion / E2E async-main / tuple destructuring)

### 参照

- PRD archive (closed 2026-04-25, retrievable via git history):
  `backlog/I-161-I-171-truthy-emission-batch.md`
- T1 red-state report: `report/i161-i171-t1-red-state.md`
- T7 cohesion report: `report/i161-i171-t7-classifier-emission-cohesion.md`
  (cohesion gap trace + revert 経緯 + body shape × emission form trade-off matrix)
- 実装 module: `src/transformer/helpers/truthy.rs` (expr-level API) +
  `src/transformer/helpers/peek_through.rs` (wrapper unwrap) +
  `src/transformer/expressions/binary.rs` (Layer 1 Bang dispatch) +
  `src/transformer/statements/option_truthy_complement.rs` (Layer 2 if-stmt
  consolidated match) + `src/pipeline/narrowing_analyzer/guards.rs` (Bang arm
  peek-through + OptChain narrow event)

---

## I-178 + I-183 + Rule corpus optimization batch (closed 2026-04-25)

### Background

I-161 + I-171 batch close (2026-04-25) で発見された 2 系統の framework gap (`I-178` Rule 6-10 拡張、`I-183` `/check_job` 4-layer 化) を解消し、併せて `.claude/rules/` corpus 全体の DRY / cohesion / format / cross-reference を一括整備した batch PRD。次の matrix-driven PRD (I-177-D / I-177) 起票時に同 root cause の再発を構造的に防ぐ framework prerequisite として位置付け。

### Audit findings (corpus 全体評価、2026-04-25)

`.claude/rules/` 18 file (1175 LOC total) を 8 観点 (DRY / cohesion / cross-reference / format / 階層構造 / coverage gap / dead-stale rule / naming) で audit。主要発見:
- **DRY violation**: "ideal implementation" 概念が 5 file 重複記述、matrix 概念が 3 file 重複、Tier 分類が `S1` / `Tier 1` の 2 命名混在
- **Cohesion 肥大**: `spec-first-prd.md` (249 LOC) が 4 concern (Stage workflow + Adversarial Checklist + Defect Classification + `/check_job` Stage Dispatch) を含む
- **Cross-reference 不整合**: `Related Rules` table が 4/18 file のみ、one-way 参照多数
- **Format 不統一**: Japanese / English header 混在、`Trigger` vs `When to Apply` 揺らぎ
- **Coverage gap**: `/check_job` review に対応 rule 不在 (I-183 で解消)

### Rule 6-10 wording の refinement (Part 1)

TODO に既記述された draft を 5 観点 (明確性 / lesson grounding / verification feasibility / scope precision / 既存 rule 重複) で批判的再評価し、以下の改善を適用:
- **Rule 6 (Matrix/Design integrity)**: 「spec-traceable に一致」→「token-level に一致」 + side-by-side diff verification 手順を明示
- **Rule 7 (Control-flow exit sub-case completeness)**: "body-exit" → "control-flow exit" に一般化 (将来 try/catch, switch にも適用可能)、集約禁止例 3 種列挙
- **Rule 8 (Cross-cutting invariant enumeration)**: 4 必須項目 (Property statement / Justification / Verification method / Failure detectability) 必須化、候補 invariant カテゴリ 5 種列挙 (探索 prompt として活用)
- **Rule 9 (Dispatch-arm sub-case alignment)**: Spec→Impl と Impl→Spec の双方向 verification を明示
- **Rule 10 (Cross-axis matrix completeness)**: 3 step procedure (axis enumeration / orthogonality verification / Cartesian product expansion) + 8 default check axis を提示

### File 構造の変化

```
Before (18 file in .claude/rules/):
  spec-first-prd.md (249 LOC, 4 concern)

After (21 file in .claude/rules/):
  spec-first-prd.md (~194 LOC, Stage 1/2 lifecycle 専念)
  spec-stage-adversarial-checklist.md (NEW, ~156 LOC) — 10-rule 集約
  check-job-review-layers.md (NEW, ~338 LOC) — 4-layer + Stage Dispatch
  post-implementation-defect-classification.md (NEW, ~112 LOC) — 5-category trace
```

### Tier 2 (corpus optimization) で実施

- **DRY 解消 (5 file)**: `todo-prioritization.md` / `prd-completion.md` / `design-integrity.md` / `conversion-feasibility.md` / `problem-space-analysis.md` の "ideal implementation" 再記述を `[ideal-implementation-primacy.md](...)` reference 1-line に縮小。`S1` notation を `Tier 1` に統一。`conversion-correctness-priority.md` line 24-32 を `type-fallback-safety.md` reference に縮小
- **Format unification**: `ideal-implementation-primacy.md` / `todo-prioritization.md` の Japanese section header (`第一目標` / `数値指標の位置付け` / `判断フロー` / `関連ルール`) を English (`Top-Level Goal` / `Metric Positioning` / `Decision Flow` / `Related Rules`) に統一。`dependencies.md` / `testing.md` の `Trigger` / `Actions` を `When to Apply` / `Constraints` に rename
- **Cross-reference 双方向化**: 全 21 rule に `## Related Rules` table 必須化。one-way reference を双方向化

### `/check_job` 4-layer framework (I-183)

`/check_job` review が現状 static analysis (Layer 1 mechanical) 中心で、empirical / structural / adversarial 層が deep iteration でのみ実施される pattern を構造的に解消。新 rule `check-job-review-layers.md` で 4 layer (Mechanical / Empirical / Structural cross-axis / Adversarial trade-off) を初回 invocation で全実施することを規範化。`/check_job deep` / `deep deep` modifier は廃止。

各 layer は 5 sub-section (責務 / Verification methodology / 必要 artifacts / Output format / Failure mode) で完全 spec、各 layer の出力 format (table / matrix) も明示。

### Defect classification 5-category の独立化

`post-implementation-defect-classification.md` 新設で Grammar gap / Oracle gap / Spec gap / Implementation gap / Review insight の 5 category と trace 方法を独立 spec。**Spec gap は framework 失敗 signal** として framework 自体の改善検討対象であることを明記、改善検討手順 (Rule 10 の axis 追加 / Rule 8 の invariant カテゴリ追加 / Rule 9 の verification 強化) を rule 内で記述。

### Tier 3-4 deferrals

以下を TODO 起票 (本 batch から除外):
- [I-184] `.claude/rules/INDEX.md` 新設 (rule corpus 俯瞰 meta-doc、L0/L1/L2/L3 hierarchy 可視化)
- [I-185] Versioning section の全 rule 統一
- [I-186] Rule naming clarification (`dependencies.md` / `testing.md` rename)
- [I-187] Glossary 整備 (SDCDF / Phase 用語集)
- [I-188] frontmatter `paths:` 整合性確認
- [I-189] `testing.md` の test design techniques 分離検討 (conditional)
- [I-190] `todo-prioritization.md` Step 0 (INV) 分離検討 (conditional)

### Quality gate

本 PRD は documentation/rule のみで code 変更なし。cargo test / clippy / fmt は pre/post で同一結果 (regression なし)。検証は文章 self-review (typo / cross-reference 整合性 / format 統一) で実施。次回 matrix-driven PRD (I-177-D) で新 framework が動作 verify される。

### Phase 2 拡張 (post-Phase-1 audit follow-up、2026-04-25)

Phase 1 完了直後の full-corpus audit (rules + skills + commands + CLAUDE.md + 関係性) で、rule layer の整備が skill / command / CLAUDE.md layer まで貫徹していない **asymmetric completion gap** を検出。Phase 2 で structural 解消:

**Audit findings (Phase 2 trigger)**:
- 16 skill 中 1 (`analyze-ga-log`) + 9 command 中 8 が CLAUDE.md Workflow table 欠落 (Discoverability gap)
- 21 rule 中 9 rule が CLAUDE.md で言及ゼロ (cross-layer reference gap)
- `prd-template` skill が post-batch 新設 rule (`spec-stage-adversarial-checklist` / `check-job-review-layers` / `post-implementation-defect-classification`) を参照していない (S1+S2 stale reference)
- skill / command layer に `Related Rules / Skills / Commands` table が存在せず、rule layer のみ bidirectional の asymmetric framework
- skill / command 自体の作成 procedure missing (`rule-writing` skill のみ存在、skill-writing / command-writing なし)
- `session-todos.md` path / role が `ideal-implementation-primacy.md` で reference されるが定義なし
- `correctness-audit` の trigger threshold が "5+ PRDs" magic number

**Phase 2 解消内容**:
1. **prd-template skill update**: 10-rule checklist + 4-layer review への参照、Matrix Completeness Audit 5 項目 → `spec-stage-adversarial-checklist.md` への delegation で DRY
2. **correctness-audit skill update**: `conversion-correctness-priority` / `type-fallback-safety` / `pipeline-integrity` / `testing` rule への explicit reference
3. **CLAUDE.md update**: Code of Conduct に 3 新 rule + 6 既存 rule (command-output-verification / conversion-correctness-priority / type-fallback-safety / pipeline-integrity / design-integrity / dependencies) を追加。Workflow table を Skills / Commands の 2 sub-table に分割、欠落 commands 8 + skill 1 を追加
4. **全 16 skill に `Related Rules / Skills / Commands` table 必須化**: rule layer に対称化、bidirectional reference graph 完成
5. **全 9 command を structural form に restructure**: action chain + Related table 付き、Variant note で similar skill / command との差別化を明示
   - `/start`: 10 lines → action chain (Step 1-4 で stage 別 skill invoke を明示)
   - `/end`: 1 line → 5-step chain + skill / rule reference
   - `/bench`: hono-cycle skill との light vs full 差別化 Variant note
   - `/check_problem`: `/check_job` Layer 4 との light vs structural 差別化
   - `/refresh_todo_and_plan`: `todo-grooming` skill との event-driven vs periodic 差別化
   - `/step-by-step`: 専用 stage skill 推奨を明示 (vague trigger の disambiguation)
   - `/refresh_report` / `/semantic_review` / `/check_job`: Related table 追加
6. **skill-writing skill + command-writing skill 新設**: framework artifact 作成 procedure を完全 spec、`rule-writing` skill と並列の 3 sibling 構成 (rule / skill / command 各々の writing skill が存在)
7. **`session-todos.md` 定義**: path = project root 直下、role = interim patch 削除基準集約、format example、不在時の禁止条件を `ideal-implementation-primacy.md` に追記
8. **`correctness-audit` trigger 4 条件 rule 化**: N=5 PRDs / Phase boundary / Tier promotion / User-requested の 4 条件、magic number 排除

**Framework symmetry の到達範囲 (実態、Phase 3 self-review 後 2026-04-25)**:

| Direction | Phase 1 後 | Phase 2 後 (実態) | 備考 |
|-----------|-----------|-------------------|------|
| rule ↔ rule | ✓ | ✓ | 全 21 rule に `Related Rules` table、双方向 |
| rule → skill | ✗ | ✗ **uni-directional 維持** | rule layer の `Related Rules` table は **rule entry のみ**。skill から rule への参照は確立 (skill 側 table) だが、rule から skill への back-ref は未実装。Discoverability は CLAUDE.md Workflow table と各 skill の `When to Apply` で代替 |
| rule → command | ✗ | ✗ **uni-directional 維持** | 同上、command 側のみ rule reference を持つ。CLAUDE.md Commands sub-table が discoverability hub |
| skill ↔ skill | △ ad-hoc 5 か所 | ✓ | 全 18 skill に `Related Rules / Skills / Commands` table、双方向 |
| skill ↔ command | ✗ 0 | ✓ | skill 側 / command 側 双方の table で双方向参照 |
| command ↔ command | ✗ 不明示 | △ partial | `/check_job` ↔ `/check_problem` ↔ `/semantic_review` の triad は双方向。残 6 commands は Variant note + 1-way Related table |

**Phase 2 完了時点での over-claim 訂正 (Phase 3 self-review 由来 2026-04-25)**: 当初 design-decisions.md / plan.md は「全 direction で bidirectional reference graph 確立」と claim したが、self-review で **rule layer は cross-layer back-reference を持たない** ことが confirmed。実態は「skill / command layer 内 + skill ↔ command + skill ↔ rule (skill 側のみ) + command ↔ rule (command 側のみ) + rule ↔ rule で reference graph 確立」が正確。Discoverability は CLAUDE.md Workflow table が rule + skill + command 全 mention で hub 機能を担うため、rule → skill / command back-ref の追加は YAGNI と判断 (本 batch では実装しない)。

**Phase 2 で達成された structural value**: skill / command layer の **Related table 必須化 + structural form (action chain + Variant note)** により、skill ↔ rule / skill ↔ skill / skill ↔ command / command ↔ skill / command ↔ rule の 5 direction で確立。rule ↔ skill / rule ↔ command の back-reference は CLAUDE.md hub model で代替。

**Net delta (Phase 1 + 2)**: 新規 5 file (~900 LOC) + 修正 25+ file (rule + skill + command + CLAUDE.md + plan.md + TODO + design-decisions)、production code 変更ゼロ。

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
