# Vec index read access の Option<T> context 対応 — silent semantic change 解消 (I-138)

## Background

TypeScript の `arr[i]` (Vec index read access) は Rust に変換される際、無条件に `.get(i).cloned().unwrap()` として出力される (`src/transformer/expressions/member_access.rs:284`)。このコードは `arr` の要素型 `T` の値を返す。

一方、`return arr[0]` が `Option<T>` を return type とする関数内で使われた場合、`convert_expr_with_expected` (`src/transformer/expressions/mod.rs:42-100`) が expected type `Option<T>` を検出して `Some(...)` で外側をラップする。結果として以下の silent semantic change が発生する:

TS 入力:
```ts
function first(items: string[]): string | undefined {
    return items[0];
}
```

現在の Rust 出力 (silent bug):
```rust
fn first(items: Vec<String>) -> Option<String> {
    Some(items.get(0).cloned().unwrap())  // ← 空 Vec で runtime panic
}
```

期待出力:
```rust
fn first(items: Vec<String>) -> Option<String> {
    items.get(0).cloned()  // ← 空 Vec で None
}
```

### 問題の性質

- **Rust compile は成功する** (型的には Option<String> を満たす)
- **Runtime 挙動が TS と異なる**: TS は空配列で undefined を返すのに対し、Rust は panic
- `.claude/rules/conversion-correctness-priority.md` の **Tier 1 (Silent semantic change)** — 最も危険な分類
- `.claude/rules/todo-prioritization.md` の **L1 (Reliability Foundation)** — すべての開発成果の信頼性を汚染する

### 発見経緯

I-040 (optional param wrap) の `/check_problem` Deep Review で、`tests/fixtures/functions.input.ts` の
`first` 関数変換結果の compile check 中に確認。I-024 (Option truthy narrowing) が I-040 fix
で部分解消された後、この独立した silent bug が `functions` fixture の compile を
阻害していることが判明。

### 関連既存 defect

- `.claude/rules/conversion-correctness-priority.md`: 本件は Tier 1 (最優先)
- I-012 (解消済): `find` の Option 二重ラップ — 同系統の invariant だが Rust API 側 (`Iterator::find` が Option を返す) で対処済
- `produces_option_result` (`src/transformer/expressions/mod.rs:364`): 既に `find` / `pop` を "Option-producing 式" として認識しラップをスキップする仕組み。本 PRD で Vec index の pattern を追加

## Goal

**完了状態**: TS `arr[i]` (Vec index read access) が `Option<T>` expected context で使われたとき、Rust は `Option<T>` を返す式 (`.get(i).cloned()`) を直接生成し、`.unwrap()` + `Some(...)` の二重化を行わない。

検証可能な基準:

1. 以下のすべての Option expected context で二重化が発生しない:
   - return context: `fn f() -> Option<T> { return arr[i]; }`
   - variable assignment: `let x: Option<T> = arr[i];`
   - function argument: `f(arr[i])` where `f` expects `Option<T>`
   - nullish coalescing: `arr[i] ?? default` の左辺
2. 非 Option context (`const x: T = arr[i]`) の挙動は現状維持 (`.unwrap()` で panic)
3. 空 Vec に対する runtime 動作が TS と一致 (tsx 実行と Rust 実行で stdout 一致)
4. `cargo test` 全 pass、clippy 0 warnings、fmt 0 diffs
5. `functions` fixture snapshot が expected 出力 (`items.get(0).cloned()`) に更新される

## Scope

### In Scope

- `convert_member_expr_inner` (`src/transformer/expressions/member_access.rs:249-285`) の
  computed index read path に expected type 判定を追加
- Option<T> expected context で `build_safe_index_expr(...)` (no unwrap) を emit
- 非 Option context では現行の `build_safe_index_expr_unwrapped(...)` を維持
- `produces_option_result` (`src/transformer/expressions/mod.rs:364-377`) に `.cloned()`
  pattern (specifically `<obj>.get(<idx>).cloned()`) を追加し、外側の Some ラップを回避
