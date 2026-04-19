# I-142 Step 4 follow-up — 引継ぎドキュメント

## 引継ぎの経緯

I-142 (`??=` NullishAssign Ident LHS structural rewrite) PRD は Step 1 / Step 2 /
Step 3 を実装完了し、その時点で **PRD を完了 (closed) 扱い** とした。

Step 3 完了後の `/check_job` 敵対的第三者レビュー (2026-04-15) で検出された未解決
defect 群 (C-1 〜 C-9 + D-1) は、本引継ぎドキュメントに記録する。

これらは I-142 PRD の枠組みでは解消せず、**I-SDCDF (Spec-Driven Conversion
Development Framework)** の spec-first workflow を適用して再着手する。SDCDF は
2026-04-17 に Phase 1-4 完了・Pilot 成功 (Spec gap = 0) で正式導入済み。
Rule: `.claude/rules/spec-first-prd.md`。

## 引継ぎ項目の性質

以下の項目は「I-142 実装内の既知 defect」として accept された状態で close された。
新 framework の pilot 完了後、以下いずれかの方法で解消する:

1. **Pilot 対象に I-142 を含める場合**: 新 framework の spec-first workflow で
   問題空間を再導出し、C-1 〜 C-9 + D-1 が pilot の定義通りに検出・解消される
   ことを確認。
2. **Pilot 対象が別 PRD の場合**: 新 framework 定着後、本引継ぎ項目を個別の
   sub-PRD (`backlog/I-142-step4-c1-*.md` など) として再起票し、新 workflow で
   処理。

## 現状コードベースの既知 compromise

以下は I-142 close 時点で code に存在する compromise:

### コード側
- `src/transformer/statements/nullish_assign.rs::expr_has_reset` の AssignExpr arm
  が op 種別を区別せず、compound arithmetic (`x += 1`) / UpdateExpr (`x++`) を
  narrowing-reset と false-positive surface (C-1)。
- `src/transformer/statements/nullish_assign.rs::pick_strategy` が `Option<Any>` を
  ShadowLet にマップ (C-7): inner Value coerce が I-050 依存のため実質 silent
  compile error 潜在。
- `src/transformer/expressions/assignments.rs` Identity arm の `.clone()` emission
  が INTERIM comment のみで、`ideal-implementation-primacy.md` Interim Patch
  条件 #4 (removal criterion) が TODO I-048 に未記載 (C-8)。

### テスト側
- `cell14_closure_body_reassign_does_not_surface_reset` が silent compile error を
  lock-in している疑い (C-2)。compile 検証未実施。
- scanner 再帰 branch の test coverage が 22+ variant 中 4 variant のみ (C-3)。
- 非-reset ケース (compound arith / UpdateExpr / for-of 新規 binding / inner fn /
  class method) の明示 lock-in test 欠落 (C-4)。
- D-2 Class D (transparent TS wrapper) の 7 variant 中 3 variant のみテスト (C-5)。
- `d2_seq_rhs_surfaces_unsupported` の assertion が weak、Seq 固有エラーを検証
  していない (C-6)。

### 観測事実 (未特定)
- Hono bench error 62 → 63 (+1 OBJECT_LITERAL_NO_TYPE `utils/concurrent.ts:12`) の
  根本原因が assumption のまま記録 (C-9)。INV-Step4-2 bisection が未実施。

### 設計
- `pre_check_narrowing_reset` の call site が 6 箇所に分散 (D-1)。DRY 違反 +
  新規 iteration site 追加時の silent regression risk。

## 新 framework 適用時の着手順序

新 framework pilot が I-142 を対象とする場合、以下順序を推奨:

1. **INV-Step4-1 / INV-Step4-2 を新 framework の「外部 oracle 観測」phase で実施**
   - 引継ぎ item: INV-Step4-1 (closure body reassign の Rust compile 可否) は
     `cargo check` での実測が未実施。
   - INV-Step4-2 (+1 OBJECT_LITERAL_NO_TYPE bisection) は commit 単位の bench
     比較が未実施。
   - 両 INV とも新 framework の「tsc / cargo を external oracle として先行観測」
     原則に合致するので、pilot の先行調査 phase として実施する。

2. **Problem Space matrix を grammar-derived 形式で再導出**
   - SWC AST grammar + RustType variants + emission contexts の Cartesian product を
     形式的に enumerate (新 framework 提供予定の reference 使用)。
   - Step 3 時点の matrix (14 cell + RHS 4 class) は intuition-driven。新 matrix
     は grammar 由来で C-3 / C-4 / C-5 / C-7 の未カバー cell を含む完全列挙になる
     べき。

3. **Per-cell E2E fixture を先行書き下し (red-green-refactor で red)**
   - `tests/e2e/scripts/nullish_assign/<cell-id>.ts` の形で cell 単位の TS fixture。
   - tsc / tsx で runtime stdout を観測・記録。
   - ts_to_rs 変換結果の Rust を `cargo run` して stdout 一致を assert。
   - 全 cell で red 状態から開始して spec の完全性を検証。

4. **Spec-stage 敵対的レビュー実施** (実装着手前)
   - Matrix 完全性 / ideal output の正当性 / NA justification を spec 段階で
     review。実装レビューで発見される defect (Step 3 の D-1〜D-7 / Step 4 の
     C-1〜C-9) の多くは spec 段階で検出可能だったはず。

5. **実装着手**
   - C-1 / C-2 / C-3 / C-4 / C-5 / C-6 / C-7 / C-8 / C-9 / D-1 を新 matrix / spec
     に沿って解消。

## Detailed defect inventory

以下は I-142 PRD Step 4 section に記録されていた詳細 defect inventory。
新 framework 適用時の reference として保持する。

