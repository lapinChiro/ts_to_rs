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
11. [I-177-E + I-177-B + I-177-F batch (narrow framework cohesion 完成)](#i-177-e--i-177-b--i-177-f-batch-narrow-framework-cohesion-完成closed-2026-04-26)
12. [PRD 2.7: framework Rule 改修 + audit script CI 化](#prd-27-framework-rule-改修--audit-script-ci-化closed-2026-04-27)
13. [Lock-in テスト (削除禁止)](#lock-in-テスト-削除禁止)
14. [残存 broken window](#残存-broken-window)

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

### 1. `__ts_` prefix namespace reservation (I-153/I-154 + T7 + I-224 T1)

ts_to_rs が emission する全 internal identifier は `__ts_` prefix で統一。reservation の **canonical 単一 source of truth** は [`check_ts_internal_label_namespace`](../../src/transformer/statements/mod.rs) (`src/transformer/statements/mod.rs:27-47`) の doc comment — 新規 reservation を追加する際は必ず同 doc を更新する。本 handoff doc は 3 category (labels / value bindings / function rename target) の handoff 説明であり、固定 list の duplication は避ける。

#### 1-a. Labels (statement-level、I-153/I-154 origin)

| Label | 位置 | 用途 |
|-------|------|------|
| `'__ts_switch` | `src/transformer/statements/switch.rs:179` (`TS_INTERNAL_SWITCH_LABEL`) | switch case body 内 nested break の target (conditional wrap 発動時のみ emit) |
| `'__ts_try_block` | `src/transformer/statements/error_handling.rs:125` | try body の throw / break / continue rewrite 先 |
| `'__ts_do_while` | `src/transformer/statements/loops.rs:360/382` | do-while body 内 continue の rewrite 先 (needs_labeled_block 発動時) |
| `'__ts_do_while_loop` | `src/transformer/statements/loops.rs:356` | do-while の outer Loop label fallback (user label なしの時) |

#### 1-b. Value bindings (expression-level、setter / UpdateExpr / compound-assign desugar 用、I-205 由来)

| Binding | constant | 位置 | 用途 |
|---------|----------|------|------|
| `__ts_old` | `TS_OLD_BINDING` | `src/transformer/expressions/mod.rs:62` | postfix UpdateExpr / compound assign の old-value 保存 (B4 setter dispatch) |
| `__ts_new` | `TS_NEW_BINDING` | `src/transformer/expressions/mod.rs:75` | prefix UpdateExpr / compound assign の new-value 保存 (B4 setter dispatch) |
| `__ts_recv` | `TS_RECV_BINDING` | `src/transformer/expressions/mod.rs:103` | side-effect 持ち receiver の 1-evaluate 保証用 IIFE binding (I-205 INV-3 source-side single-evaluation contract) |

#### 1-c. Function rename target (module-level、I-224 T1 で追加、INV-5)

| Identifier | constant | 位置 | 用途 |
|------------|----------|------|------|
| `__ts_main` | `TS_MAIN_RENAME` | `src/transformer/expressions/mod.rs:133` | top-level executable script の `fn main` 自動生成時に user-defined `function main` を rename して collision 回避 (I-224 = B2 fn main mechanism)。INV-5 で Tier 2 honest reject |

#### 1-d. Lint enforcement (label-side + module-level identifier-side、symmetric)

User の `__ts_*` prefix 使用は **2 axis × 全 reachable site** で構造的 reject:

- **Label axis** (`check_ts_internal_label_namespace` @ `src/transformer/statements/mod.rs`、3 entry points + defense-in-depth):
  - `convert_stmt::ast::Stmt::Labeled` (label 宣言)
  - `convert_stmt::ast::Stmt::Break` (labeled break 参照)
  - `convert_stmt::ast::Stmt::Continue` (labeled continue 参照)
  - defense-in-depth: `convert_labeled_stmt` (loops.rs) 内にも同 check
- **Module-level identifier axis** (`check_ts_internal_fn_name_namespace` @ `src/transformer/statements/mod.rs:88` 経由 `scan_for_ts_namespace_collisions` @ `src/transformer/namespace_lint.rs`、I-224 T1 で追加):
  - `transform_module` / `transform_module_collecting` 双方が A-axis dispatch (`is_executable_mode` / `detect_user_main` / `try_capture_module_item_into_main_stmts`) の **手前** で全 module-level Decl を walk、Fn / Class / Var (BindingIdent) / Using / TsInterface / TsTypeAlias / TsEnum / TsModule + ExportDecl-wrapped / ExportDefaultDecl の各 variant で `__ts_*` 識別子を Tier 2 honest reject
  - SWC AST `_` arm 不在 (Rule 11 (d-1) 自己適用準拠、新 variant 追加時は compile error で全 dispatch 強制更新)

#### 1-e. Reservation rationale + INV-5 (I-224 source)

`__ts_` namespace reservation は ts_to_rs の **rename / synthesis mechanism の structural foundation**。reservation 不在では以下の silent collision risk が発生する:

- **Label**: user `__ts_switch:` label が存在すると nested break rewrite が user label を target にする silent semantic change (= I-153/I-154 origin)
- **Value binding**: user `let __ts_old = ...` と T7 setter dispatch の `__ts_old` 内側 emission が同 scope で重複し semantic shadowing (= T7 origin)
- **Module-level identifier (I-224 T1 + INV-5)**: user `function __ts_main()` と I-224 synthesize の rename target `__ts_main` が module scope で collision、Rust E0428 duplicate definitions で compile fail。lint で **identifier-level reservation を A-axis structural dispatch より優先** することで synthesis の prerequisite を構造的に保証

INV-5 (I-224 invariant) は本 reservation を `__ts_main` に拡張、reachable B4 cells (matrix # 9 / 19 / 20 / 29 / 39 / 40 / 49 / 59 / 69 / 79 / 80) で全 collision を Tier 2 honest reject する。詳細は [`backlog/I-224-top-level-fn-main-mechanism.md`](../../backlog/I-224-top-level-fn-main-mechanism.md) `## Invariants > INV-5` 参照。

#### 1-f. CI invariant lock-in

`pub fn init` mechanism (旧 library mode emission target) は I-224 T4 で structural fix 完了、CI script `scripts/audit-no-pub-fn-init.sh` (= INV-4 lock-in) が enforced paths (`src/` / `tools/` / `tests/e2e/rust-runner/`) を merge gate として scan、再混入を 0 hits invariant で block する (失敗時は exit 1 で merge block)。

`tests/e2e/scripts/**/*.rs` (cell-by-cell の `cargo run -- <fixture>.ts` 出力 artefact) は `.gitignore` 配下の **working-tree-only artefact** (= committed history に存在せず、各 developer の `cargo run --` 実行履歴に応じて post-I-224-T4 converter 出力 (`fn main`) と pre-I-224-T4 converter 出力 (`pub fn init`、過去 invocation 由来) が混在し得る)。CI fresh clone では生成自体が起きないため audit advisory hits は 0 件 (CI exit code 不影響)、ローカル working tree のみ advisory として表示する設計。advisory mode は **transient working-tree state** を可視化する補助情報であり、enforced paths の 0 hits invariant とは independent な軸。

**引継ぎ**:
- 新規 `__ts_*` reservation 追加時は **必ず `src/transformer/statements/mod.rs:27-47` の canonical doc** + 該当 constant 定義 + 該当 lint walker (label 側 or module-level 側) を同時更新する。doc-only 追加は禁止 (= reservation の structural enforcement に bypass する route が生まれる)
- user identifier 衝突は label / module-level 双方で lint reject される。SWC parser が未定義 label への break を accept する挙動 (tsx は reject) にも対応済
- 変数名 hygiene の history 詳細は I-159 参照 (本 section が現在の単一 source of truth)

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

1. `convert_method_signature` (interface method) — `pipeline/type_converter/interfaces.rs:466`
2. `convert_callable_interface_as_trait` (callable interface) — `pipeline/type_converter/interfaces.rs:141`
3. `convert_ident_to_param` (class method / ctor) — `classes/members.rs:453`
4. `convert_fn_type_to_rust` (embedded fn type) — `utilities.rs:127`
5. `try_convert_function_type_alias` (fn type alias) — `pipeline/type_converter/type_aliases.rs:370`
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

`label == "__ts_try_block"` self-skip (transformer/statements/error_handling.rs:436) は
TryBodyRewrite が自身の nested try labeled block を re-rewrite しないための guard。I-154
rename 時に更新済。

**引継ぎ**: TryBodyRewrite の label 名を更新する際は transformer/statements/error_handling.rs:436
の self-skip check も同期更新。

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

## I-177-E + I-177-B + I-177-F batch (narrow framework cohesion 完成、closed 2026-04-26)

Plan η Step 1.5 + 2 + 2.5 batch close。I-177-D (TypeResolver suppression scope refactor、closed 2026-04-26) で確立した narrow framework を、Synthetic registry / leaf type lookup / fn body traversal の 3 軸で cohesion 完成させた structural fix 群。後続 PRD (mutation propagation 本体、I-177-A else_block_pattern、I-177-C symmetric XOR、I-048 closure ownership) の prerequisite として 3 つの cross-cutting invariant を確立。

### 1. Synthetic fork query consistency (I-177-E、INV-CE-1)

**Invariant**: `SyntheticTypeRegistry::fork_dedup_state()` の戻り値で `synthetic.get(name)` を query した場合、parent で `get(name)` が `Some(def)` なら fork でも `Some(def)` を返す。

**実装** (`src/pipeline/synthetic_registry/mod.rs::fork_dedup_state`):

```rust
pub fn fork_dedup_state(&self) -> Self {
    Self {
        types: self.types.clone(),                       // ← MUST clone (formerly empty BTreeMap)
        union_dedup: self.union_dedup.clone(),
        struct_dedup: self.struct_dedup.clone(),
        intersection_enum_dedup: self.intersection_enum_dedup.clone(),
        struct_counter: self.struct_counter,
        synthetic_counter: self.synthetic_counter,
        type_param_scope: Vec::new(),
    }
}
```

**Why**: `external_types::load_builtin_types()` が builtin pre-registered union 型 (`union:F64,String → F64OrString` など 22+ entries) を `synthetic.types` に積む。`pipeline/mod.rs` の `synthetic.fork_dedup_state()` を空 BTreeMap で fork すると、TypeResolver が `string|number` の register 試行で **`union_dedup` から hit するが `types` には存在しない** という inconsistent state を生む。`compute_complement_type` (`narrowing_analyzer/guards.rs`) が `synthetic_enum_variants(name)` 経由で fork.types を query して None → narrow event 失効 (silent type widening、`else_branch_complement` と post-if `EarlyReturnComplement` が両方 push されない)。

**Why unit test cover 不能**: `SyntheticTypeRegistry::new()` を直接使う unit test では builtin loader が走らないため再現不能。production pipeline 経由の integration / E2E でのみ顕在化。後続 PRD で synthetic registry を fork する場合は **integration / E2E test 必須**。

**Memory cost**: per-file fork で ~10-50 KB clone overhead。Hono 規模 (158 file) でも合計 MB order 未満で acceptable。

### 2. Canonical type lookup precedence (I-177-B、INV-CB-1/CB-2)

**Invariant**: 任意の AST 位置における型 lookup は「Ident なら narrowed_type 優先 → expr_type fallback、非 Ident なら expr_type のみ」の **単一 contract** を持つ。3 production site (`get_type_for_var` / `get_expr_type` / `collect_expr_leaf_types`) は同一 narrow state input に対し同一 type を返す。

**実装** (`src/pipeline/type_resolution.rs::FileTypeResolution`):

```rust
impl FileTypeResolution {
    /// Canonical: name + span lookup with narrow precedence
    pub fn resolve_var_type(&self, name: &str, span: swc_common::Span) -> Option<&RustType> {
        self.narrowed_type(name, span).or_else(|| self.expr_type_for_name(name, span))
    }

    /// Canonical: Expr lookup (Ident → resolve_var_type、non-Ident → expr_type)
    pub fn resolve_expr_type(&self, expr: &ast::Expr) -> Option<&RustType> {
        match expr {
            ast::Expr::Ident(id) => self.resolve_var_type(&id.sym, id.span),
            other => self.expr_type(other.span()),
        }
    }
}
```

3 production site は thin wrapper:
- `Transformer::get_type_for_var` → `resolve_var_type`
- `Transformer::get_expr_type` → `resolve_expr_type`
- `transformer::return_wrap::collect_expr_leaf_types` → `resolve_expr_type(...).cloned()`

**Why**: pre-fix では `collect_expr_leaf_types` (`return_wrap.rs`) のみ precedence が逆順 (expr → narrow fallback only on Unknown) で encode されていた。`expr_type(span)` は Ident に対し declared union type (`F64OrString`) を `ResolvedType::Known` で常に返すため、narrowed_type は永久に query されず、return-wrap が declared union を見て variant 解決失敗 (hard error or silent semantic risk)。

**Why DRY violation 解消が必要**: 3 site で同一 knowledge を独立 encode する状態は、新規 site 追加時に precedence ずれが発生する構造的 risk。canonical primitive 経由の単一契約に統合することで future addition でも自動的に正順 (defense-in-depth)。

**新規 site 追加時の規約**: 型 lookup 用途で `narrowed_type` と `expr_type` を別々に呼ぶ pattern は `FileTypeResolution::resolve_*_type` 以外で禁止 (grep verification 対象)。

### 3. Function body traversal uniformity (I-177-F、INV-CF-1/CF-2)

**Invariant**: 任意の TypeScript function 形式 (declaration / arrow / function-expression / class constructor / class method) の body block stmt は **`visit_block_stmt` 経由で walk** する。`current_block_end` は body block の `.hi` に set される。`detect_early_return_narrowing` の `EarlyReturnComplement` push がこの invariant 依存。

**実装** (`src/pipeline/type_resolver/`):

| Site | Path |
|------|------|
| `visit_fn_decl` | `visitors.rs:129` |
| `resolve_arrow_expr` (`BlockStmtOrExpr::BlockStmt` arm) | `fn_exprs.rs:201-206` |
| `resolve_fn_expr` | `fn_exprs.rs:257-262` |
| `visit_class_decl` constructor body | `visitors.rs:480-486` |
| `visit_method_function` (class method) | `visitors.rs:537-541` |

5 site 全てで `for stmt in &body.stmts { self.visit_stmt(stmt); }` 形式の **直接 iterate を禁止**、必ず `self.visit_block_stmt(body);` を呼ぶ。

**Why**: `visit_block_stmt` 内部で `current_block_end = Some(body.span.hi.0)` を save/restore する mechanism を持つ。直接 iterate すると `current_block_end` が None のまま、`detect_early_return_narrowing` (`visitors.rs:735-740`) で:

```rust
if then_exits && !else_exits {
    if let Some(block_end) = self.current_block_end {  // ← None なら skip
        let if_end = if_stmt.cons.span().hi.0;
        detect_early_return_narrowing(&if_stmt.test, if_end, block_end, self);
    }
}
```

の None match → post-if scope に `EarlyReturnComplement` narrow event を push しない (typeof / instanceof / nullcheck / OptChain / Bang guard 全て affected、Truthy direction は useful narrow なしで no-op)。

**Why scope expansion**: 初版 PRD scope は `resolve_arrow_expr` / `resolve_fn_expr` の 2 site のみ。`/check_job deep deep` Layer 3 (Structural cross-axis) 監査で「Method (class method body) / Constructor (class constructor body) も同 traversal bug を持つ可能性」が enumerate され、grep audit で confirmed → scope に編入。後続の **新規 fn body site 追加時は必ず `visit_block_stmt` 経由必須** (本 invariant の symmetric maintenance)。

**Nested scope safety**: nested function context (fn 内 arrow / arrow 内 fn etc.) で `current_block_end` は `visit_block_stmt` 内 `prev_block_end` save/restore により stack-like に正しく挙動する (新 fix で broken なし)。

### 関連 PRD / framework signal

- **I-177-D** (closed 2026-04-26): TypeResolver `narrowed_type` suppression scope refactor (案 C trigger-kind-based dispatch)。本 batch の cohesion 完成 base
- **I-177-G** (TODO、L4): `apply_substitutions_to_items` round-trip mutation safety (defense-in-depth、現状 reachability なし)。I-177-E fork inheritance fix で顕在化候補に
- **I-198** (TODO → PRD 2.7 で吸収): non-matrix-driven PRD でも cross-axis enumerate を適用する framework rule 拡張 (5 件 Spec gap chain で reinforced、`spec-stage-adversarial-checklist.md` Rule 10 sub-rule (e) Mandatory 化として PRD 2.7 で実装済)

### 参照

- 実装 module: `src/pipeline/synthetic_registry/mod.rs::fork_dedup_state` (I-177-E) + `src/pipeline/type_resolution.rs::FileTypeResolution::resolve_*_type` (I-177-B canonical primitive) + `src/pipeline/type_resolver/{visitors.rs, fn_exprs.rs}` の 5 fn body site (I-177-F)
- E2E fixture: `tests/e2e/scripts/i177-e-synthetic-fork-narrow-cohesion.ts` + `i177-b-leaf-narrow-cohesion.ts` + `i177-f-arrow-fn-expr-block-end.ts`
- PRD archive (closed 2026-04-26、retrievable via git history): `backlog/I-177-E-synthetic-fork-types-inheritance.md` + `backlog/I-177-B-collect-expr-leaf-types-cohesion.md` + `backlog/I-177-F-resolve-arrow-fn-expr-block-end.md`

---

## PRD 2.7: framework Rule 改修 + audit script CI 化 (closed 2026-04-27)

I-198 + I-199 + I-200 cohesive batch。framework Rule 3/4/10/11/12 改修 + TypeResolver coverage extension (StaticBlock + Prop::Method/Getter/Setter body resolve + AutoAccessor Tier 2 honest error reported) + ast-variants.md Prop/PropOrSpread/Decorator section 新規追加 + audit scripts CI 化。15 task + 4-layer review (initial invocation で 9 課題発見) + Implementation Revision 1 (PropOrSpread Grammar gap) + Revision 2 (cell 15 Prop::Assign critical Spec gap、Tier 2 honest error reclassify) を self-applied integration として実施。

### Framework rule 改修 (lesson source は本 PRD)

詳細は `.claude/rules/spec-stage-adversarial-checklist.md` の各 Rule 内 Lesson source 引用 (= 本 PRD 自身が first-class adopter として self-applied verify)。

| Rule | 改修内容 |
|------|----------|
| Rule 3 (3-1)〜(3-3) | NA cell 確定前に **SWC parser empirical observation 必須** (TS spec ≠ SWC parser behavior、Implementation Revision 2 source) |
| Rule 4 (4-1)〜(4-3) | doc-first dependency order の structural enforcement (PRD 内 doc update task = code 改修 task の prerequisite、`audit-prd-rule10-compliance.py` で auto verify) |
| Rule 10 axis (i) | AST dispatch hierarchy: parent enum + child enum の各 layer を独立 axis として enumerate (PropOrSpread 最外層 + Prop 内側 dispatch のような layer 混在記述禁止) |
| Rule 11 (d-1)〜(d-4) | AST node enumerate completeness check: `_ => ` arm 全面禁止 + phase 別 mechanism (Transformer = `UnsupportedSyntaxError`、TypeResolver = no-op + reason comment、NA = `unreachable!()`) + `doc/grammar/ast-variants.md` single source of truth + `audit-ast-variant-coverage.py` CI 化 |
| Rule 12 (e-1)〜(e-8) | Rule 10/11 Mandatory application + structural enforcement (matrix-driven / non-matrix-driven 区別なし)、Permitted/Prohibited reasons 列挙、machine-parseable yaml block format hard-code、`prd-template` skill Step 0c で必須 section 化 |

### audit script CI mechanism (後続 PRD prerequisite)

PRD 2.7 で導入された 2 audit script は **全 PRD で merge gate** として動作する (`.github/workflows/ci.yml`)。後続 PRD 起票 / 実装時の prerequisite:

| Script | 役割 |
|--------|------|
| `scripts/audit-prd-rule10-compliance.py` | PRD doc parse + Rule 10 application section + prohibited keywords (「scope 小」「light spec」等) 不在 verify + Rule 4 (4-3) doc-first dependency chain auto verify。yaml fenced code block parse |
| `scripts/audit-ast-variant-coverage.py` | tree-sitter-rust 経由で `_` arm 全面禁止 verify + `doc/grammar/ast-variants.md` Tier sync verify (Tier 1 Handled / Tier 2 Unsupported reported via `UnsupportedSyntaxError`)。`--files <impact-area>` flag で本 PRD scope file の pre-draft audit に利用 (Rule 11 (d-5) `## Impact Area Audit Findings` section embed mandatory) |

新規 PRD 起票時は `prd-template` skill Step 3-pre で `audit-ast-variant-coverage.py --files <impact-area>` を run し、結果を PRD doc の `## Impact Area Audit Findings` section に embed する。Anti-pattern keyword (`scope 小` / `light spec` / `pragmatic` / `~LOC` / `短時間` / `manageable` / `effort 大` 等) は PRD doc 内禁止 (`feedback_no_dev_cost_judgment.md` 整合)。

### TypeResolver coverage extension (production code、本 PRD scope)

```
src/pipeline/type_resolver/
├── visitors.rs    # visit_class_body の StaticBlock arm + AutoAccessor explicit no-op + TsIndexSignature/Empty filter-out reason
└── expressions.rs # Object expr inner match で Prop::Method/Getter/Setter body の visit_block_stmt 経由 walk + visit_prop_method_function helper
                   # Prop::Assign は Implementation Revision 2 で no-op (cell 15 NA → Tier 2 honest error reclassify)

src/transformer/expressions/data_literals.rs
  # 3 site (convert_object_lit + convert_discriminated_union_object_lit + try_convert_as_hashmap)
  # 全 wildcard 削除 + UnsupportedSyntaxError 経由 Tier 2 honest error 統一
```

`doc/grammar/ast-variants.md` に PropOrSpread (section 12) + Prop (section 13) + Decorator (section 20) 追加。AutoAccessor entry を **Tier 2 honest error reported via UnsupportedSyntaxError** + I-201-A/B 言及に update。

### Implementation Revision lessons (recurring problem evidence)

PRD 2.7 は Implementation stage で **2 度 Spec への逆戻り** を実施。Spec gap chain 5 → 3 → 0 → 1 → 0 → 1 → 0 trajectory で完了。

- **Revision 1** (PropOrSpread Grammar gap): T11 (`ast-variants.md` update) 中に PropOrSpread section 不在を発見、Grammar gap として本 PRD scope 内 fix。Rule 10 axis (i) AST dispatch hierarchy 追加の lesson source。
- **Revision 2** (cell 15 Prop::Assign critical Spec gap): cell 15 (`Prop::Assign` in object literal context、`{ x = expr }`) を当初 NA 認識 (TS spec parse error 前提) で `unreachable!()` macro 設計 → SWC parser empirical observation で **accept** 確認、precondition violation 発覚 → Tier 2 honest error reclassify。Rule 3 (3-1)〜(3-3) SWC parser empirical observation 必須 sub-rule の lesson source。

### 関連 PRD / 後続作業

- **I-203** (TODO、user 承認 2026-04-27): Codebase-wide AST match exhaustiveness compliance (Rule 11 (d-1) 既存 codebase application、本 PRD で確立した `_` arm 全面禁止 + `unsupported_arm!()` macro pattern を codebase 全体に適用)
- **I-204** (TODO): Transformer StaticBlock emission strategy 改修 (cell 6 GREEN 化用、L1 候補)
- **I-201-A** (PRD 2.8、user 承認 2026-04-27): AutoAccessor 単体 Tier 1 化 (decorator なし subset)。本 PRD で `ast-variants.md` AutoAccessor entry を Tier 2 → Tier 1 (decorator なし subset) 昇格する base が確立
- **I-201-B** (PRD 7、user 承認 2026-04-27、L1): Decorator framework 完全変換 (TC39 Stage 3、audit 2026-04-27 で完全未実装 = silent drop = Tier 1 silent semantic change と判明)
- **I-202** (PRD 2.9、user 承認 2026-04-27): Object literal Prop::Method/Getter/Setter Tier 1 化

### 参照

- framework rule: `.claude/rules/spec-stage-adversarial-checklist.md` (各 Rule 内 Lesson source 引用)
- audit script: `scripts/audit-prd-rule10-compliance.py` + `scripts/audit-ast-variant-coverage.py` + `.github/workflows/ci.yml`
- production code: `src/pipeline/type_resolver/{visitors.rs, expressions.rs}` + `src/transformer/expressions/data_literals.rs`
- grammar reference: `doc/grammar/ast-variants.md` の PropOrSpread / Prop / Decorator section
- E2E fixture: `tests/e2e/scripts/prd-2.7/` (cell 6 = post-PRD I-204 として `#[ignore]`、cell 10/11 GREEN)
- SWC parser empirical regression: `tests/swc_parser_object_literal_prop_assign_test.rs` (Rule 3 (3-2) empirical lock-in test)
- PRD archive (closed 2026-04-27、retrievable via git history): `backlog/PRD-2.7-rule10-coverage-extension.md`

---

## I-224: top-level fn main mechanism + framework v12-2 candidate empirical 補強 chain (closed 2026-05-09)

PRD I-224 (B2 fn main mechanism + Option β cohesive batch) の close 経緯と、本 PRD で発見した **framework v12-2 candidate (= "Spec wording / claim vs 実体 work cross-check") の empirical recurrence chain** を保存。本 section は I-D PRD batch (= framework rule integration) 起票時の **primary lesson source**。

### Architectural concern statement (本 PRD discovery context、future similar PRD の reference)

**問題**: TS module-load semantics ≠ Rust binary entry の semantic mismatch。TS では module body の top-level statements が module load 時に実行されるが、Rust binary は `fn main()` のみ entry point。ts_to_rs は当初 `function main()` user 定義時のみ Rust `fn main()` emit、それ以外は `pub fn init()` のみ emit (= never called) で **Tier 1 silent semantic change** を発生。

**解決**: 80-cell matrix (Axis A 8 × Axis B 5 × Axis C 2、Axis E orthogonality merge declaration) で全 reachable cell の dispatch を spec、3-tuple match dispatch tree で `(is_executable_mode, user_main_kind, has_top_level_await)` から 13 reachable arms に 1-to-1 mapping (Rule 9 (a) compliance)、`MainStmt` IR + `synthesize_fn_main` で `#[tokio::main] async fn main()` (top-await present) または sync `fn main()` (top-await absent) を emit。

**Future similar PRDs 参考価値**: TS → Rust semantic mismatch を扱う 다른 PRDs (e.g., module-level closure capture、Rust orphan rule との衝突、destructuring pattern lifting 等) で同 architectural concern statement template (= "問題 = TS X semantics ≠ Rust Y、解決 = matrix + dispatch tree + IR enum") が再利用可能。

### Option β cohesive batch decision pattern (= "scope 分離 を理由とした compromise" 排除 mechanism)

**Iteration v2 → v3 で発生した structural decision**:

- **Iteration v2 design**: cells 14-18/30 + 6/7/8 (= top-level await ✗ / NA cells) を「test harness limitation」を理由に新 PRD I-226 (test harness ESM upgrade + top-level await Tier 1 化) へ defer
- **Third-party review H-2 finding**: `Rule 12 (e-3) Permitted reasons` (= "infra で AST input dimension irrelevant" / "refactor で機能 emission decision なし" / "pure doc 改修") に該当しない **gray zone violation**、**ideal-implementation-primacy 観点で「実装範囲が広い (test harness 跨ぎ)」を defer 理由とする compromise を排除**
- **Iteration v3 fix**: Option β cohesive batch 採用 (= 本 PRD scope を "Top-level executable script form の Rust emission strategy + verify infrastructure" cohesive concern に拡張)、I-226 起票撤回、TS-5/TS-6 + T7/T8/T9 task 追加

**Lesson (cross-PRD applicable)**: 「test harness / lib API / external dependency limitation を理由に PRD scope を分離する」decision は `feedback_no_dev_cost_judgment.md` 違反 risk = `Rule 12 (e-3)` Permitted reasons に該当するか **explicit verify 必須**。該当しない場合は cohesive batch で本 PRD scope に integrate (= "1 PRD = 1 architectural concern" boundary を architectural concern relevance で再定義、scope 規模ではなく concern boundary で判断)。

### 4-axis Problem Space + Axis E orthogonality merge declaration pattern (= matrix size reduction methodology)

**問題**: 4-axis full Cartesian product = **160 cells** = matrix table が manual review で扱える density limit を超える (cross-reference consistency 維持困難)。

**解決 (Axis E orthogonality merge declaration、Rule 1 (1-4) compliant)**:
- Axis E (Module export presence E0/E1) を入力次元として明示
- ただし Axis E は本 PRD architectural concern (= fn main mechanism + executable mode dispatch) の dispatch logic に **直接影響しない** (= Rust binary crate 内 `pub` modifier 有無は library 公開度の concern、execution order の concern と orthogonal)
- E0/E1 cells は同一 dispatch logic を通過、E1 cells では既存 path で `pub` modifier preserve (= regression lock-in)
- **matrix sub-axis 化せず**、`## Problem Space > Axis E Orthogonality Probe` sub-section で structural verify (= Implementation Stage T3 で `test_axis_e_export_preserve_symmetric` で probe lock-in)

**結果**: 160 cells → **80 cells** matrix (Axis A 8 × Axis B 5 × Axis C 2)、各 cell ideal output が E0/E1 共通。

**Lesson (cross-PRD applicable)**: 4+ axes Cartesian product を取る PRD で、ある axis が architectural concern dispatch logic に直接影響しない (= regression preserve のみ要求) 場合、`Rule 1 (1-4) Orthogonality merge legitimacy + Spec-stage structural verify` 適用で matrix を 1 axis 分 reduce 可能。条件:
- (1-4-a) Orthogonality verification statement 明示 (= source cell # 明示)
- (1-4-b) Spec-stage structural consistency verify (= AST shape level で identical)
- (1-4-c) Spec-stage referenced cell symmetry probe (= Implementation Stage で unit test 経由 lock-in)

### 25 NA cells unified mutual exclusion reasoning + SWC parser empirical lock-in pattern

**問題**: 80 cells matrix のうち 25 cells (Axis A0/A2/A4/A5a/A5b + C1) が **AST 構造的 mutually exclusive** (= Axis C C1 = top-level await は AST 上 `Stmt::Expr(Expr::Await)` または `Decl::Var` with `Expr::Await` init を要求、これは A0/A2/A4/A5a/A5b 各 partition と空集合)。

**解決 (Rule 3 (3-1/3-2) compliant)**:
1. **Spec-traceable NA reason 統一**: 25 cells を 1 unified reasoning ("Axis A vs Axis C1 構造的 mutual exclusion") で record (Rule 3 (3-1) `spec-traceable な根拠のみ` compliant)
2. **SWC parser empirical lock-in test**: `tests/swc_parser_top_level_await_test.rs` 4 tests で structural reasoning を verify (Rule 3 (3-2) `SWC parser empirical observation 必須` compliant):
   - `test_top_level_bare_await_parses_as_stmt_expr_await_axis_a1` (= A1 partition lock-in)
   - `test_top_level_var_decl_with_await_init_parses_as_decl_var_axis_a3` (= A3 partition lock-in)
   - `test_pure_axis_a0_source_contains_no_await_expression` (= A0 partition lock-in)
   - `test_axis_c1_implies_a1_or_a3_partition_synthesis` (= C1 forms 4 variations → A1/A3 collapse synthesis lock-in)

**Lesson (cross-PRD applicable)**: NA cells が複数 partition で発生する場合、**1 unified mutual exclusion reasoning** + **SWC parser empirical lock-in test 4 件** で structural lock-in 達成可能 (= 25 cells × 個別 NA reason ではなく、1 reasoning + 4 tests で coverage)。Future PRD で similar AST shape vs feature axis の mutual exclusion がある場合、本 pattern を template として採用可能。

### 3-tuple dispatch tree + per-item runtime decision split pattern (= Rule 9 (a) 1-to-1 mapping compliance)

**問題 (Iteration v4 Critical 1 + Medium 1 fix)**: 旧 4-tuple match `(is_executable_mode, user_main_kind, is_async_required, has_lit_top_level_const)` で 2 つの structural defects:
- **Critical 1 (axis-tuple ↔ definition mismatch)**: Library mode + FnAsync user main arm が `is_async_required=false` を pattern claim、しかし `is_async_required = (FnAsync || has_top_level_await)` 定義より cells #5/#25 は `is_async_required=true` で `unreachable!()` panic に fall-through する logical bug
- **Medium 1 (1-to-1 mapping violation)**: `has_lit_top_level_const` 次元は A6 (mixed) cells で複数 arm に partition (cells #71/#72 等が 2 arms に double-claim)、Rule 9 (a) 1-to-1 違反

**解決 (Iteration v4 structural fix)**:
- **3-tuple match に simplify**: `(is_executable_mode, user_main_kind, has_top_level_await)` で 80 cells が **1-to-1 mapping** (各 leaf に matrix # annotation)
- **`has_lit_top_level_const` を per-item runtime decision に移行**: top-level item iteration 中、各 Decl::Var with Lit init のみ Item::Const として top-level emit、それ以外は MainStmt として fn main capture (= dispatch dimension から分離)
- **`test_dispatch_arm_one_to_one_mapping_per_in_scope_cell` regression lock-in test** で axis-tuple ↔ definition mismatch を structural detect (= future iteration で同種 bug 混入時 unit test fail で発覚保証)

**Lesson (cross-PRD applicable)**: dispatch tree の axis-tuple 次元数増加は **logical bug + 1-to-1 mapping violation** の risk source。**axis-tuple 次元削減 + per-item runtime decision split** が structural fix pattern。並列 verify として helper test (= `tests/<prd>_helper_test.rs`) で各 cell ↔ dispatch arm の 1-to-1 mapping を assert することで regression lock-in。

### INV-1〜INV-7 4-item invariant pattern (Rule 8 (8-c) compliance、cross-PRD applicable)

本 PRD は 7 invariants を spec、各 invariant は **4-item structure (a)(b)(c)(d)** で記述 (Rule 8 invariant verification の structural completeness 保証):

| Item | Content | 例 (INV-1 source) |
|------|---------|------------------|
| (a) Property statement | 1 文で書けるレベルの不変条件 | "Cell A != A0 の全 cell で TS module top-level statements の execution order が Rust fn main body 内で byte-exact preserve" |
| (b) Justification | なぜこの invariant が必要か (違反でどんな defect class) | "違反すると TS execution stdout と Rust execution stdout が divergent = Tier 1 silent semantic change" |
| (c) Verification method | 実装後に invariant 成立を verify する具体手順 (probe / test / static analysis) | "Per-cell E2E fixture で TS / Rust stdout の byte-exact match を verify (TS-3 で fixture 作成、T6 で green 化)" |
| (d) Failure detectability | invariant 違反が compile error / runtime error / silent semantic change のどれで顕在化するか | "silent semantic change (Rust compile pass + runtime stdout divergent)" |

7 invariants spec した dimensions (= future PRD invariant design template):
- **INV-1**: Source-order preservation (= top-level execution order invariant)
- **INV-2**: User symbol preservation (= rename + substitute invariant)
- **INV-3**: Sync/async dispatch consistency (= multi-trigger OR-condition invariant)
- **INV-4**: Mechanism 廃止 invariant (= legacy code 0-hits structural lock-in)
- **INV-5**: Namespace reservation extension consistency (= I-154 namespace rule + collision detection)
- **INV-6**: Layer separation invariant (= 他 pipeline phase に side effect なし)
- **INV-7**: External API breaking change audit (= reachability empirical 0-confirm)

**Lesson (cross-PRD applicable)**: 7 invariants の dimension coverage (= source-order / symbol / dispatch / mechanism 廃止 / namespace / layer separation / external API audit) は **structural concern を持つ PRDs の invariant design template** として再利用可能。各 invariant に 4-item structure を適用 + `tests/<prd>_invariants_test.rs` に test stub を Spec stage iteration v7 段階で作成 (= "deferred verification = unverified claim" compromise を排除する Spec stage convention、I-205 v1.6 self-applied integration pattern 踏襲)。

### 6-category test layout pattern (matrix-driven PRD test architecture template)

本 PRD で確立した test layout は future matrix-driven PRDs の template として再利用可能:

| Category | Location | Purpose | I-224 example |
|----------|----------|---------|---------------|
| **Unit tests** | `src/transformer/main_synthesis/tests.rs` (= module-internal) | Module logic の Equivalence partitioning + Boundary value + Decision Table + AST variant exhaustiveness coverage | 75 unit tests (collect_top_level_executions / synthesize_fn_main / classify_init_kind / detect_user_main / etc.) |
| **Integration tests** | `tests/<prd>_namespace_test.rs` / `tests/<prd>_decl_var_dual_path_test.rs` 等 (= cross-module integration) | Public API E2E + boundary value | I-224 では namespace + Decl::Var dual-path |
| **Helper test contracts** | `tests/<prd>_helper_test.rs` (= dispatch arm 1-to-1 mapping verify) | Rule 9 (a) compliance + axis-tuple ↔ definition consistency lock-in | `test_dispatch_arm_one_to_one_mapping_per_in_scope_cell` + `test_axis_b_b1a_b_c_rename_dispatch_symmetric` + `test_axis_e_export_preserve_symmetric` + `test_axis_a5a_compositional_orthogonality_with_b_axis` |
| **Invariants verification tests** | `tests/<prd>_invariants_test.rs` (= INV-N test stubs from Spec stage iteration v7) | Rule 8 (8-c) helper test contracts NEW + invariant lock-in | INV-1〜INV-7 stubs (Spec stage v7 で `#[ignore]` author + Implementation T1〜T9 で fill in) |
| **E2E tests** | `tests/e2e/scripts/<prd>/cell-NN-*.{ts,expected}` + `tests/e2e_test.rs::test_e2e_cell_<prd>_<NN>_*` | Per-cell stdout byte-exact match (TS oracle vs cargo run) | 27+ fixtures + `test_e2e_cell_i224_NN_<semantic_name>` per-cell entry |
| **SWC parser empirical tests** | `tests/swc_parser_<feature>_test.rs` (= Rule 3 (3-2) lock-in、AST shape mutual exclusion) | NA cells の structural reasoning を SWC parser empirical で lock-in | `tests/swc_parser_top_level_await_test.rs` 4 tests (Axis A vs C1 mutual exclusion) |

**Snapshot tests**: 不要 (= IR-level emission concern で snapshot 過剰)。本 PRD は IR token-level assertion を unit/integration tests で cover。

**Lesson (cross-PRD applicable)**: 上記 6 categories で `unit / integration / helper / invariants / e2e / swc-parser-empirical` の coverage を提供 (snapshot は IR-level concern では不要)。Spec stage iteration v7 で全 invariants test stub を `#[ignore]` で author し Implementation Stage で fill in する **Spec stage convention** が "deferred verification = unverified claim" compromise を排除 (= I-205 v1.6 self-applied integration pattern)。

### R-2 / R-4 empirical reachability audit methodology (= breaking change / reserved identifier audit pattern)

本 PRD で確立した audit methodology は future PRDs で再利用可能:

#### R-4 audit methodology (reserved identifier collision audit)

**Method**: `grep -rn '<reserved-identifier>' src/ tests/ tools/`

**Findings classification**:
- Production source (`src/`): 0 hits 期待 (= 既存 namespace constant のみ)
- Test infrastructure (`tests/` 配下、本 PRD 自身の test fixtures を除く): 0 hits 期待 (= breaking change reachability 不在 verify)
- Tools (`tools/`): 0 hits 期待
- 本 PRD 自身の test fixtures: hits 多数 (= 意図的 collision detection / multi-call substitution test fixture、本 audit の対象外)

**External codebase verify (Hono benchmark target)**: Implementation stage T5 で Hono bench Tier-transition compliance verify 時に同 grep を実施 (= INV-5 prerequisite 等)。

**判定**: 0 reachable user-defined collision in scoped paths → reservation extension は existing user code に breaking change 引き起こさない (Rule 12 e-3 Permitted reason "infra で input dimension irrelevant" 適用可能)。

#### R-2 audit methodology (external API breaking change reachability audit)

**Method**: `grep -rn '\b<api-name>\b' src/ tests/ tools/` (definition site enumerate) + `grep -rn '\b<api-call-pattern>\b' tests/<external-test-runner>/` (call site enumerate)

**Hit type classification**:
- Definition site (production): 該当 PRD で削除 + doc comment update
- Definition site reference (test): 該当 PRD で test を新 form に migrate
- Generated snapshot artefacts: e2e re-run で自動 clear、advisory hits 扱い
- Call site (production): 0 hits 期待 = breaking change reachable surface 不在 confirm
- PRD-related comment references: historical reference として keep

**判定**: Call site 0 hits → API 廃止は external API breaking change なし、Implementation Stage 移行 block する reachable surface 不在。

**Lesson (cross-PRD applicable)**: 本 R-2/R-4 audit methodology は any PRD that introduces (a) reserved identifier extensions、(b) public API removals、(c) generated code form changes で再利用可能。Spec stage の TS-N (Pre-Implementation Audit Findings) task として実施し、PRD doc `## Pre-Implementation Audit Findings` section に embed (= Spec stage 移行 block する reachable surface 不在を spec-traceable に確定)。

### Audit script design pattern (`scripts/audit-no-pub-fn-init.sh` template)

本 PRD で新規作成した `scripts/audit-no-pub-fn-init.sh` (+ T5-2 で `scripts/audit-no-init-call-site.sh`) は future PRDs の structural lock-in audit script template:

**Design**:
- **Enforced paths** (= violation 検出で `exit 1`): `src/`, `tools/`, `tests/e2e/rust-runner/`
- **Advisory paths** (= advisory print のみ、`exit 0` 影響なし): `tests/e2e/scripts/` (= generated snapshot artefacts、e2e re-run で自動 clear)
- **Pattern**: `\b<forbidden-identifier-pattern>\b` (Rust source files only、`*.rs` filter)
- **Pre-fix expected behavior**: `exit=1` with N enforced hits + M advisory hits (= 本 audit 時点 record state)
- **Post-fix expected behavior**: `exit=0` (= invariant lock-in、helper 削除 + test migrate + e2e re-run で advisory hits 自動 clear)
- **CI integration target**: `.github/workflows/ci.yml` に integrate、PR merge gate

**Lesson (cross-PRD applicable)**: 本 PRD で確立した "enforced + advisory paths split" pattern は future PRDs の "structural lock-in invariant の CI merge gate 化" で再利用可能。Particularly:
- Enforced/advisory split で **snapshot artefacts の handling pattern を documentation 化** (= e2e regenerate 待ちの advisory hits を error にしない)
- Pre/post expected behavior の **explicit declaration** で CI behavior の **regression detection mechanism** を提供

### 23 sub-commits decomposition + Quality gate per commit + /check_job timing pattern

本 PRD Implementation Stage で確立した commit policy (= user 確定 2026-05-01 post-Spec-stage closure):

- **各 T (T1〜T9) を 2-4 sub-commits に decompose**、合計 23 sub-commits で Implementation 完遂
- **単一焦点 deliverable**: 1 sub-commit = 1 architectural change category (constant 追加 / validator 新規 / IR enum 追加 / dispatch tree 統合 / refactor / e2e green-ify / etc.)
- **Quality gate per commit**: cargo check + cargo test (該当 scope) + cargo fmt --all --check + cargo clippy --all-targets -- -D warnings 全 pass
- **Commit message format** (per `incremental-commit.md`):
  - `[WIP] I-224 T<N>-<sub>: <single-focus deliverable>` (中間 sub-commits)
  - `[WIP] I-224 T<N> 完了: <T-level summary> + 4-layer review pass` (T-完了 commit、`/check_job` 4-layer review post-fix を含む)
  - `[CLOSE] I-224 PRD 完了: ...` (T9-2 final commit のみ)
- **`/check_job` 4-layer review timing**: 各 T 完了 commit (= 各 T の最後の sub-commit) で実施、Layer 1-4 全 0 findings or 全 fix 後に commit

**Lesson (cross-PRD applicable)**: 本 23 sub-commits decomposition pattern は any matrix-driven PRD の Implementation Stage で再利用可能。Particularly:
- "1 sub-commit = 1 architectural change category" boundary で **commit-level cohesion** を保証
- Each T-完了 commit に `/check_job` 4-layer review 実施 = **incremental defect detection** + **post-fix re-commit** mechanism (= sub-commit cycle 内で defect resolve、PRD-level review iteration を avoid)

### v12-2 pattern empirical recurrence chain (= self-applied review accuracy gap、本 PRD で empirical lock-in 達成)

**Pattern definition (framework v12-2 candidate)**: PRD spec の wording (= Sub-commits 一覧の completion criteria + T task description) と production code / infra work の actual state、または PRD review の self-applied claim (= "Critical = 0 / High = 0 / Layer 1-4 全 0 findings") と third-party adversarial review が発見する actual findings の **乖離** pattern。Spec stage adversarial review (12-rule checklist) では検出不能 (= self-applied review は claim 内部の整合性しか check しない)、third-party invocation で初めて発覚する pattern。

**Empirical recurrence chain (本 PRD I-224 における 全 occurrence)**:

| Round | 日付 | Self-claim | Third-party finding | Pattern subtype |
|-------|------|------------|---------------------|------------------|
| **v2 → v3** | 2026-05-01 | Iteration v2 self-claim "Critical=0/High=0" | Third-party `/check_job` で 21 件 actions (Critical 4 + High 8 + Medium 4 + Review 5) | **A. Self-claim accuracy gap (= 1st occurrence、formal v12-2 naming より前)** |
| **v3 → v4** | 2026-05-01 | Iteration v3 self-claim ✓ | Adversarial agent re-review 2nd round 5 件 (Critical 1 + High 2 + Medium 2 + Review 4) | A. Self-claim accuracy gap (2nd) |
| **v4 → v5** | 2026-05-01 | Iteration v4 self-claim ✓ | Adversarial agent re-review 3rd round 13 件 + Compromise audit fix | A. Self-claim accuracy gap (3rd) |
| **v5 → v6 minor** | 2026-05-01 | Iteration v5 self-claim ✓ | Adversarial agent re-review 4th round 2 件 | A. Self-claim accuracy gap (4th) |
| **T2 R1 → R2** | 2026-05-07 | T2 R1 self-applied review ✓ → 5 findings | T2 R2 adversarial deep review で 3 NEW findings | A. Self-claim accuracy gap (5th) |
| **T5-1 /check_job** | 2026-05-08 | T5-1 完了 self-applied | /check_job 3 iteration + /check_problem 2 round 累積 structural fix 5 件 | A. Self-claim accuracy gap (6th-7th) |
| **T6a /check_job** | 2026-05-08 | T6a 完了 self-applied | 1st round line-ref drift 2 件 → 2nd round adversarial で 4 NEW factual fix | A. Self-claim accuracy gap (8th) |
| **T7 (Iteration v12)** | 2026-05-08 | T7 spec wording (= rust-runner tokio dep + ESM-mode runner template + observe-tsc.sh CI invoke) | 実体 infra work 乖離 (= harness 側 ESM mode write が必要、tokio dep 既存、observe-tsc.sh は spec stage tool で CI 不参与) | **B. T-task spec wording vs work reality (= 1st occurrence of subtype B)** |
| **T8 (Iteration v13)** | 2026-05-09 | T8 spec wording (= MainStmt::ExprAwait/LetAwait emission + INV-3 dispatch trigger 拡張) | 実体 production code 乖離 (= T1-T5-2 累積実装で完成済) | B. T-task spec wording vs work reality (2nd) |
| **v13 self-review 1st-round** | 2026-05-09 | PRD doc Final 4-Layer Review section "Layer 1-4 全 0 findings (Defect category)" | Third-party `/check_job` で 7 distinct findings (L1-1/2/3/4 + L3-1/2/3 + L4 Trade-off #4) | A. Self-claim accuracy gap (9th) |
| **v13 self-review 2nd-round** | 2026-05-09 | 1st round fix work "structural cohesion 向上" claim | Third-party `/check_job` で 4 NEW findings (L1-N1/N2/N3/N4) + L3-N1 (= /check_job recursion convergence criterion 不在) | A. Self-claim accuracy gap (10th) |

**Pattern subtype A (Self-claim accuracy gap)**: **計 10 度連続 occurrences** (= 真の structural pattern、formal v12-2 naming より遥かに前から発生していた = framework rule 不在の structural gap)

**Pattern subtype B (T-task spec wording vs work reality)**: **計 2 度連続 occurrences** (= subtype A の sub-class、特に Implementation Stage で発生する subtype)

**Combined v12-2 pattern**: subtype A + B = **計 12 度 occurrences across 13 review rounds in I-224 PRD lifecycle**

**真の framework structural gap signal の lock-in**:
- 1 回 = 事故 / 2 回 = 偶然 / 3 回 = pattern / **5 回以上 = structurally inevitable framework gap**
- 本 PRD I-224 で **12 度 empirical recurrence** = framework rule の structural integrity 確立に **v12-2 candidate (= self-applied + third-party 二重実施 mandatory) は absolute prerequisite**

### Implementation-level structural fixes (Iteration v8〜v11、cross-PRD applicable lessons)

本 PRD Implementation Stage で発生した複数の Spec への逆戻り + structural fix から抽出した cross-PRD applicable design patterns:

| Iteration | Source defect | Structural fix | Cross-PRD lesson |
|-----------|---------------|----------------|-------------------|
| **v8** (2026-05-07、T2 完了時) | `has_top_level_await` AST shape direct interpretation が nested await (`f(await x)` 等) を miss + class super_class outer-context await detection 漏れ + ExportDecl-wrapped Decl::Var with side-effect init を library mode 誤分類 + multi-declarator first-only check 限界 + Lit::Regex inclusion intra-PRD inconsistency (= I-228-a/b/c/d 4 sub-entries) | (a) Recursive walk extension (= AST direct → walker pattern)、(b) ANY-rule for multi-declarator (= first-only → ANY)、(c) ExportDecl unwrapping at module-level scan、(d) Lit::Regex narrow to Lit::Num/Bool/Str/Null/BigInt (= Rust const 適合) | **AST shape direct interpretation は nested cases / multi-declarator / wrapper-decl variants を構造的に miss する pattern** = recursive walker default + ANY-rule for collection + wrapper-decl unwrapping を design template として default 採用。`Lit::Regex` 等の "Rust const 不適合" subtypes を enum variant level で narrow する pattern も同 lesson source |
| **v9** (2026-05-08、T5-1 着手中) | cells 12/24 で `const v: T = { ... };` (= Object/Array literal init) が executable mode で silently dropped → consumer `console.log(v.x)` が undefined → E0425 downstream | `InitKind` enum を 4 variants (Lit / SideEffect / AwaitInit) → **5 variants (+ NonTriggerDef + NonTriggerData)** に split、Object/Array literal を NonTriggerData として明示分類 + per-declarator routing で fn main body capture | **Silent drop の root cause は enum variant の under-classification = "実行 trigger を含まないが captured されるべき shape" を別 variant に split する pattern**。Future PRD で similar silent-drop 発見時に enum variant split structural fix を default 採用 |
| **v10** (2026-05-08、T5-1 完了後 /check_job 3 iter + /check_problem 2 round) | TsAs/TsSatisfies/TsTypeAssertion expected_type propagation 非対称 (= 1 wrapper だけ propagation で他 2 wrapper が dropping) + destructuring Tier 2 silent drop + classify_decl_var_path legacy aggregating classifier の dual-classifier maintenance burden + prd-completion.md Tier-transition wording の "Tier 1 silent → Tier 2 honest" classification 不在 | (a) 3 wrapper symmetric path で expected_type propagation extension、(b) destructuring を Tier 2 honest error reject (= silent drop 排除)、(c) legacy classifier 完全削除 + per-declarator classifier に migration、(d) Tier-transition wording に "Improvement (Tier 1 silent → Tier 2 honest)" classification 追加 | **Type wrapper variants (TsAs/TsSatisfies/TsTypeAssertion) は **symmetric pass-through** が default、片方 wrapper だけ extension は asymmetric drop pattern を発生させる** = 3-wrapper symmetric path enumerate を design template として default 採用。Legacy aggregating classifier の存続は dual-classifier maintenance burden = Implementation Stage で migration 後即削除 |
| **v11** (2026-05-08、T5-2 着手時) | B2 + executable-mode `__ts_main()` substitute call **.await wrap が dropped** → renamed async user main の Future が silently dropped (= cells 11/23/75 で Tier 1 silent semantic loss) + cells 16/30/36 で `convert_expr` substitute-time .await wrap が outer Expr::Await と二重に作用 (= **double-await bug** で `__ts_main().await.await` emission = `()` does not implement Future の compile error) + `transformer/mod.rs` 1005 行 file-size threshold 超過 | (a) `UserMainSubstitution` enum + `from_dispatch` constructor DRY 解消 (= sync/async substitute logic single source of truth) + UserMainKind / UserMainSubstitution / detect_user_main 三者を `user_main.rs` 同居 cohesion 向上、(b) `Transformer::suppress_main_await_wrap` flag + `convert_expr_in_await_context` helper で context-aware suppression structural fix (= 3 entry sites で uniform 適用)、(c) `.claude/rules/file-size-resolution.md` 新設 (= 機械的末尾切り出し禁止 + 周辺ファイル調査 + DRY/凝集度 enumerate + 関連実装も含めた再構成 plan の 4-step procedure) | **(a) Substitute / rewrite logic dispatch arm の symmetric coverage**: sync substitute / async substitute / no substitute の 3 arm 全てが test cell coverage を持つ verify mechanism を Spec stage で確定 (= Rule 9 (a) Dispatch-arm sub-case alignment を substitute logic にも extend)。**(b) Caller-supplied wrap context awareness**: `convert_expr` の substitute-time wrap が source-level wrap と二重に作用する double-wrap bug は Layer 3 直交軸 review 不在で latent 化、Rule 10 axis (i) "AST dispatch hierarchy" に "rewrite / substitute logic の caller-supplied wrap context awareness" を default check axis として追加 (= 改善 v11-3 candidate)。**(c) File size 超過時の機械的末尾切り出し禁止**: 周辺ファイル調査 + DRY/凝集度 enumerate + 関連実装も含めた再構成 plan の 4-step procedure を `file-size-resolution.md` に lock-in (= 改善 v11-2 candidate、本 PRD で NEW rule file 新設) |

**Cross-PRD design pattern summary (v8〜v11 累積)**:

1. **AST recursive walker default + wrapper-decl unwrapping pattern** (v8): `has_top_level_await` 等の "AST に該当 shape が存在するか" predicate で direct interpretation ではなく recursive walker + ExportDecl/TsAs/TsSatisfies/TsTypeAssertion 等 wrapper-decl unwrapping を default 採用
2. **Enum variant split for silent-drop avoidance** (v9): "実行 trigger 不在だが captured されるべき shape" を別 enum variant に split (= classification under-classification 排除)
3. **Wrapper variants symmetric pass-through enumerate** (v10): TsAs/TsSatisfies/TsTypeAssertion 等 type wrapper variants で symmetric extension を default、片方だけ拡張は asymmetric drop pattern を発生
4. **Substitute/rewrite logic dispatch arm symmetric coverage** (v11-1): substitute logic にも Rule 9 (a) Dispatch-arm sub-case alignment を symmetric 適用
5. **Caller-supplied wrap context awareness** (v11-3): `Transformer::suppress_main_await_wrap` flag pattern = caller-supplied wrap context を flag で suppress する design template
6. **`file-size-resolution.md` 4-step procedure** (v11-2): 機械的末尾切り出し禁止 + 周辺ファイル調査 + DRY/凝集度 enumerate + 関連実装も含めた再構成 plan

### Framework 改善 candidate (本 PRD I-224 close 時 I-D PRD batch 起票候補、**計 9 件 NEW** = I-224-derived chain v12 + v13 + v13 self-review 1st-round + v13 self-review 2nd-round)

**Note**: 下記 9 candidates は **I-224 PRD chain から derive された subset**。I-D PRD batch 全体の framework 改善 candidates 累積総数 (= I-178/I-183/I-205/I-399 等の earlier PRDs から derive された candidates も含む) は **計 32 件 / 14 rounds adversarial review** (詳細 = TODO `[I-D]` entry 参照)。本 section の 9 candidates は I-224 由来の primary lesson source として archive。**詳細 chain history は前述「v12-2 pattern empirical recurrence chain」table + 後述「4 度連続 v12-2 pattern recurrence」section 参照** (= v2→v3 から始まる subtype A self-claim accuracy gap × 10 度 + subtype B T-task spec wording vs work × 2 度 = 計 12 度 across 13 review rounds)。

| Candidate | Round source | Rule target | Resolution direction |
|-----------|--------------|-------------|----------------------|
| **v12-1** | Iteration v12 | `spec-first-prd.md` 「Spec への逆戻り」 procedure | "Implementation stage 着手直前 prerequisite 調査 mandatory" sub-step 追加 (= 各 T task 着手直前に "spec wording / completion criteria が現実と整合するか empirical cross-check" を mandatory step として挿入) |
| **v12-2** | Iteration v12 | `check-job-review-layers.md` Layer 3 (Structural cross-axis) | "Spec wording と実体 infra work の cross-check" を Layer 3 default check axis に追加 (= Spec stage / Implementation stage transition 時点で spec wording の実体整合性を第三者視点で empirical verify する mechanism) |
| **v13-1** | Iteration v13 | `spec-first-prd.md` 「Spec への逆戻り」 procedure | v12-1 の structural enforcement strengthening (= manual cross-check 依存だと再発 risk あり、(a) `prd-template` skill / `tdd` skill の Step 0 に automated check 追加 / (b) `audit-prd-rule10-compliance.py` 拡張で auto-detect / (c) 本 PRD I-224 v12+v13 経験を case study として `spec-first-prd.md` に embed) |
| **v13-2** | Iteration v13 | Web API runtime integration (= 別 architectural concern PRD candidate、I-D batch とは scope 性質異なり) | `transpile_with_builtins()` lib API が `pub struct Promise<T>` 定義のみ load + `impl Promise<T> { fn resolve(value: T) -> ... }` 等 method implementation 不在 → fixture が `Promise.resolve(N)` を含むと compile error E0599。Resolution: 別 PRD で `Promise.resolve` / `Promise.all` / `fetch` / `Response.json` 等 method implementation の load_builtin_types extension |
| **v13-3** | Iteration v13 | lib / CLI API consistency restoration (= 別 PRD candidate) | `transpile()` lib API は no-builtin、CLI binary は with-builtin が default = lib / CLI 間で fundamental API inconsistency。e2e harness は `transpile()` 経由 (= CLI と異なる pipeline)。Resolution: (a) `transpile()` を builtins-default 化、(b) e2e harness を CLI binary 経由 subprocess invoke、(c) lib API 設計 rationalize |
| **v13-4** | v13 self-review 1st-round | `check-job-review-layers.md` Layer 4 + `spec-first-prd.md` PRD close procedure | **Self-applied 4-layer review と third-party adversarial review の二重実施 mandatory** at PRD close。Iteration v13 self-review = 3 度連続 v12-2 pattern empirical 補強 source。Resolution: (a) PRD close commit 前の third-party `/check_job` invocation prerequisite 化、(b) self-applied claim "0 findings" を third-party invocation で independent verify、(c) 不一致を framework signal として record |
| **v13-5** | v13 self-review 1st-round | `spec-stage-adversarial-checklist.md` Rule 9 / Rule 13 | Cell numbering convention single-source-of-truth enforcement。本 PRD で INV-3 = matrix # / e2e fixture filename = sequential 2 surface convention drift で reader confusion 発生 (= 同名 "cell-14" が different matrix cells を表す)。Resolution: framework rule で convention 統一 mandatory + audit script auto-detect |
| **v13-6** | v13 self-review 1st-round | `spec-first-prd.md` 「Spec への逆戻り」 procedure | "fixture content 変更時の Oracle re-grounding mandatory" sub-step。本 PRD Iteration v13 fixture rewrite (= Promise.resolve → getVal user-defined async fn pattern) で Spec stage artifact #2 (Oracle Observations) を invalidate したが、formal `scripts/observe-tsc.sh` re-run + Oracle re-document 未実施。Resolution: fixture content modification 時に Oracle re-grounding 手順を mandatory 化 |
| **v13-7 NEW** | v13 self-review 2nd-round (本 round) | `check-job-review-layers.md` Layer 4 + `spec-first-prd.md` PRD close procedure (= v13-4 strengthening) | **`/check_job` recursion convergence criterion**。v13-4 candidate は "self-applied + third-party 二重実施 mandatory" を spec するが **convergence 条件不在** = 1st round → 2nd round で fix work 自体に新 findings 発見 (= meta-finding pattern)、理論的に N round 無限 loop risk。本 round で empirical lock-in (= 1st round 7 findings → fix → 2nd round 4 NEW findings = 1st→2nd round 2 度連続 fix-of-fix pattern)。Resolution direction (4 設計 options を I-D batch 評価): (a) **Convergence criterion**: 0 findings 到達まで recursive invocation、ただし severity classification (Critical/High = continue / Medium/Low = next-PRD-batch defer 可能) / (b) **Max round limit**: e.g., max 3 rounds、3 round で 0 findings 不到達なら remaining findings を I-D batch / new PRD として escalate / (c) **Diminishing returns detection**: round N の findings count が round N-1 と比べて同等以下 + Critical 0 なら convergence と判定 / (d) **Meta-finding tracking**: round N の finding が "round N-1 の fix work 自体に対する finding" (= meta-finding) の場合、別 category として classify (= structurally pure productivity vs perfectionism) |

### 7 度連続 v12-2 pattern recurrence (= v13-4/v13-7 framework strengthening の empirical evidence chain、I-D-pre Phase 3/4 + /check_problem 追加 2026-05-11)

**Archive scope note (2026-05-11、I-D-pre Phase 4 /check_problem Issue #2 user 承認 = Option A broader archive)**: 本 section は元々 I-224 close 時 snapshot (4 度連続) として archive されたが、後続 PRD I-D-pre chain で同 pattern が更に 3 度再発 = **計 7 度連続** が判明、I-D-main batch 起票時 evidence 強化のため本 archive scope を "I-D-pre 系列 + 後続 PRDs 由来 occurrences" に拡張 update。

本 PRD chain (I-224 → I-D-pre) で **v12-2 pattern (= "Spec wording / claim と actual state の乖離 を self-applied review で検出できない")** が **7 度連続再発** = 真の framework structural gap signal:

1. **Iteration v12** (2026-05-08): T7 spec wording (= rust-runner tokio dep 追加 + ESM-mode runner template 拡張 + observe-tsc.sh CI invoke) **vs** 実体 infra work (= harness 側 ESM mode write、tokio 既存、observe-tsc.sh は spec stage tool で CI runtime 不参与) の **乖離**。Spec への逆戻り procedure 発動で resolve。
2. **Iteration v13** (2026-05-09): T8 spec wording (= MainStmt::ExprAwait/LetAwait emission を追加 + INV-3 sync/async dispatch trigger 拡張) **vs** 実体 production code (= T1-T5-2 累積実装で完成済) の **乖離**。Spec への逆戻り procedure 発動 + N/A re-classify で resolve。
3. **v13 self-review 1st-round** (2026-05-09): PRD doc Final 4-Layer Review section "Layer 1-4 全 0 findings" claim **vs** 7 findings reality の **乖離** = third-party `/check_job` で 7 findings (L1-1/2/3/4 + L3-1/2/3 + L4 Trade-off #4) 発見 → in-batch fix 4 件 + I-D batch defer 4 件 (= v13-4/5/6 NEW)。
4. **v13 self-review 2nd-round** (2026-05-09): 1st round fix work "structural cohesion 向上" claim **vs** 4 NEW findings reality の **乖離** = third-party `/check_job` で 4 NEW findings (L1-N1 cross-reference table mnemonic factual inaccuracy + L1-N2 sync branch "only" wording factual error + L1-N3 redundant assertion DRY violation + L1-N4 design-decisions.md "5 件 NEW" stale count) + L3-N1 (= /check_job recursion convergence criterion 不在 = meta-finding) 発見 → in-batch fix + v13-7 NEW candidate 起票。
5. **PRD I-D-pre Phase 3 /check_job 4-layer + deep deep review** (2026-05-11): Phase 3 implementation 直後 self-applied "全 PASS" claim **vs** 9 findings reality の **乖離** = 4-layer review で 6 findings (Spec gap 3 + Implementation gap 2 + Low 4 = A1-A9) → 即時 fix + retroactive Iteration v2 embed、続く deep deep review で 3 additional findings (I3 mechanical 妥協 sys.path.insert + `# noqa` + I4 dual verify 不在 + I5 Closed PRD test 不在) 発見 → 即時 fix + retroactive embed + TODO C4 candidate lock-in。
6. **PRD I-D-pre Phase 4 /check_job 4-layer review** (2026-05-11): Phase 4 implementation 直後 self-applied "findings 0" claim **vs** 4 findings reality の **乖離** = third-party-style `/check_job` で 2 Implementation gaps (L1-1 OOB-via-glob C1 branch coverage + L1-2 range-form partition coverage) + 2 Review insights (R1 v12-2 5 度目補強 retroactive embed + R2 syntactic-verify wording ambiguity = TODO C5) 発見 → 即時 fix + Iteration v4 retroactive embed。
7. **PRD I-D-pre Phase 4 後続 /check_problem** (2026-05-11、本 round): `/check_job` fix work "ideal-clean 達成" claim **vs** 4 additional issues reality の **乖離** = `/check_problem` で 4 issues 発見 (Issue #1 Impact Area stale LOC + #3 backwards-range silent failure + #4 GLOB_ROOTS future-proofing + #5 directory-mixed-drift fixture gap) → Issue #1 + #3 即時 fix (= LOC + byte counts sync + INVALID_RANGE 新 drift category) + TODO C6 candidate lock-in (= Issue #4) + #5 skip 提案 + #2 user 判断で本 archive update 採用。**Recursive self-audit structure 完成 evidence**: Issue #3 fix で audit script byte count 変更 → Path E utility Axis 4 即時 detect → Impact Area row 自動 re-sync を促す cycle で structural drift prevention mechanism が動作することを empirical 証明。

**1 回 = 事故 / 2 回 = 偶然 / 3 回 = pattern / 4 回 = 真の structural framework gap empirical lock-in / 5-7 回 = self-applied review accuracy は framework leverage 無しでは structurally unattainable の empirical 結論補強**。本 7 度連続 chain は I-D-main PRD batch (= framework rule integration) の v13-4/v13-7 candidates (= "self-applied + third-party 二重実施 mandatory + convergence criterion") 必須化 evidence + 最低 1 round 後続 `/check_problem` light review の structural enforcement candidate evidence (= `/check_job` だけでは捕捉漏れ、occurrence 7 でも light review 経由で更に 4 issues 発見 = depth-axis vs breadth-axis review 両軸必要)。

### 関連実装の structural lock-in artifact (PRD doc 削除後 access path)

PRD doc は `backlog/I-224-top-level-fn-main-mechanism.md` から削除済 (`backlog-management` skill "Completed Item Handling" 規定準拠)。Audit trail は git log で `[CLOSE] I-224 PRD 完了` commit 参照。Structural lock-in artifact:

- `src/transformer/main_synthesis/` (= MainStmt IR + UserMainKind + UserMainSubstitution + classify_dispatch_arm + synthesize_fn_main 全 components)
- `tests/i224_invariants_test.rs` (INV-1〜INV-7 全 GREEN、特に INV-3 4 sub-case + Edge sub-case full coverage)
- `tests/i224_helper_test.rs` (Rule 9 (a) 1-to-1 mapping lock-in)
- `tests/e2e/scripts/i-224/cell-*.ts` (80-cell matrix の in-scope 14 + collision 6 + NA 60 cells 全 verdict locked-in)
- `scripts/audit-no-pub-fn-init.sh` + `scripts/audit-no-init-call-site.sh` (CI integrated、INV-4 / INV-7 lock-in)

### 参照

- TODO entry `[I-D]` (= 本 5 candidates v13-4/v13-5/v13-6 + 既存 v12-1/v12-2/v13-1/v13-2/v13-3 集約)
- TODO entry `[I-180]` (= I-224 T9 batch 内 fast-track verify で empirical resolved 2026-05-09、別 PRD 起票不要)
- git log `[CLOSE] I-224 PRD 完了` commit (= 本 PRD audit trail 全保持)

---

## 残存 broken window

### `Item::StructInit::name: String` に display-formatted `"Enum::Variant"` 形式が格納

`transformer/expressions/data_literals.rs:90` で discriminated union の struct variant 変換時に
`format!("{enum_name}::{variant_name}")` で生成。Rust の enum struct-variant 構文として偶然動作
するが pipeline-integrity 違反。`StructInit` IR に `enum_ty: Option<UserTypeRef>` を追加して
構造化すべき (TODO I-074)。

---

## I-D-pre: Audit mechanism bootstrap (Path B split adoption + bootstrapping circularity 構造的解消、closed 2026-05-11)

### 概要

PRD I-D parent Spec Stage Iteration v17 plateau で empirical 発覚した **3rd-order pattern = bootstrap
utility correctness ceiling** (= 各 bootstrap utility が次 round の dominant defect class を自ら生成
する無限 chain) を **Path B (PRD split into I-D-pre + I-D-main)** で構造的解消した bootstrap utility
formal lock-in PRD。Iteration v1〜v5 (5 cells matrix-driven、6 phases implementation、2 度の `/check_job`
adversarial review)、interim patch 0 件、structural fix 14 件 (4 + 10) で convergence 達成。

### Path B split rationale (= ideal-implementation-primacy + 妥協禁止 directive 適用結果)

3 path options 評価:
- **Path E+** (continue): utility correctness ceiling = 無限 chain 継続 = 妥協 → rejected
- **Path F** (criterion re-design): asymptotic floor 受容 = explicit compromise → rejected
- **Path B** (PRD split): bootstrapping circularity 構造的解消 + 1 PRD = 1 architectural concern 原則
  準拠 → **accepted**

Cohesion principle 適合 evidence: 5 audit mechanism cells と 24 rule integration cells が異なる
architectural concern (memory `feedback_prd_cohesion_granularity.md` 整合)。

### 5 cells resolution + 6 phases implementation timeline

| Cell | I-D source | Resolution | Phase |
|------|-----------|-----------|-------|
| 1 | v3-6+v4-2 | `verify_pending_verdict_findings_consistency` + Path E Axis 2 (F7 fix integrated) | Phase 3 |
| 2 | v5-1 | `verify_cross_reference_cell_consistency` + Path E Axis 1 (F6 fix integrated) | Phase 3 |
| 3 | v11-5 | `scripts/audit-handoff-doc-line-refs.py` (NEW 260 行) + CI step + handoff doc 5 ambiguous refs structural fix | Phase 4 |
| 4 | v11-7 | `check-job-review-layers.md` Layer 1 sub-step (4) factual accuracy semantic check + `verify_line_refs.py` Method A formal lock-in | Phase 2 + Phase 5 |
| 5 | v13-5 | `spec-stage-adversarial-checklist.md` Rule 9 (d) + Rule 13 (13-6) cell numbering convention + `verify_cell_numbering_drift_detection` + Path E Axis 3 `CELL_SLOT_AS_IDENTIFIER_RE` | Phase 3 + Phase 5 |

### 主要 lesson source: framework rule-audit symmetry principle (Rule 13 (13-6-c) v1.8 empirical validation)

**2 度独立 iteration で framework 自己改善 cycle 発動**:

1. **1st `/check_job` 4-layer review**: 新 Layer 1 sub-step (4) factual accuracy semantic check author
   直後、(4-3) hard-coded reference の `verify_prd_self_audits.py` Axis 4 verify_external_file_drift に
   **100-byte tolerance threshold** (line 590: `abs(actual_bytes - claimed_bytes) > 100`) 存在 →
   新 rule への infrastructure asymmetry violation = Spec gap。即時 structural fix (tolerance 排除 =
   strict byte-exact comparison、24-byte drift も即時 detect)。
2. **2nd `/check_job` deep deep review**: 新 Rule 9 (d) "single-source-of-truth = matrix #" author
   直後、I-D-pre 自身の `## Cell Numbering Convention` section に "Single-source-of-truth = matrix #"
   と "cell # / Cell N / candidate ID / I-D source / matrix # は single canonical naming" の
   **contradictory wording 並列** 存在 (本 PRD body で "Cell N" を 17 回使用) → 自己違反 = Spec gap。
   即時 structural fix (Conceptual identifier vs Written form 明確分離 + Allowed forms enumeration)。

**= framework rule-audit symmetry principle (Rule 13 (13-6-c) v1.8) の empirical 自己実証**: 新 framework
rule author 時、既存 infrastructure / 自 PRD body との symmetry violation が initial iteration 内で
発見可能、structural fix で fix 完了可能。本 lesson が後続 PRD で同 pattern 発生時の primary reference。

### Recursive self-audit structure 完成 + Path E strict mode 採用

Phase 4 で完成、Phase 5 /check_job で strict mode 化 (= tolerance 排除):
- `scripts/verify_prd_self_audits.py` (Path E utility) が own + sibling utilities (= `verify_line_refs.py` +
  `audit-handoff-doc-line-refs.py` + `audit-prd-rule10-compliance.py`) 全 4 utility の byte claim を
  Impact Area row で auto-verify
- strict byte-exact comparison で 1 byte 単位の drift も即時 detect → Impact Area row sync を強制
- Phase 5 で **3 度連続 cascading detection cycle** (50568→50544 + 20730→20665 + 31728→32520) を
  empirical 実証、recursive self-audit structure が structural drift prevention mechanism として動作

### Framework v1.8 self-applied integration (rule wording strengthening = Phase 5 T2-pre-1 + T2-pre-2)

`.claude/rules/check-job-review-layers.md` v1.8 + `.claude/rules/spec-stage-adversarial-checklist.md`
v1.8 の coordinated self-applied integration (= 互いに cross-reference + 同じ framework v1.8 で同時
昇格、PRD I-205 v1.3 pattern 継承):
- Layer 1 sub-step (4): Factual accuracy semantic check + 3 hard-coded enforcement mechanisms
- Rule 9 sub-rule (d): Cell numbering convention single-source-of-truth (Written form "Cell N",
  Conceptual identifier matrix #、Allowed forms enumeration)
- Rule 13 sub-rule (13-6): Cell numbering convention audit symmetry (section presence verify +
  auto-detect helper as dispatcher + audit ↔ Rule symmetry principle)

### Quality gate post close

| 指標 | 値 |
|------|-----|
| 全 i_d_pre tests | 17 PASS / 5 ignored (= invariants stub for I-D-main retroactive verify、Phase 6 で fill 不要 = INV-5 retroactive 担当) |
| INV-1〜INV-5 | INV-1〜INV-4 達成 / INV-5 = I-D-main 着手後 retroactive verify (= forward-looking criterion per PRD spec) |
| Path E utility | strict byte-exact comparison 採用、I-D-main で 0 drifts maintained |
| INV-4 baseline | post-close 3-tuple (I-050 FAIL preserve / I-205 PASS / I-D-main PASS、I-D-pre は close で audit out-of-scope) |
| Hono bench | Preservation (= 107 clean / 72 errors、production code 0 LOC change、framework infra PRD) |
| cargo clippy / fmt / file-size | 0 warnings / 0 diffs / 0 violations |

### 14 structural fixes / 0 patches (= empirical evidence of "妥協絶対不許可" directive compliance)

- Phase 1-5 implementation: 6 phases / ~10 tasks
- 1st `/check_job` 4-layer review fixes: 4 件 (L1-1 placement + L1-2 aliasing + L3-1 Spec gap Path E + L2-1 byte claim drift)
- 2nd `/check_job` deep deep review fixes: 10 件 (A inline comment / B v1.8 symmetry / D Iteration v5 entry / F Cell Numbering Convention contradictory / C+H test enhancement + 4 cascading byte syncs + 1 audit script re-sync)
- すべて structural fix、interim patch 0 件、ideal-implementation-primacy + Rule 13 (13-6-c) audit-rule symmetry principle 完全準拠

### 後続 PRD への影響

- **I-D-main spec stage 再開**: completed bootstrap utilities full leverage で initial iteration
  convergence target 可能化 (= framework rule-audit symmetry empirical proof 利用、24 cells を Iteration v19 で convergence target)
- **全 future PRDs**: bootstrap utilities (verify_line_refs.py / verify_prd_self_audits.py /
  audit-handoff-doc-line-refs.py) + Layer 1 sub-step (4) factual accuracy semantic check rule + Rule 9 (d) /
  Rule 13 (13-6) cell numbering convention rule の compound benefit、spec stage iteration cost 構造的削減

### Active TODO items spawned post-close

- `[I-205-retroactive-cell-numbering-section]`: 案 γ Phase 2 T15 で I-205 PRD doc に `## Cell Numbering
  Convention` + `## Spec→Impl Mapping` section 追加 (= audit scope 内自動 promote、future-proof design)
- `[I-D-future-vocab-fork]`: broader vocabulary fork detection (cell # / candidate ID / matrix # 間
  semantic-level mixed canonical naming detection) = L4 latent、案 γ Phase 0 完了後再評価
- `[I-D-future-audit-extensions-hardening]`: 6 candidate classes (C1=Path E API stability / C2=byte-exact
  invariant / C3=scripts/ file-size policy / C4=Closed PRD test / C5=4-layer self-applied review action
  embed / C6=GLOB_ROOTS hardcoded auto-detect) cohesive batch = L4 latent
- `[I-D-future-self-applied-symmetry-audit]`: 新 framework rule author 時の既存 PRD body self-applied
  compliance mandatory verify rule (framework v1.9 candidate) = L4 latent、本 PRD 2nd `/check_job`
  deep deep Spec gap #2 由来

---

## バージョン / 更新履歴

本ドキュメントは design handoff のアーカイブ。各 section の対応 PRD は section 見出しで明記。
内容が実装と乖離した場合は個別 section を最新化する (削除は禁止 — 過去の設計判断は reference
として保持)。