- 全 Option context (return / assignment / call arg / nullish coalescing 等) が単一の
  `expected_types` 経由で収束することの検証

### Out of Scope

- **HashMap 経由の computed access** (`map[key]`): 現状 `convert_index_to_usize` を
  string key にも適用する pre-existing 別 bug (**I-027**) が先にあり、`build_safe_index_expr_*`
  経由に到達していない。I-027 解消後に本 PRD と同じ invariant が自然に適用される
- **Array destructuring (`const [a, b] = arr`)** の context-aware 対応: 現在
  `try_convert_array_destructuring` は per-element type annotation を抽出しないため、
  独立した extraction 実装が必要。本 PRD と同一 invariant だが integration path が別。
  I-138 完了後に独立 PRD 起票推奨
- **Tuple destructuring (I-031)**: Rust Tuple は `.get()` を持たず `tuple.0` 構文が必要。
  根本原因が `build_safe_index_expr_unwrapped` の helper 選択ロジック (Tuple に不適) で、
  本 PRD の invariant (unwrap 抑制) と直交
- **`.first()` / `.last()` / `HashMap::get` の wrap-skip**: 返り値が `Option<&T>` (borrowed)
  で `Option<T>` expected と型構造が異なる。`produces_option_result` に追加**禁止**
  (silent type mismatch 導入の risk)
- **非 Option context の `.unwrap()` → `.expect("...")` 改善**: semantic correctness は
  `.unwrap()` で確保済 (TS で undefined を経由して後段 TypeError になるのと等価)。
  診断情報の質は直交関心事
- **I-025 (Option<T> 戻り値の暗黙 None 未返却)**: if/else 欠落の独立 bug
- **I-024 (Option truthy narrowing)**: I-040 で部分解消済。残課題は別対応

## Design

### Technical Approach

#### 核心: 単一の判定点による context-aware emission

`convert_member_expr_inner` の computed index read path (`member_access.rs:283-284`)
を以下のように変更する:

```rust
// Read access: safe bounds-checked indexing with context-aware unwrap
// arr[0] → arr.get(0).cloned()           (Option<T> expected context)
// arr[0] → arr.get(0).cloned().unwrap()  (T expected context)
let safe_index = convert_index_to_usize(index);
let expected = self
    .tctx
    .type_resolution
    .expected_type(Span::from_swc(member.span()));
if matches!(expected, Some(RustType::Option(_))) {
    return Ok(build_safe_index_expr(object, safe_index));
}
return Ok(build_safe_index_expr_unwrapped(object, safe_index));
```

判定は **単一の expected_type クエリ** に収束。全 context (return / assignment /
call arg / nullish coalescing / ternary) は TypeResolver が `expected_types[span]`
に Option<T> を propagate する仕組みが既に存在するため、この 1 箇所の判定で全
context をカバーする。

#### 副次修正: `produces_option_result` の pattern 拡張

`convert_member_expr_inner` が Option<T> 版を emit した後、`convert_expr_with_expected`
(`mod.rs:53-99`) の Option wrap logic が外側で `Some(...)` を付与してしまう。これを
防ぐには、`produces_option_result` が emit された `.get(i).cloned()` pattern を
Option-producing として認識する必要がある。

```rust
fn produces_option_result(expr: &Expr) -> bool {
    let Expr::MethodCall { object, method, args, .. } = expr else { return false; };
    match method.as_str() {
        "find" => args.len() == 1,
        "pop" => args.is_empty(),
        // `<obj>.get(<idx>).cloned()` — Vec index read in Option context (I-138)
        "cloned" => args.is_empty() && matches!(
            object.as_ref(),
            Expr::MethodCall { method: m, args: a, .. } if m == "get" && a.len() == 1
        ),
        _ => false,
    }
}
```

これにより外側の Some wrap は skip され、最終出力は `items.get(0).cloned()` の
単一 Option<T> 式になる。

#### 設計上の不変条件

- **単一判定点**: Option context の判定は `convert_member_expr_inner` 内の `expected_type`
  クエリ 1 箇所に限定。ad-hoc な各 context 分岐 (return / assignment / etc.) は禁止