### Step 4 の位置付け

Step 3 完了報告後の `/check_job` 敵対的第三者レビュー (2026-04-15) で、以下の
**spec-driven な未解決 defect / matrix gap / investigation debt** が検出された。
いずれも `ideal-implementation-primacy.md` / `problem-space-analysis.md` /
`prd-completion.md` / `todo-entry-standards.md` 準拠上、interim として許容
できず **structural fix または明示的 lock-in** が必要。

本 PRD は Step 4 完了までは **完成扱いとしない** (`prd-completion.md`)。Step 3
は「reported defect (D-1〜D-7) を scope 通りに実装した」段階であり、その実装
自体に内在する compromise と未 enumerate cell は Step 4 で解消する。

実装はすべて **次セッション** で行い、本 section は「現状の defect inventory」
として機能する。Step 4 着手時は C-1 / C-2 / C-3 / C-4 を最優先 (correctness
critical)、続けて C-5 / C-6 / C-7 (matrix / test quality)、C-8 / C-9 / D-1
(documentation / design improvement) の順で取り組む。

### C-1 (🔴 correctness critical) narrowing-reset scanner が compound ops / UpdateExpr を false-positive surface

**症状 (再現 TS)**:
```ts
function valid_narrow_maintained(x: number | null): number {
    x ??= 0;
    x += 1;        // TS: narrow 維持 (x: number のまま)、正当な pattern
    return x;
}
function valid_update(x: number | null): number {
    x ??= 0;
    x++;           // TS: narrow 維持
    return x;
}
```

**現状挙動**: 両 fixture とも `UnsupportedSyntaxError("nullish-assign with
narrowing-reset (I-144)")` を surface。**しかし TS は narrow を維持し、生成 Rust
は `mark_mutated_vars` による `let mut` 格上げで compile 成功する正当な case**。
変換品質を不必要に降格させている。

**Root cause** (`src/transformer/statements/nullish_assign.rs` 現状):

`expr_has_reset` の該当 arm:
```rust
ast::Expr::Assign(assign) => {
    let lhs_hit = matches!(
        &assign.left,
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(id))
            if id.id.sym.as_ref() == ident
    );
    lhs_hit || expr_has_reset(&assign.right, ident)
}
ast::Expr::Update(up) => {
    matches!(up.arg.as_ref(), ast::Expr::Ident(id) if id.sym.as_ref() == ident)
}
```

op 種別を区別せず全 AssignExpr (および全 UpdateExpr) を reset 扱い。

**TS 意味論 (AssignOp 別)**:

| AssignOp | narrow 動作 | reset 扱い |
|---------|------------|----------|
| `=` (Assign) | RHS 型で narrow 再計算 (re-widen 可) | ✓ reset |
| `??=` (NullishAssign) | RHS 型で narrow 再計算 (shadow 後の nested ??= は silent compile error) | ✓ reset |
| `\|\|=` (OrAssign) | RHS 型で narrow 再計算 (falsy 分岐で reassign) | ✓ reset |
| `&&=` (AndAssign) | RHS 型で narrow 再計算 (truthy 分岐で reassign) | ✓ reset |
| `+=` (AddAssign) | narrow 型上の加算、**narrow 維持** | ✗ non-reset |
| `-=` `*=` `/=` `%=` `**=` | narrow 型上の算術、**narrow 維持** | ✗ non-reset |
| `<<=` `>>=` `>>>=` `&=` `\|=` `^=` | narrow 型上の bit 演算、**narrow 維持** | ✗ non-reset |

**UpdateExpr (`x++` / `x--` / `++x` / `--x`)**: numeric 型上で閉じた演算。narrow 維持。non-reset。

**Fix 方針 (structural)**:

`expr_has_reset` の AssignExpr arm を op 種別で分岐:
```rust
ast::Expr::Assign(assign) => {
    let op_rebinds = matches!(
        assign.op,
        ast::AssignOp::Assign
            | ast::AssignOp::NullishAssign
            | ast::AssignOp::OrAssign
            | ast::AssignOp::AndAssign
    );
    let lhs_hit = op_rebinds
        && matches!(
            &assign.left,
            ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(id))
                if id.id.sym.as_ref() == ident
        );
    // RHS は op に関わらず scan (nested Assign を探すため)
    lhs_hit || expr_has_reset(&assign.right, ident)
}
ast::Expr::Update(_up) => false,  // narrow 維持、non-reset
```

AssignExpr の RHS scan は op に関わらず継続 — `x += (y = null)` の nested Assign
(y への再代入) を検出する必要があるため。LHS match のみが op-conditional。

**成功条件**:

- `cell14_narrowing_reset_compound_add_assign_does_not_surface` が `x ??= 0; x += 1;`
  で UnsupportedSyntaxError を surface しないことを assert。
- 全 compound op variant (`-=` `*=` `/=` `%=` `**=` `<<=` `>>=` `>>>=` `&=` `|=` `^=`)
  に対して個別 test (C-4 統合)。
- `cell14_narrowing_reset_update_expr_does_not_surface` (`x ??= 0; x++;` など) が
  non-surface を assert。
- 既存の reset test (linear `x = null`、if body reset、for-of body 内 `x = v`) は
  引き続き surface を lock-in。
- Hono bench で compound op 使用箇所が新たに unsupported になっていないことを確認
  (差分記録)。

### C-2 (🔴 correctness critical) `cell14_closure_body_reassign_does_not_surface_reset` が silent compile error を lock-in している疑い

**問題 test** (`src/transformer/expressions/tests/nullish_assign.rs::cell14_closure_body_reassign_does_not_surface_reset`):

```ts
function closureOk(x: number | null): number {
    x ??= 0;
    const reassign = () => { x = 1; };
    reassign();
    return x;
}
```

