# ts_to_rs 開発計画

## 最上位目標

**理論的に最も理想的な TypeScript → Rust トランスパイラの獲得。**

詳細原則は [`.claude/rules/ideal-implementation-primacy.md`](.claude/rules/ideal-implementation-primacy.md) 参照。
ベンチ数値は defect 発見のシグナルであり、最適化ターゲットではない。

---

## 現在の状態 (2026-04-14)

| 指標 | 値 |
|------|-----|
| Hono bench clean | 113/158 (71.5%) |
| Hono bench errors | 59 |
| cargo test (lib) | 2447 pass |
| cargo test (integration) | 112 pass |
| cargo test (compile) | 3 pass |
| cargo test (E2E) | 90 pass |
| clippy | 0 warnings |
| fmt | 0 diffs |

**I-040 完了による bench 変動**: 変動なし (clean 71.5% / errors 59 維持)。本 PRD は pre-existing
defect 修正で、Hono には該当 optional param パターンが実際には少なく bench に現出していなかった。
変換器内部では 10 経路の Option ラップが統一され、DRY 違反と silent loss の両方を構造的に解消。

**Step 2 完了による bench 変動**: clean 72.2% → 71.5% (-0.7pp)、error 58 → 59 (+1) は
OBJECT_LITERAL_NO_TYPE。strictNullChecks 有効化で `T | undefined` / optional param が正しく
抽出されるようになり、Hono 側の generic 型注釈要求が顕在化した（silent semantic change ではなく、
既存の defect が表面化したシグナル）。Phase B (RC-11 expected type 伝播) 範囲で解消予定。

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

Phase A (compile_test skip 解消) の Step 3 に着手する。

---

## 開発ロードマップ

### Phase A: コンパイルテスト skip 解消

compile_test の skip リストを全解消し、変換品質のゲートを確立する。
skip 解消後は新たな skip 追加を原則禁止とし、回帰検出を自動化する。

**完了済み:**
- Step 0: `basic-types` unskip
- Step 1 (RC-13): `union-fallback`, `ternary`, `ternary-union` unskip + `external-type-struct` with-builtins unskip
- Step 2: `array-builtin-methods` unskip + `closures` の I-011 filter 参照セマンティクス解消

**永続 skip (2件):** `callable-interface-generic-arity-mismatch` (意図的 error-case), `indexed-access-type` (マルチファイル用、別テストでカバー)

**残: 14 fixture / 14 イシュー**

#### 次の Step

```
Step 3 (Box::new + Option) ←── 次はここ
  ↓                                  Step 6 (string + intersection)
Step 4 (control flow + DU)           type-narrowing は Step 1 + 6 で完全解消
  ↓
Step 5 (type conversion + null)
  ↓
Step 7 (builtin impl)
```

**Step 3: クロージャ Box 化 + Option 暗黙返却** — Tier 2、レバレッジ最大（4 fixture）

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-020 | `needs_trait_box_coercion()` (`expressions/mod.rs:288`) | クロージャ式 → `Box<dyn Fn>` の `Box::new()` ラップ追加 |
| I-025 | control_flow 周辺 | `Option<T>` 戻り値の if 文に暗黙 `else { None }` を補完 |
| I-024 | `try_generate_narrowing_match()` (`control_flow.rs:250`) | `if (x)` where `x: Option<T>` → `if let Some(x) = x` の truthy パス対応 |

- unskip: `closures`（Step 2 と合わせて完全解消）, `keyword-types`, `void-type`
- 部分解消: `functions`（I-024 解消で残エラー消滅を要確認）

---

**Step 4: 制御フロー + DU** — Tier 2、独立した 2 修正

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-023 | `convert_try_stmt()` (`error_handling.rs:96-138`) | try/catch 両方に return がある場合の unreachable code 除去 |
| I-021 | `is_du_field_binding()` (`type_resolution.rs:209`) | match body でデストラクチャ変数を使うべき箇所が `event.x` のまま |

- unskip: `async-await`, `discriminated-union`
- `functions` 完全解消（Step 3 と合わせて）

---

**Step 5: 型変換 + null セマンティクス** — Tier 2、型変換パイプライン

| イシュー | 修正箇所 | 内容 |
|----------|---------|------|
| I-022 | `binary.rs:45-60` | ネスト `??` の中間結果型を `Option<T>` → `T` に unwrap |
| I-026 | 型 assertion 変換 | `as unknown as T` の中間 `unknown` を消去して直接キャスト |
| I-029 | null/any 変換 | `null as any` → `None` が `Box<dyn Trait>` 文脈で型不一致 |
| I-030 | `build_any_enum_variants()` (`any_narrowing.rs:85`) | any-narrowing enum の値代入で型強制 |

- unskip: `nullish-coalescing`, `type-assertion`, `trait-coercion`, `any-type-narrowing`

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
| closures | Step 3 | ~~Step 2 (I-011)~~ |
| keyword-types | Step 3 | — |
| void-type | Step 3 | — |
| functions | Step 4 | Step 3 (I-020, I-024) |
| async-await | Step 4 | — |
| discriminated-union | Step 4 | — |
| nullish-coalescing | Step 5 | — |
| type-assertion | Step 5 | — |
| trait-coercion | Step 5 | — |
| any-type-narrowing | Step 5 | — |
| string-methods | Step 6 | — |
| intersection-empty-object | Step 6 | — |
| type-narrowing | Step 6 | Step 1 (I-007) |
| instanceof-builtin | Step 7 | — |

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