- **idempotent な pattern 認識**: `produces_option_result` は構造的 pattern match
  のみで判定。型情報に依存しない (builtin 有無で挙動変化なし)
- **`.first()` / `.last()` / `HashMap::get` の除外維持**: `produces_option_result` 追加
  pattern は `.cloned()` 後の `.get()` **かつ** `args.len() == 1` に限定。
  `.first()` (0 引数) / `HashMap::get` (借用返し `Option<&V>`) は自然に除外される
- **Vec 以外への拡張は個別判定**: `String::get` (char slice) / `HashMap::get` など他の
  `.get()` を持つ型でも技術的には pattern match するが、これらは現状 `build_safe_index_expr`
  helper 経由で到達しない (Vec のみが helper を呼ぶ)。将来他の型が helper を使う場合、
  その時点で pattern の適用妥当性を再確認する

### Design Integrity Review

`.claude/rules/design-integrity.md` チェックリスト:

#### Higher-level consistency

- `convert_member_expr_inner` は member expression 変換の単一責務。expected type 判定を
  追加しても責務は変化しない (「read context に応じた Vec 要素取得式の emission」)
- `produces_option_result` は「生成 IR が Option<T> を返すか」の構造的判定が単一責務。
  Vec index の pattern 追加は既存責務の自然な拡張
- 他モジュールとの interface は不変 (関数シグネチャ変更なし)

#### DRY / Orthogonality / Cohesion

- **DRY**: 修正前は Option<T> context での「unwrap 抑制」判定が不在 → `convert_member_expr_inner`
  の 1 箇所に追加。他モジュールに同じ判定を作らない
- **Orthogonality**: expected_type の読み取りは TypeResolver の責務、emission は transformer
  の責務。本 PRD で境界を跨がない
- **Cohesion**: Vec index 関連のすべてのロジックが `member_access.rs` 内の
  `convert_member_expr_inner` と 2 つの helper (`build_safe_index_expr` /
  `build_safe_index_expr_unwrapped`) に集中。追加はこの cohesion を損なわない

#### Broken windows 発見 → 対応方針

1. **`build_safe_index_expr_unwrapped` の doc comment** (`member_access.rs:43-49`):
   `".unwrap()" bridges the type gap until proper Option<T> propagation is implemented`
   と明記されている → 本 PRD が "proper Option<T> propagation" の実装そのもの。
   実装完了後に doc comment を更新 (現状記述が過去形 interim 感の文面のため)
2. `produces_option_result` の doc comment (`mod.rs:350-363`): `find` / `pop` のみを
   想定した記述。Vec index pattern 追加に伴い doc を更新
3. I-138 と同系統の silent semantic change が他の場所 (e.g., builtin method の
   `.shift()` / `.at(n)` の Option 返し) に存在しないか、実装中の grep で網羅確認

### Impact Area

**修正対象ファイル**:
- `src/transformer/expressions/member_access.rs` — `convert_member_expr_inner` に expected_type 判定、および 2 helper の doc 更新
- `src/transformer/expressions/mod.rs` — `produces_option_result` に `.cloned()` pattern 追加、および doc 更新

**影響を受けるテスト / snapshot**:
- `tests/snapshots/integration_test__functions.snap` — `first` 関数の出力が
  `Some(items.get(0).cloned().unwrap())` → `items.get(0).cloned()` に更新
- 他 snapshot で同系統の二重化が検出された場合、手動判定の上 accept

**影響を受けない領域** (unchanged):
- 非 Option context の Vec index: `arr[i]` → `arr.get(i).cloned().unwrap()` (current)
- Tuple index: `tuple[0]` → `tuple.0` (既存の Tuple 分岐)
- Range slice: `arr[a..b]` → `arr[a..b]` (既存の Range 分岐)
- Write access: `arr[i] = x` → `arr[i] = x` (既存の for_write 分岐)

### Semantic Safety Analysis

本 PRD は Vec index read expression の出力式形態を context に応じて切り替える。
`.claude/rules/type-fallback-safety.md` の 3 段階分析を適用:

**Step 1: 導入される変換パターン**

| Source TS | Expected context | Current output | New output |
|---|---|---|---|
| `arr[i]` | Option<T> (return/assign/arg) | `Some(arr.get(i).cloned().unwrap())` | `arr.get(i).cloned()` |
| `arr[i]` | T (non-Option) | `arr.get(i).cloned().unwrap()` | `arr.get(i).cloned().unwrap()` (unchanged) |
| `arr[i]` | None (no context) | `arr.get(i).cloned().unwrap()` | `arr.get(i).cloned().unwrap()` (unchanged) |

**Step 2: 使用サイト分類**

1. **Option<T> expected context での `arr[i]`**:
   - 旧出力 `Some(...unwrap())`: 空 Vec で panic、TS は undefined を返す → **Silent semantic
     change (UNSAFE)**
   - 新出力 `arr.get(i).cloned()`: 空 Vec で None、TS の undefined と等価 → **Safe (TS 意味論一致)**
   - 結論: 旧が UNSAFE、新が安全化される方向の変化 → **改善**

2. **非 Option context での `arr[i]`**:
   - 出力不変 → Safe (identical behavior)

3. **型 resolve 失敗時 (expected = None)**:
   - 出力不変 (`.unwrap()` 維持) → Safe (identical behavior)

**Step 3: Verdict per pattern**

全パターンで **Safe**。silent semantic change は導入されず、逆に既存の Tier 1
silent change を解消する。`Option<Option<T>>` のような二重ラップも発生しない
(新出力は Option<T> 単一、外側の Some wrap が `produces_option_result` で skip)。

### Failure mode 検証

`convert_member_expr_inner` が Option<T> context で `build_safe_index_expr` を emit
するが、`produces_option_result` が新 pattern を認識しない場合:
- 外側の Some wrap が走り、`Some(arr.get(0).cloned())` (Option<Option<T>>) になる
- これは Rust compile error (expected Option<T>, found Option<Option<T>>)
- **コンパイルエラーで検出される = Safe**

`produces_option_result` が過剰に pattern match して非該当ケースも skip する場合:
- 例: `some_vec.get(i)` (未 cloned) や別 API の `.get()` → `.cloned()` パターン
- `.cloned()` が `Option<&T>` → `Option<T>` 変換しか行わない性質上、`.get()` の返り値が
  Option でなければ `.cloned()` は別 trait (IteratorCloneable) 経由となり型不一致で
  compile error
- 万一 skip した結果の式が Option<T> でない場合、外側の context が Option<T> を期待
  しているため type mismatch → compile error
- **コンパイルエラーで検出される = Safe**

## Task List

TDD 順序: 各タスクで RED (失敗する test) → GREEN (最小実装) → REFACTOR (整理)。

### T1: 失敗する integration test 追加 (Option return context)

- **Work**: `tests/integration_test.rs` に 4 件追加 (すべて現状では失敗):
  1. `test_vec_index_in_option_return_context_emits_get_cloned` — `function first(items: string[]): string | undefined { return items[0]; }` が `items.get(0).cloned()` を emit、`Some(...)` / `.unwrap()` を含まない
  2. `test_vec_index_in_option_assignment_context_emits_get_cloned` — `const x: string | undefined = arr[0];` の同様検証
  3. `test_vec_index_in_option_call_arg_context_emits_get_cloned` — `function consumer(s?: string): void {}` + `consumer(arr[0])` の call arg context
  4. `test_vec_index_in_non_option_context_keeps_unwrap` — `function first(items: string[]): string { return items[0]; }` が現行 `.unwrap()` を維持 (回帰防止)
- **Completion criteria**:
  - 4 件すべて add 時点で RED (1,2,3 は silent bug で FAILED、4 は PASS だが expected 値を lock-in)
  - assertion message が診断可能な情報量を含む (実際の出力を含む)
- **Depends on**: None
- **Prerequisites**: なし

### T2: 失敗する unit test 追加 (`produces_option_result` 拡張)