```rust
let (rust, unsupported) = crate::transpile_collecting(src).unwrap();
assert!(!unsupported.iter().any(|u| u.kind.contains("narrowing-reset")));
assert!(rust.contains("let x = x.unwrap_or(0.0)") || rust.contains("x.unwrap_or(0.0)"));
```

test が assert するのは:
1. narrowing-reset surface **されない** こと
2. shadow-let (`let x = x.unwrap_or(0.0)`) が emit **されている** こと

**未検証**: 生成された Rust が実際に `cargo check` で compile するか。

**理論的懸念の詳細**:

TS の control-flow analysis は closure 越しの mutation を narrowing 計算で **無視**
する (INV-Step3-1 case 03/05)。scanner の closure 非降下 policy はこの TS 仕様に
依拠。

しかし **Rust の lexical scope rule では shadow-let `let x` は closure body 内でも
可視**。以下の compile error が発生する可能性:

```rust
fn closureOk(x: Option<f64>) -> f64 {
    let mut x = x;
    let x = x.unwrap_or(0.0);  // outer shadow (immutable, f64)
    let reassign = || { x = 1.0; };  // ← E0594: cannot assign to `x`, which is not declared as mutable
    reassign();
    x
}
```

`mark_mutated_vars` は closure body 内の assign を検出して outer `let x` を
`let mut x` に格上げできるか? 仮に格上げ成功しても:

```rust
let mut x = x.unwrap_or(0.0);
let reassign = || { x = 1.0; };  // FnMut: mut-borrows x
reassign();                       // reassign を call するには let mut reassign or &mut reassign 必要
x                                 // ← E0502: cannot use `x` while it is borrowed by `reassign`
```

**これは D-1 が解消しようとした pattern (silent compile error lock-in) を test 自身
で再導入している疑いがある**。INV-Step3-1 の「TS CFG 境界」と Rust 生成側の
「lexical scope 境界」の乖離が未解決。

**必須調査 [INV-Step4-1]**: closure body reassign の Rust compile 可否実測

- **Known**:
  - TS CFG は closure 越し mutation を narrowing で無視 (INV-Step3-1 case 03/05)。
  - scanner は closure 非降下 policy (`expr_has_reset` の `ast::Expr::Arrow | Fn |
    Class => false`)。
  - `mark_mutated_vars` は closure 内の ident assign を `let mut` 格上げに反映
    するか未確認。
- **Unknown**: 上記 TS を ts_to_rs 変換した Rust output が `cargo check` で通る
  か否か、通らない場合の diagnostic 種別。
- **Investigation method**:
  1. 該当 TS を `transpile_collecting` 経由で変換し `/tmp/i142-step4-inv1/src/main.rs`
     に書込。
  2. 最小 `Cargo.toml` を準備し `cargo check --manifest-path /tmp/i142-step4-inv1/Cargo.toml`
     を実行。
  3. 結果 (成否 + full diagnostics) を `report/i142-step4-inv1-closure-compile.md`
     に記録。
  4. 追加で tsx / Rust 実行結果の runtime stdout 一致を検証。
  5. closure 内で `x ??= ...` / `x = ...` / `x += 1` など variant を switch して
     compile 結果の差分も記録。
- **Impact**: C-2 の fix 方針 (scanner policy 変更要否 / emission 側修正要否) を
  確定する最重要 gate。
- **Resolution target**: Step 4 実装着手前 (他 C-1 / C-3 / C-4 は本 INV 結果と
  独立に進行可能だが、C-2 自身の fix 着手には本 INV 完了が必須)。

**仮説別 Fix 方針**:

**仮説 A — Rust output が compile する場合**:
- 現状の scanner policy (closure 非降下) が正しい。
- test を compile 検証込みに強化:
  - `tests/compile-check/src/lib.rs` 系 fixture に closure reassign case を追加。
  - または `tests/e2e/scripts/nullish_assign_closure.ts` を追加して実 Rust compile
    + runtime 一致を lock-in。
- C-2 完了。

**仮説 B — Rust output が compile しない場合**:
- 現行 test は silent compile error を lock-in している。D-1 の本旨違反。
- scanner policy の見直しが必要。Option:
  - **(B1) closure body 内の outer ident 再代入も reset 扱い** (保守的、
    over-surfacing)。TS 境界ではなく Rust lexical scope 境界で scan する。
    `expr_has_reset` の Arrow / Fn / Class arm を descend 経路に変更、ただし
    function parameters / 内側で再宣言された同名 ident は除外。
  - **(B2) emission 側の修正**: closure 内 outer ident assign を検出したら
    `let x = x.unwrap_or(...)` ではなく `let mut x: Option<T> = x; x.get_or_insert_with(|| ...)`
    経路に emit し、closure capture の型を `Option<T>` に保つ。I-144 structural
    fix の前倒し。
  - **(B3) 全面的な UnsupportedSyntaxError surface** (最も保守的)。I-144 完了
    まで「closure body 内の outer ident mutation + outer ??=」パターンを全て
    surface 化。
- 選択は INV-Step4-1 の diagnostic 種別を見て判定。

**成功条件**:
- INV-Step4-1 調査が完了し、実測結果が `report/` に記録。
- 仮説 A: `cell14_closure_body_reassign_*` test が compile 検証込みに書き換え。
- 仮説 B: scanner 修正 + regression test (closure body 内の各 assign / update
  variant) + Hono bench 差分記録。
- どちらの場合も「silent compile error が lock-in されていない」ことを第三者
  レビューで確認。

### C-3 (🔴 test coverage) narrowing-reset scanner の再帰 branch 全 variant に test 欠落

**問題**: `problem-space-analysis.md` の「各 scan 境界 cell に個別 lock-in test」
要件に対し、現状 test は以下のみ:

- ✓ linear stmt (`x ??= 0; x = null;`) — `cell14_narrowing_reset_surfaces_unsupported_blocked_by_i144`
- ✓ if consequent body — `cell14_narrowing_reset_detects_inner_if_block`
- ✓ for-of loop body — `cell14_narrowing_reset_detects_loop_body_reassign`
- ✓ closure body (non-surface) — `cell14_closure_body_reassign_does_not_surface_reset` (※ C-2 対象)

**未カバー (lock-in test 不在)**:

| Scan 境界 | test 欠落内容 | scanner code path 存在? |
|---|---|---|
| if alt body | `if (c) {} else { x = null; }` | ✓ (`if_stmt.alt`) |
| while body | `while (c) { x = null; }` | ✓ (`Stmt::While.body`) |
| do-while body | `do { x = null; } while (c);` | ✓ (`Stmt::DoWhile.body`) |
| while condition | `x ??= 0; while (x = null, false) {}` | ✓ (`Stmt::While.test`) |
| for 3-clause init | `for (x = null; ... ; ...) {}` | ✓ (`Stmt::For.init`) |
| for 3-clause test | `for (...; x = null; ...) {}` | ✓ (`Stmt::For.test`) |
| for 3-clause update | `for (...; ...; x = null) {}` | ✓ (`Stmt::For.update`) |
| for 3-clause body | `for (...; ...; ...) { x = null; }` | ✓ (`Stmt::For.body`) |
| for-in body | `for (const k in obj) { x = null; }` | ✓ (`Stmt::ForIn.body`) |
| for-of head rebind | `for (x of arr) { ... }` (outer x 再代入) | ✓ (`for_head_binds_ident`) |
| switch discriminant | `switch (x = null) { ... }` | ✓ (`Stmt::Switch.discriminant`) |
| switch case test | `switch (k) { case (x = null): ... }` | ✓ (`SwitchCase.test`) |
| switch case cons | `switch (k) { case 1: x = null; break; }` | ✓ (`SwitchCase.cons`) |
| try block | `try { x = null; } catch {}` | ✓ (`Stmt::Try.block`) |
| catch body | `try {} catch { x = null; }` | ✓ (`CatchClause.body`) |
| finally body | `try {} finally { x = null; }` | ✓ (`Stmt::Try.finalizer`) |
| labeled body | `L: { x = null; }` | ✓ (`Stmt::Labeled.body`) |
| nested block | `{ x = null; }` | ✓ (`Stmt::Block.stmts`) |
| nested `??=` in inner block | `x ??= 0; if (c) { x ??= 5; }` | ✓ (via AssignExpr ??=) |
| vardecl RHS | `x ??= 0; const y = (x = null);` | ✓ (`Decl::Var init`) |
| return RHS | `x ??= 0; return (x = null);` | ✓ (`Stmt::Return.arg`) |
| throw arg | `x ??= 0; throw (x = null);` | ✓ (`Stmt::Throw.arg`) |
| call args | `x ??= 0; foo(x = null);` | ✓ (`Call.args`) |
| ternary branches | `x ??= 0; c ? (x = null) : 0;` | ✓ (`Cond.cons/alt`) |

**合計 22+ 未カバー cell**。

**Fix 方針**: 全 variant に parameterized lock-in test を追加。test 命名:
- surface 検出系: `cell14_narrowing_reset_detects_<branch>_reset`
- non-surface 期待系: `cell14_narrowing_reset_does_not_surface_for_<context>`

**成功条件**:
- 全 scan 境界 variant に対応する test が存在。
- 各 variant で適切な surface / non-surface 挙動を assert。
- test 命名規則が統一され cell 対応が明確。

### C-4 (🔴 test coverage) 非-reset ケースの明示 lock-in test 不在

C-1 fix 後、以下 case が **narrowing-reset として surface されない** ことを
lock-in しないと silent regression 検出不能 (conservative 方向の後戻り検出不能):

**Compound arithmetic**:
- `x ??= 0; x += 1; return x;`
- `x ??= 0; x -= 1;`
- `x ??= 0; x *= 2;`
- `x ??= 0; x /= 2;`
- `x ??= 0; x %= 3;`
- `x ??= 0; x **= 2;`

**Bitwise / shift**:
- `x ??= 0; x &= 1;`
- `x ??= 0; x |= 1;`
- `x ??= 0; x ^= 1;`
- `x ??= 0; x <<= 1;`
- `x ??= 0; x >>= 1;`
- `x ??= 0; x >>>= 1;`

**UpdateExpr**:
- `x ??= 0; x++;`
- `x ??= 0; x--;`
- `x ??= 0; ++x;`
- `x ??= 0; --x;`

**for-of 新規 binding (outer ident 非関与)**:
- `x ??= 0; for (const v of arr) { /* use v, not x */ }`
- `x ??= 0; for (let i = 0; i < 10; i++) { /* use i, not x */ }`

**scope-boundary non-reset**:
- `x ??= 0; function inner() { x = null; }` (inner fn decl body、closure 同様の scope)
- `x ??= 0; class C { m() { x = null; } }` (method body、closure 同様の scope)

**Fix 方針**: parameterized test 追加:
```rust
#[test]
fn cell14_compound_assign_does_not_surface_reset() {
    for op in ["+=", "-=", "*=", "/=", "%=", "**=", "&=", "|=", "^=", "<<=", ">>=", ">>>="] {
        let src = format!(r#"
            function f(x: number | null): number {{
                x ??= 0;
                x {op} 1;
                return x;
            }}
        "#);
        let f = TctxFixture::from_source(&src);
        let (_items, unsupported) = f.transform_collecting(&src);
        assert!(
            !unsupported.iter().any(|u| u.kind.contains("narrowing-reset")),
            "compound op {op} must not surface narrowing-reset, got: {:?}",
            unsupported
        );
    }
}
```