- **Work**: `src/transformer/expressions/mod.rs::produces_option_result_tests` に以下を追加:
  1. `test_get_cloned_is_option_producing` — `.get(idx).cloned()` pattern を `produces_option_result` が true と判定
  2. `test_cloned_without_get_not_option` — `.cloned()` 単体 (例: `x.cloned()`) は false
  3. `test_get_without_cloned_not_option` — `.get(idx)` 単体 (例: `map.get(key)`) は false
  4. `test_get_cloned_with_multi_args_not_option` — `.get(a, b).cloned()` (理論上ありえないが防御的) は false
- **Completion criteria**:
  - 1 が RED (現状 `produces_option_result` は `.cloned()` を認識しない)、2/3/4 は PASS
  - pattern match が構造的 (型情報非依存) であることが test 名で明示
- **Depends on**: None
- **Prerequisites**: なし

### T3: `produces_option_result` に `.cloned()` pattern を追加 (GREEN for T2-1)

- **Work**: `src/transformer/expressions/mod.rs:364-377` の `produces_option_result`
  に以下の arm を追加:
  ```rust
  "cloned" => args.is_empty() && matches!(
      object.as_ref(),
      Expr::MethodCall { method: m, args: a, .. } if m == "get" && a.len() == 1
  ),
  ```
  doc comment を更新して Vec index pattern に言及。
- **Completion criteria**:
  - T2-1 が GREEN
  - T2-2, T2-3, T2-4 は依然 PASS (既存動作維持)
  - `cargo test --lib produces_option_result` 全 pass
- **Depends on**: T2
- **Prerequisites**: なし

### T4: `convert_member_expr_inner` に expected_type 判定を追加 (GREEN for T1-1〜3)

- **Work**: `src/transformer/expressions/member_access.rs:283-284` を以下のように変更:
  ```rust
  let safe_index = convert_index_to_usize(index);
  let expected = self
      .tctx
      .type_resolution
      .expected_type(Span::from_swc(member.span()));
  if matches!(expected, Some(RustType::Option(_))) {
      return Ok(build_safe_index_expr(object, safe_index));
  }
  return Ok(build_safe_index_expr_unwrapped(object, safe_index));
  ```
  必要な import (`Span::from_swc`, `swc_common::Spanned`) を追加。
- **Completion criteria**:
  - T1-1, T1-2, T1-3 が GREEN
  - T1-4 (非 Option context) が依然 PASS (`.unwrap()` 維持)
  - `cargo test --lib` 全 pass
- **Depends on**: T3 (produces_option_result 拡張が先に入らないと、本 T4 の変更で
  Option<Option<T>> を生む)
- **Prerequisites**: T3 完了

### T5: E2E test 追加 (runtime semantic 一致検証)

- **Work**: `tests/e2e/scripts/vec_index_option_return.ts` を新規作成:
  ```ts
  function firstOrNone(items: string[]): string | undefined {
      return items[0];
  }
  function main(): void {
      console.log(firstOrNone(["a", "b"]));      // "a"
      console.log(firstOrNone([]));              // TS: undefined, Rust: None display
      console.log(firstOrNone(["solo"]));        // "solo"
  }
  ```
  `tests/e2e_test.rs` に `test_e2e_vec_index_option_return_ts_rust_stdout_match` を追加。
- **Completion criteria**:
  - tsx 実行と Rust 実行の stdout が一致 (empty case は両者 "undefined" / "None"
    の文字列表現が TS/Rust で異なる可能性があるため、空 case は `if (x !== undefined)`
    narrowing で "none" と出力するよう script を調整)
  - test pass
- **Depends on**: T4
- **Prerequisites**: T4 完了で Rust 側が panic しない

### T6: snapshot 整合性確認と update

- **Work**: `cargo test` で snapshot 変更を検出し、1 件ずつ手動判定:
  1. `tests/snapshots/integration_test__functions.snap` — `first` 関数の出力が
     `Some(items.get(0).cloned().unwrap())` → `items.get(0).cloned()` に変更 (expected)
  2. 他 snapshot で同系統の二重化解消が発生した場合、手動で「新出力が意味論的に
     正しいか」を判定
- **Completion criteria**:
  - 全 `.snap.new` について判定理由を PRD completion notes に記録
  - 正しい判定は `cargo insta accept`
  - 誤った snapshot 変更 (regression) があれば T4 に戻って再検討
- **Depends on**: T4, T5
- **Prerequisites**: なし

### T7: doc comment 更新と broken window cleanup

- **Work**:
  1. `build_safe_index_expr_unwrapped` の doc comment (`member_access.rs:43-49`) から
     `"until proper Option<T> propagation is implemented"` 文を削除し、「非 Option
     context で使用される」に記述更新
  2. `build_safe_index_expr` の doc を「Option<T> context で使用される」に更新
  3. `produces_option_result` の doc に Vec index pattern 追加の経緯を記述
  4. `convert_member_expr_inner` に「expected type による分岐の設計理由」を 2 行 doc コメント追加
- **Completion criteria**:
  - すべての doc が修正後の挙動を正確に説明
  - clippy の missing_docs warning が発生していない
- **Depends on**: T4
- **Prerequisites**: なし

### T8: 他の silent bug pattern の有無確認 (grep 網羅)

- **Work**: 実装中に以下を grep で網羅確認し、類似の silent semantic change が他
  モジュールに存在しないか検査:
  - `.unwrap()` 直後に `Some(...)` で外側ラップされるパターン (builtin method 変換系)
  - `build_safe_index_expr_unwrapped` の他 call site (現状 `destructuring.rs:213` のみ)
  - `produces_option_result` の不対応 Option-returning API pattern
- **Completion criteria**:
  - 発見された issue は本 PRD scope 内 (Vec index 起因) なら include、別系統は
    TODO 追記のみ (根拠: 本 PRD の invariant「Vec index read access の context-aware
    unwrap 抑制」)
  - 調査結果を PRD completion notes に記録
- **Depends on**: T4
- **Prerequisites**: なし

### T9: 最終品質ゲート

- **Work**: `cargo test` 全 pass / `cargo clippy --all-targets --all-features -- -D warnings`
  0 warnings / `cargo fmt --all --check` 0 diffs / `./scripts/hono-bench.sh` 実行
- **Completion criteria**:
  - 全ゲート clean
  - Hono bench 数値変動を記録 (本 PRD は pre-existing silent bug の顕在化を伴うため、
    error 数が増減する可能性あり — 増加なら新 compile error 露出、減少なら既存
    defect 解消)
- **Depends on**: T6, T7, T8
- **Prerequisites**: なし

## Test Plan

| テスト種別 | 対象 | 内容 |
|---|---|---|
| Unit | `produces_option_result` | `.get(i).cloned()` pattern 認識 (T2-1)、他 pattern 非認識 (T2-2, T2-3, T2-4) |
| Integration | Option return context | `fn f() -> Option<T> { return arr[i]; }` が `arr.get(i).cloned()` 直接使用 (T1-1) |
| Integration | Option assignment context | `let x: Option<T> = arr[i];` 同様 (T1-2) |
| Integration | Option call arg context | `f(arr[i])` where f expects Option<T> 同様 (T1-3) |
| Integration | Non-Option context (回帰防止) | `fn f() -> T { return arr[i]; }` が `.unwrap()` 維持 (T1-4) |
| E2E | runtime 一致 | empty Vec / 1 要素 / 複数要素で TS と Rust の stdout 一致 (T5) |
| Snapshot | `functions` fixture | `first` 関数の新出力 `items.get(0).cloned()` への更新承認 (T6) |

### Boundary analysis

- 空 Vec (`[]`): 期待 None
- 1 要素 Vec (`["a"]` index 0): 期待 Some("a")
- 1 要素 Vec (`["a"]` index 1 = OOB): 期待 None
- 複数要素 Vec (`["a", "b", "c"]` index 0, 1, 2): 期待 Some(...)
- 負 index (`["a"]` index -1): 期待 None (`.get(-1 as usize)` は巨大な値となり None)
- 変数 index (`arr[i]` where i は実行時決定): `convert_index_to_usize` で cast 処理