類似 test を UpdateExpr / for-of 新規 binding / inner fn / class method に対して
追加。

**成功条件**: 上記 22+ variant 全てに対する non-reset lock-in test が存在し、
C-1 fix 後に全 pass。

### C-5 (🟡 test coverage) D-2 Class D 残 4 variant の parameterized 網羅不足

**INV-Step3-2 の設計**:
> Class D (transparent TS wrapper): Paren, TsAs, TsTypeAssertion, TsSatisfies,
> TsConstAssertion, TsInstantiation, TsNonNull — inner Expr に再帰し、上記 A/B/C
> で再判定

**現状の実装 test** (`src/transformer/expressions/tests/nullish_assign.rs`):
- ✓ `d2_class_d_ts_as_rhs_stmt_peeks_through` (TsAs)
- ✓ `d2_class_d_ts_non_null_rhs_stmt_peeks_through` (TsNonNull)
- ✓ `d2_class_d_paren_rhs_stmt_peeks_through` (Paren)

**未 test**:
- `TsTypeAssertion` (`x ??= <string>d`) — 旧 syntax、現行 TS で稀だが JSX 無し環境では
  accept される
- `TsSatisfies` (`x ??= (d satisfies string)`) — TS 4.9+
- `TsConstAssertion` (`x ??= (d as const)`) — tuple / literal narrow 用
- `TsInstantiation` (`x ??= f<string>`) — TS 4.7+

INV-Step3-2 で Class D を 7 variant と明記したのに、test は 3/7 のみ。問題空間
enumerate 片手落ち。

**Fix 方針**: parameterized test で 7 variant 全てを 1 回の enumerate で測定。

```rust
#[test]
fn d2_class_d_all_transparent_wrappers_peek_through() {
    let rhs_variants = [
        ("Paren", r#"(d)"#),
        ("TsAs", r#"(d as string)"#),
        ("TsTypeAssertion", r#"<string>d"#),
        ("TsSatisfies", r#"(d satisfies string)"#),
        ("TsConstAssertion", r#"(d as const)"#),
        ("TsNonNull", r#"d!"#),
        ("TsInstantiation", r#"identity<string>"#), // 別途 identity 定義必要
    ];
    for (name, rhs) in rhs_variants {
        // Stmt context + Expr context 両方検証
        // ...
    }
}
```

**成功条件**: 全 7 variant に対して peek-through 挙動が assert され、inner Expr
への再帰が確認される。

### C-6 (🟡 test quality) Seq RHS test の assertion が weak

**現状 test** (`d2_seq_rhs_surfaces_unsupported`):
```rust
let result = crate::transpile_collecting(src);
match result {
    Err(_) => { /* direct error path — acceptable */ }
    Ok((_rust, unsupported)) => {
        assert!(!unsupported.is_empty(),
            "Seq RHS must produce at least one UnsupportedSyntaxError");
    }
}
```

**問題**: Err / Ok どちらの経路でも 「Seq 由来のエラー」 であることを assert
していない。

- Err 経路: 任意の anyhow error を accept。例えば TypeResolver の別エラーで
  transpile 自体が fail しても test pass。
- Ok 経路: 任意の UnsupportedSyntaxError が 1 件でもあれば pass。他の silent
  bug が混入しても検出されない。

**Fix 方針**:
```rust
#[test]
fn d2_seq_rhs_surfaces_unsupported() {
    let src = r#"/* ... */"#;
    let result = crate::transpile_collecting(src);
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("Seq") || msg.contains("unsupported expression"),
                "Seq RHS must fail with Seq-specific error, got: {msg}"
            );
        }
        Ok((_rust, unsupported)) => {
            assert!(
                unsupported.iter().any(|u|
                    u.kind.contains("Seq") || u.kind.contains("unsupported expression")
                ),
                "Seq RHS must produce a Seq-specific UnsupportedSyntaxError (not unrelated error), got: {:?}",
                unsupported
            );
        }
    }
}
```

**成功条件**: Seq RHS test が Seq 固有のエラー message を assert、他の unrelated
エラーでは pass しない。他の D-2 未カバー RHS class (yield / await / throw 等) にも
同様の strict assertion を追加。

### C-7 (🟡 matrix gap) `Option<Any>` LHS cell が problem space に未 enumerate

**問題空間の未 enumerate cell**:
TS の `x: any | null` / `x: unknown | undefined` は IR 上
`RustType::Option(Box::new(RustType::Any))`。`pick_strategy` の現行 match:

```rust
match lhs_type {
    RustType::Option(_) => ShadowLet,  // ← Option<Any> もここで matches
    RustType::Any => BlockedByI050,
    // ...
}
```

**silent semantic / compile bug potential**:

1. `x: any | null`, `x ??= "str"`:
   - `pick_strategy` → ShadowLet
   - emission: `let x = x.unwrap_or_else(|| "str".to_string());`
   - Rust: `x: Option<serde_json::Value>` に対して `unwrap_or_else(|| -> String)`
     → 型不一致 compile error (silent、I-050 の構造的 coerce 未実装で検出不能)。

2. `x: any | null`, return-expr `return (x ??= "str")`:
   - emission: `*x.get_or_insert_with(|| "str".to_string())`
   - 同様に型不一致 compile error。

**現行 Cell #5 / #9 (Any LHS、非 Option) との対称性**:
- Cell #5 / #9 は `x: any` (pure Any) → BlockedByI050 で surface 済。
- `Option<Any>` は「outer Option / inner Any」で異なる TS 型意味論。Cell #5 / #9
  とは別 cell として matrix に加える必要。

**追加 cell 案**:

| # | LHS 型 | Context | inner | Ideal 出力 | 所属 PRD | 現状 | 判定 |
|---|--------|---------|-------|----------|----------|------|------|
| 15 | `Option<Any>` | Stmt | Any (!Copy) | `if x.is_null() { x = Value::from(d); }` 相当 | **I-050 依存** | ShadowLet silent fail | ✗→⏸ |
| 16 | `Option<Any>` | Expr | Any (!Copy) | `{ if x.is_null() { x = Value::from(d); } x.clone() }` 相当 | **I-050 依存** | ShadowLet silent fail | ✗→⏸ |

**Fix 方針 (Step 4 scope)**:

`pick_strategy` に明示的な `Option<Any>` 分岐を追加し `BlockedByI050` を返す:
```rust
pub(crate) fn pick_strategy(lhs_type: &RustType) -> NullishAssignStrategy {
    use NullishAssignStrategy::{BlockedByI050, Identity, ShadowLet};
    match lhs_type {
        // Option<Any>: inner Value coerce が I-050 依存。pure Any cell (#5/#9) と
        // 対称に BlockedByI050 surface。
        RustType::Option(inner) if matches!(inner.as_ref(), RustType::Any) => BlockedByI050,
        RustType::Option(_) => ShadowLet,
        RustType::Any => BlockedByI050,
        // 以下略
    }
}
```

table test 追加: `pick_strategy_option_any_maps_to_blocked_by_i050`。