### Partition analysis

Context の partition:
- {return, variable assignment, call arg, nullish coalescing, ternary, conditional} × {Option<T>, T, 型不明}

本 PRD は Option<T> × 全 context を統一判定点でカバー。他は非変更 (回帰防止 test で lock-in)。

### Branch coverage (C1)

`convert_member_expr_inner` の新 branch:
1. `matches!(expected, Some(RustType::Option(_)))` true → `build_safe_index_expr` emit
2. 同 false → `build_safe_index_expr_unwrapped` emit (existing)

両 branch を T1-1 (true) と T1-4 (false) でカバー。

`produces_option_result` の新 arm:
1. `method == "cloned" && args.is_empty() && object is .get(idx)` → true
2. その他: false

T2-1 (true)、T2-2/T2-3/T2-4 (false) でカバー。

## Completion Criteria

- [ ] T1 の integration test 4 件が RED → GREEN (T4 完了後)
- [ ] T2 の unit test 4 件が RED (T2-1 のみ) → GREEN (T3 完了後)
- [ ] T5 の E2E test が pass (T4 完了後)
- [ ] `tests/snapshots/integration_test__functions.snap` の更新が正しく、他 snapshot
      regression なし
- [ ] `build_safe_index_expr_unwrapped` / `build_safe_index_expr` / `produces_option_result` /
      `convert_member_expr_inner` の doc comment が修正後の挙動を正確に説明
- [ ] T8 の grep 網羅確認完了、発見 issue は scope 内 fix または TODO 追記
- [ ] `cargo test` 全 pass (lib / integration / compile / E2E)
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
- [ ] `cargo fmt --all --check` 0 diffs
- [ ] `./scripts/hono-bench.sh` 実行結果を PRD notes に記録
- [ ] plan.md の「次のタスク」セクションから本 PRD 参照を削除
- [ ] TODO から I-138 を削除

### Impact 検証 (3 代表 instance のコードパス追跡)

ラベルベース見積を禁じるため、以下 3 instance で修正適用路を確認:

1. **`return items[0]` (return context, `Option<String>` return type)**:
   - TypeResolver: `visit_return_stmt` が `expected_types[return_expr.span] = Option<String>` 設定
   - transformer: `convert_return_stmt` → `convert_expr_with_expected` で expected = Option<String>
   - → `convert_member_expr` (member.obj=items, prop=Computed(0))
   - T4 fix: `expected_type` クエリで Option 検出 → `build_safe_index_expr` emit (no unwrap)
   - 外側 `convert_expr_with_expected`: `produces_option_result` が `.cloned()` pattern match → Some wrap skip
   - 生成: `items.get(0).cloned()` ✓

2. **`const x: string | undefined = arr[0];` (variable assignment context)**:
   - TypeResolver: `visit_var_decl` が `expected_types[init_expr.span] = Option<String>` 設定
   - transformer: `convert_var_decl` → `convert_expr_with_expected` で expected = Option<String>
   - 同じ路で `items.get(0).cloned()` emit ✓

3. **`f(arr[0])` where `f` expects `string | undefined` (call arg context)**:
   - TypeResolver: `set_call_arg_expected_types` が `expected_types[arg_expr.span] = Option<String>` 設定
   - transformer: `convert_call_args_with_types` → `convert_expr_with_expected` で expected = Option<String>
   - 同じ路で `arr.get(0).cloned()` emit ✓

3 つの代表 instance すべてで T4 + T3 の fix がこの PRD の invariant を満たすことを
確認。expected_type の propagation は全 context で TypeResolver が既に行っている
ため、transformer 側の単一判定点で全 context を統一カバー可能。

## Notes

- PRD 開始時点の Hono bench: clean 71.5% (113/158) / errors 59
- 本 PRD 完了後の bench 変動は完了時に追記
- 本 PRD は I-040 完了直後の `/check_problem` Deep Review で発見された Tier 1 silent
  semantic bug。L1 (Reliability Foundation) 準拠で Phase A Step 3 以降の他作業に先行
  する位置付け