lock-in test (Cell #15 / #16) 追加:
- `cell15_option_any_lhs_stmt_is_blocked_by_i050`
- `cell16_option_any_lhs_expr_is_blocked_by_i050`

両 test は `any | null` 型の TS fixture を入力とし、`kind` が
`"nullish-assign on Any LHS (I-050"` を含むことを assert。

**I-050 umbrella PRD への追記**:
`backlog/I-050-any-coercion-umbrella.md` の「依存・連携 PRD」section に `I-142
Cell #15 / #16 (Option<Any> LHS)` を追加し、ideal 出力を記載。

**TODO I-050 entry 更新**: 既存の「依存する下流: I-142 Cell #5/#9」を
「Cell #5/#9/#15/#16」に拡張。

**成功条件**:
- PRD matrix に cell #15 / #16 が enumerate。
- `pick_strategy` exhaustive match に `Option<Any>` 分岐が明示。
- table test + cell lock-in test が存在。
- I-050 umbrella PRD に本 cell が記載。
- Hono bench で `Option<Any>` LHS 使用箇所が blocked-surface に移行 (silent
  compile error → explicit unsupported) の変動を記録。

### C-8 (🟡 interim 不備) Cell #10 `.clone()` INTERIM の removal criterion 未記録

**現状の INTERIM comment** (`src/transformer/expressions/assignments.rs` Identity arm):
```rust
// INTERIM (I-048): the unconditional `.clone()` is conservative — an allocating
// copy is emitted even when the surrounding flow doesn't use `ident` again and
// a move would suffice. A precise move-vs-clone decision requires the
// ownership-inference umbrella (I-048); until it lands, we clone to keep the
// emission compile-safe.
```

**`ideal-implementation-primacy.md` Interim Patch 条件 4 要件**:

| 条件 | 現状 |
|------|------|
| 1. Structural fix PRD or 調査タスクが同時起票 | ✓ I-048 は TODO `I-048` entry 存在 |
| 2. Patch 箇所に `// INTERIM: <task ID>` | ✓ assignments.rs に記載 |
| 3. silent semantic change を導入していない | ✓ `.clone()` は identical semantics (allocation のみ増加) |
| 4. session-todos.md に削除基準 (when to remove) が記載 | ✗ **未実施** |

**問題**: I-048 実装者が本 PRD の Cell #10 interim を見逃す risk。I-048 完了時
に `.clone()` → 動的 move/clone 選択への変更が忘れられる可能性。

**Fix 方針**:

本 project には `session-todos.md` 相当のファイルが存在しないため、**TODO** の
I-048 entry に interim consumer 明示エントリを追加 (project の慣習に合わせた変形)。

TODO `[I-048]` 現行:
```markdown
- **[I-048]** **所有権推論（全体設計）** — 全 clone → ライフタイム分析。RC-2 の根本解決
```

を以下に拡張:
```markdown
- **[I-048]** **所有権推論（全体設計）** — 全 clone → ライフタイム分析。RC-2 の根本解決
  - **Interim consumers (I-048 完了時に置換必要な conservative `.clone()` emission)**:
    - `src/transformer/expressions/assignments.rs` の NullishAssign arm の
      `Identity` strategy — Cell #10 (non-nullable !Copy LHS × Expr context)。
      現行は無条件 `.clone()`。I-048 の move-vs-clone 動的選択が landed したら、
      後続 `ident` 使用の有無で move を選択 (allocation 削減)。
      **置換確認用 lock-in test**: `cell10_non_nullable_non_copy_expr_emits_clone`
      を「ideal output (move / clone 動的選択)」に書き換えること。
      背景: I-142 PRD の Step 3 D-5 / Step 4 C-8 参照。
```

**成功条件**:
- TODO `I-048` entry に Interim consumers section が追加。
- assignments.rs の INTERIM comment が TODO I-048 の interim consumers section
  を参照 (相互リンク)。
- I-048 実装時の test 更新義務が明文化。

### C-9 (🟡 investigation debt → ✅ **消失確認で close**、2026-04-19)

**最終 status** (2026-04-19):
- Hono bench 実測 (`/tmp/hono-bench-errors.json`): total errors **62**、`concurrent.ts` 関連 **0 件**
- INV-Step4-2 が対象としていた `+1 OBJECT_LITERAL_NO_TYPE on utils/concurrent.ts:12` の
  regression は **既に消失**。I-142 Step 4 以降の後続作業 (I-153/I-154 batch 他) で間接的に
  解消された模様。
- 根本原因 commit 特定 (bisection) は historical interest のみで実益なく、user git 操作コスト
  に見合わない → **bisection 不実施で close**。

**close 理由の traceability**:
- 観測時点 (2026-04-15 I-142 Step 3 完了時): bench errors 63、`concurrent.ts:12` に
  OBJECT_LITERAL_NO_TYPE 1 件
- 確認時点 (2026-04-19 I-145/I-150/I-161 batch commit 前): bench errors 62、
  `concurrent.ts` grep 0 件 → 再発時は本 handoff doc を refer して前回 observation 参照

---

**以下、当時の調査計画記録** (close 済、reference のみ):

**当時の現状の記録** (plan.md / PRD Step 3 完了条件):
> error instances 62 → 63 (+1 OBJECT_LITERAL_NO_TYPE: `utils/concurrent.ts:12` —
> 本 PRD 範囲外の pre-existing latency、destructuring param default `= {}` 関連)

**問題**: 上記 claim は **assumption** であり、**fact** として実証されていない。
`todo-entry-standards.md` の「Assumption を fact として記載すること」違反。

**実証すべき事項**:
1. pre-Step-3 (= Step 2 完了時点) bench で `utils/concurrent.ts:12` が OBJECT_LITERAL_NO_TYPE
   として既に検出されていたか、あるいは別 file が該当位置にあり Step 3 で差し替わった
   だけなのか。
2. Step 3 の具体的変更 (D-1 scanner 追加 / D-3 RHS convert skip / switch.rs
   refactor / classes/members.rs refactor / expressions/functions.rs refactor) の
   うちどれが本エラー増を引き起こしたか。
3. D-3 の `convert_expr(&assign.right)` skip が TypeResolver の型伝播 cache 側
   に副作用を及ぼしていないか。

**必須調査 [INV-Step4-2]**: +1 OBJECT_LITERAL_NO_TYPE 根本原因 bisection

- **Known**:
  - Step 2 bench: error 62、OBJECT_LITERAL_NO_TYPE 27 (plan.md の Step 2 記録より)。
  - Step 3 bench: error 63、OBJECT_LITERAL_NO_TYPE 28 (新規 file: `utils/concurrent.ts:12`)。
  - concurrent.ts:12 は `export const createPool = ({...} = {}): Pool => {` の
    destructuring parameter default value (object literal `{}`)。
- **Unknown**: pre-Step-3 で concurrent.ts:12 の error が既に 1 件あったか、0 件
  だったか。後者なら Step 3 変更が引き金。
- **Investigation method**:
  1. Step 3 変更を新 branch として保存 (`git switch -c i142-step3-work` 相当、
     ただしユーザーが git 操作)。
  2. main branch (Step 2 完了時点相当) に戻し、`./scripts/hono-bench.sh` 実行。
  3. `/tmp/hono-bench-errors.json` の OBJECT_LITERAL_NO_TYPE を file-path で
     grep、concurrent.ts の有無確認。
  4. Step 3 変更を commit 単位で個別適用 (D-4 exhaustive match → D-3 RHS convert
     skip → D-1 scanner → switch refactor → classes/members refactor →
     functions refactor の順)、各段階で bench 実行し concurrent.ts error 発生
     commit を特定。
  5. 特定 commit の変更内容から TypeResolver / 型伝播への副作用を解析。
  6. 結果を `report/i142-step4-inv2-object-literal-regression.md` に記録。
- **Impact**:
  - **D-3 由来と判明した場合**: `convert_expr(&assign.right)` の副作用が
    TypeResolver の expected type propagation に寄与していたことが判明 →
    該当 propagation を TypeResolver 側に明示移植する structural fix が必要
    (I-142 Step 4 scope)。
  - **別の commit 由来と判明した場合**: 該当 commit の修正方針を立てる。
  - **Pre-existing で別 file から差し替わっただけと判明した場合**: plan.md
    / PRD の記述を「unrelated pre-existing regression」に訂正 + 該当 file を
    TODO (I-004 / I-005 / I-006 OBJECT_LITERAL_NO_TYPE umbrella) に追加。
- **Resolution target**: Step 4 実装着手前 (D-3 修正を伴う可能性があるため、
  他 C-1 / C-3 / C-4 の着手前に原因特定)。

**Fix 方針**: INV-Step4-2 結果に応じて分岐 (上記 Impact 参照)。

**成功条件**:
- INV-Step4-2 調査が完了し、根本原因が `report/` に記録。
- 原因が Step 3 変更由来なら structural fix 適用。
- 原因が unrelated なら plan.md / PRD 記述を訂正 + 正しい TODO エントリに紐付け。
- Hono bench 結果を再計測し、Step 4 完了時の期待値を明示。

### D-1 (🟢 design improvement) `pre_check_narrowing_reset` call site の分散 / DRY 違反

**現状の手動呼び出し箇所** (6 箇所、同形の 3 行パターン):

1. `src/transformer/statements/mod.rs::convert_stmt_list` (block stmts iterate)
2. `src/transformer/statements/switch.rs::convert_switch_case_body` (switch case cons iterate)
3. `src/transformer/classes/members.rs` (3 箇所): constructor body / method body /
   static block body
4. `src/transformer/expressions/functions.rs` (2 箇所): fn expression body /
   arrow function block body

各箇所に同形の:
```rust
for (i, stmt) in block.stmts.iter().enumerate() {
    self.pre_check_narrowing_reset(stmt, &block.stmts[i + 1..])?;
    result.extend(self.convert_stmt(stmt, return_type)?);
}
```

**問題**:
- **DRY 違反**: 同じ iteration + pre_check pattern を 6 箇所で手動コピペ。
- **silent regression risk**: 新しい block iteration site 追加時 (将来の PRD で
  想定) に pre_check 呼び忘れ → silent compile error 再導入。
- **test 担保不能**: pre_check を呼ばない iteration site が追加されても test は
  検出できない (silent)。

**Fix 方針 (structural)**:

Transformer に helper method を追加し、全 block iteration site を統一:

```rust
impl<'a> Transformer<'a> {
    /// Iterates over a block of TS statements, running the I-142 D-1
    /// narrowing-reset pre-check on each and converting to IR via the
    /// provided closure. Centralises the `for (i, stmt) in stmts.iter().enumerate()`
    /// + `pre_check_narrowing_reset(stmt, &stmts[i+1..])` pattern so new
    /// block-iteration sites automatically inherit the scan.
    ///
    /// Context-specific filtering (e.g., switch-case `break`/`continue`
    /// skipping) should be performed inside the closure.
    pub(crate) fn iter_block_with_reset_check<F, R>(
        &mut self,
        stmts: &[ast::Stmt],
        mut convert: F,
    ) -> Result<Vec<R>>
    where
        F: FnMut(&mut Self, &ast::Stmt) -> Result<Vec<R>>,
    {
        let mut result = Vec::new();
        for (i, stmt) in stmts.iter().enumerate() {
            self.pre_check_narrowing_reset(stmt, &stmts[i + 1..])?;
            result.extend(convert(self, stmt)?);
        }
        Ok(result)
    }
}
```

全 call site を置換:
```rust
// Before:
for (i, stmt) in block.stmts.iter().enumerate() {
    sub_t.pre_check_narrowing_reset(stmt, &block.stmts[i + 1..])?;
    stmts.extend(sub_t.convert_stmt(stmt, return_type.as_ref())?);
}
// After:
stmts.extend(sub_t.iter_block_with_reset_check(&block.stmts, |t, s| {
    t.convert_stmt(s, return_type.as_ref())
})?);
```

`convert_switch_case_body` の filter 付き版:
```rust
fn convert_switch_case_body(
    &mut self,
    cons: &[ast::Stmt],
    return_type: Option<&RustType>,
    drop_continue: bool,
) -> Result<Vec<Stmt>> {
    self.iter_block_with_reset_check(cons, |t, stmt| {
        if matches!(stmt, ast::Stmt::Break(_)) { return Ok(vec![]); }
        if drop_continue && matches!(stmt, ast::Stmt::Continue(_)) { return Ok(vec![]); }
        t.convert_stmt(stmt, return_type)
    })
}
```

**design review 観点 (`prd-design-review.md`)**:
- 凝集度: `iter_block_with_reset_check` は「block iterate + pre_check」の単一
  責務。convert 自体は closure に委譲。
- 責務分離: scan knowledge が helper に集約、convert logic は caller に残る。
- DRY: 6 箇所の重複を 1 helper に統一。

**成功条件**:
- 全 block iteration site が `iter_block_with_reset_check` 経由。
- 新しい block iteration site 追加時は本 helper の呼び出しで scan が自動走行。
- helper の unit test で scan ordering (pre_check → convert) が lock-in。

---

## Step 4 完了条件 (新規、次セッション着手)

着手前 調査:
- [ ] **INV-Step4-1**: closure body reassign の Rust compile 可否実測 →
      `report/i142-step4-inv1-closure-compile.md`
- [ ] **INV-Step4-2**: +1 OBJECT_LITERAL_NO_TYPE bisection → `report/i142-step4-inv2-object-literal-regression.md`

実装:
- [ ] **C-1** (🔴): scanner AssignExpr arm の op 種別 filter (`Assign` / `NullishAssign` /
      `OrAssign` / `AndAssign` のみ reset) + UpdateExpr arm を non-reset 化
- [ ] **C-2** (🔴): INV-Step4-1 結果に応じて scanner policy / emission / test を
      修正 (仮説 A: compile 検証 test 追加 / 仮説 B: structural fix)
- [ ] **C-3** (🔴): scanner 再帰 branch 22+ variant の lock-in test 追加
- [ ] **C-4** (🔴): 非-reset ケース 22+ variant の lock-in test 追加
- [ ] **C-5** (🟡): D-2 Class D 残 4 variant (TsTypeAssertion/TsSatisfies/TsConstAssertion/TsInstantiation)
      の parameterized test 追加
- [ ] **C-6** (🟡): Seq RHS test の assertion を Seq 固有 message 検証に強化、
      他 class の weak assertion も同様に strict 化
- [ ] **C-7** (🟡): `Option<Any>` cell #15 / #16 を matrix に追加 + `pick_strategy`
      分岐 + table test + cell lock-in test + I-050 umbrella PRD 更新
- [ ] **C-8** (🟡): TODO `I-048` entry に interim consumers section 追加 + code
      comment との相互リンク
- [ ] **C-9** (🟡): INV-Step4-2 結果に応じた fix または TODO 化 + plan.md / PRD
      記述訂正
- [ ] **D-1** (🟢): `iter_block_with_reset_check` helper 追加 + 全 call site 統一

post-work:
- [ ] Step 4 完了後の敵対的 self-review で新 defect が出ないこと
- [ ] Hono bench 再計測 + 全変動を plan.md に記録 (C-7 の Option<Any> surface 追加
      + C-9 の訂正値 + C-1 fix で復活する compound op 変換の件数を個別に追跡)
- [ ] matrix completeness audit を全項目 [x] に更新
