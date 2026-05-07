//! I-224 fn main mechanism + Axis orthogonality helper test contracts (Spec stage v6 minor
//! convention compliance、Rule 9 (a) helper test contracts NEW per spec-stage-adversarial-checklist
//! v1.6、I-205 v1.6 self-applied integration pattern 踏襲)。
//!
//! Spec stage iteration v7 で本 file を `#[test] #[ignore]` stub として author (5 rounds
//! adversarial review iteration を経た convergence state での Spec stage true closure
//! commitment、deferred verification = unverified claim compromise の structural elimination)。
//! Implementation Stage T2/T3 で各 stub を fill-in、`#[ignore]` 解除で green-ify。
//!
//! ## Layered test design (I-205 pattern 踏襲)
//!
//! 本 file は **integration-level (= TS source → transpile pipeline → Rust source 文字列)**
//! の helper test contracts。Unit-level の helper test (= IR helper / dispatch arm の
//! 直接呼出) は Implementation Stage T2/T3 で `src/transformer/main_synthesis.rs::tests`
//! に配置予定。
//!
//! ## Test contracts (Spec stage commitment、4 stubs)
//!
//! 1. **Rule 9 (a) 1-to-1 mapping**: 80-cell matrix の各 in-scope cell の (Axis A/B/C)
//!    から `(is_executable_mode, user_main_kind, has_top_level_await)` 3-tuple を
//!    derive、dispatch tree の expected arm が exactly 1 つ match することを probe
//!    (= iteration v4 Critical 1 = axis-tuple ↔ definition mismatch の structural
//!    regression lock-in)
//! 2. **Axis B B1 orthogonality merge**: function decl / const arrow / const fn expr
//!    の 3 forms 全てが `__ts_main` rename + main() substitute で同一 IR 出力に collapse
//!    することを probe (Rule 1 (1-4-c) compliance)
//! 3. **Axis E orthogonality merge + `pub` modifier preservation rule**: E1 (export
//!    keyword 存在) form の入力で `pub` modifier が non-`__ts_main` symbols に preserve、
//!    `__ts_main` rename target には付与されない (= INV-5 transpiler-internal
//!    identifier 整合) を probe
//! 4. **Axis A5a × B compositional invariant**: cell 51 representative (A5a + B0 silent
//!    skip) と B-axis dispatch leaves (cells 3/5/7 のいずれか) の orthogonal composition
//!    が cells 53/55/57 の expected output と一致、cell 59 (A5a + B4 collision) は
//!    INV-5 priority arm 先行 reject を probe

use ts_to_rs::parser::parse_typescript;
use ts_to_rs::transformer::main_synthesis::{
    classify_dispatch_arm, detect_user_main, has_top_level_await, is_executable_mode, DispatchArm,
};
use ts_to_rs::transpile;

/// Helper test 1: Rule 9 (a) Spec → Impl Dispatch Arm Mapping 1-to-1 verification
///
/// 80-cell matrix の各 in-scope cell に対し以下を assert:
/// 1. cell の (Axis A variant, Axis B variant, Axis C variant) から fixture を derive
/// 2. helper で derive される `(is_executable_mode, user_main_kind, has_top_level_await)`
///    3-tuple を計算
/// 3. dispatch tree の match で expected arm が選択されることを assert
/// 4. expected arm の matrix # 列挙に本 cell が含まれることを cross-check
///
/// **Lesson source**: iteration v4 Critical 1 (旧 4-tuple match dispatch tree で cells
/// #5/#25 が `is_async_required=false` pattern claim → unreachable!() panic に fall-through)
/// の structural regression lock-in test。本 test が future iteration で同種 axis-tuple ↔
/// definition mismatch 混入を fail で発覚保証。
#[test]
fn test_dispatch_arm_one_to_one_mapping_per_in_scope_cell() {
    // Each entry = (matrix cell #, axis summary, TS source, expected DispatchArm).
    //
    // Coverage rationale (Rule 9 (a) 1-to-1): every reachable matrix cell from PRD
    // Design section #2's mapping table appears here exactly once, and every
    // `DispatchArm` variant has at least one cell. Orthogonality-merged Collision
    // cells (matrix # 49 / 59 / 69 = A4 / A5a / A5b combined with B4) are listed
    // explicitly to lock in INV-5 priority over A-axis dispatch. The 25 NA cells
    // (A0/A2/A4/A5a/A5b + C1) are excluded by AST mutual exclusion and locked in by
    // `tests/swc_parser_top_level_await_test.rs`; they are not test cases here.
    //
    // **Naming convention**: TS sources use plain `function main()` for B1, plain
    // `async function main()` for B2, plain `interface main { ... }` for B3, plain
    // `function __ts_main()` for B4. Other B1/B2 forms (const arrow / const fn-expr)
    // are covered by the Axis B orthogonality probe (`test_axis_b_b1a_b_c_rename_dispatch_symmetric`,
    // T3-2 stub).
    let cases: &[(u32, &str, &str, DispatchArm)] = &[
        // ===== Library mode A0 =====
        (1, "A0+B0+C0", "function helper(): number { return 7; }\n", DispatchArm::LibraryNone),
        (3, "A0+B1+C0", "function main(): void { }\n", DispatchArm::LibraryFnSyncDirect),
        (5, "A0+B2+C0", "async function main(): Promise<void> { }\n", DispatchArm::LibraryFnAsyncDirect),
        (7, "A0+B3+C0", "interface main { x: number; }\n", DispatchArm::LibraryNonFn),
        (9, "A0+B4+C0", "function __ts_main(): void { }\n", DispatchArm::Collision),

        // ===== Library mode A2 (Lit init Decl::Var co-existing with B-axis decl) =====
        (21, "A2+B0+C0", "const x: number = 0;\n", DispatchArm::LibraryNone),
        (23, "A2+B1+C0", "const x: number = 0;\nfunction main(): void { }\n", DispatchArm::LibraryFnSyncDirect),
        (25, "A2+B2+C0", "const x: number = 0;\nasync function main(): Promise<void> { }\n", DispatchArm::LibraryFnAsyncDirect),
        (27, "A2+B3+C0", "const x: number = 0;\ninterface main { x: number; }\n", DispatchArm::LibraryNonFn),
        (29, "A2+B4+C0", "const x: number = 0;\nfunction __ts_main(): void { }\n", DispatchArm::Collision),

        // ===== Executable mode A1 + C0 =====
        (11, "A1+B0+C0", "console.log('hi');\n", DispatchArm::ExecNoneSync),
        (13, "A1+B1+C0", "function main(): void { }\nconsole.log('hi');\n", DispatchArm::ExecFnSyncRename),
        (15, "A1+B2+C0", "async function main(): Promise<void> { }\nconsole.log('hi');\n", DispatchArm::ExecFnAsyncRename),
        (17, "A1+B3+C0", "interface main { x: number; }\nconsole.log('hi');\n", DispatchArm::ExecNonFnSync),
        (19, "A1+B4+C0", "function __ts_main(): void { }\nconsole.log('hi');\n", DispatchArm::Collision),

        // ===== Executable mode A3 + C0 (side-effect Decl::Var) =====
        (31, "A3+B0+C0", "declare function f(): number;\nconst c = f();\n", DispatchArm::ExecNoneSync),
        (33, "A3+B1+C0", "declare function f(): number;\nfunction main(): void { }\nconst c = f();\n", DispatchArm::ExecFnSyncRename),
        (35, "A3+B2+C0", "declare function f(): number;\nasync function main(): Promise<void> { }\nconst c = f();\n", DispatchArm::ExecFnAsyncRename),
        (37, "A3+B3+C0", "declare function f(): number;\ninterface main { x: number; }\nconst c = f();\n", DispatchArm::ExecNonFnSync),
        (39, "A3+B4+C0", "declare function f(): number;\nfunction __ts_main(): void { }\nconst c = f();\n", DispatchArm::Collision),

        // ===== Executable mode A6 (mixed = A1 Stmt::Expr + A2 Lit Decl::Var) + C0 =====
        (71, "A6+B0+C0", "const X: number = 1;\nconsole.log(X);\n", DispatchArm::ExecNoneSync),
        (73, "A6+B1+C0", "const X: number = 1;\nfunction main(): void { }\nconsole.log(X);\n", DispatchArm::ExecFnSyncRename),
        (75, "A6+B2+C0", "const X: number = 1;\nasync function main(): Promise<void> { }\nconsole.log(X);\n", DispatchArm::ExecFnAsyncRename),
        (77, "A6+B3+C0", "const X: number = 1;\ninterface main { x: number; }\nconsole.log(X);\n", DispatchArm::ExecNonFnSync),
        (79, "A6+B4+C0", "const X: number = 1;\nfunction __ts_main(): void { }\nconsole.log(X);\n", DispatchArm::Collision),

        // ===== Executable mode A1 + C1 (top-level Stmt::Expr Await) =====
        (12, "A1+B0+C1", "declare const p: Promise<number>;\nawait p;\n", DispatchArm::ExecNoneAsync),
        (14, "A1+B1+C1", "declare const p: Promise<number>;\nfunction main(): void { }\nawait p;\n", DispatchArm::ExecFnSyncRenameAsync),
        (16, "A1+B2+C1", "declare const p: Promise<number>;\nasync function main(): Promise<void> { }\nawait p;\n", DispatchArm::ExecFnAsyncRenameAsync),
        (18, "A1+B3+C1", "declare const p: Promise<number>;\ninterface main { x: number; }\nawait p;\n", DispatchArm::ExecNonFnAsync),
        (20, "A1+B4+C1", "declare const p: Promise<number>;\nfunction __ts_main(): void { }\nawait p;\n", DispatchArm::Collision),

        // ===== Executable mode A3 + C1 (Decl::Var await init) =====
        (32, "A3+B0+C1", "declare function f(): Promise<number>;\nconst c = await f();\n", DispatchArm::ExecNoneAsync),
        (34, "A3+B1+C1", "declare function f(): Promise<number>;\nfunction main(): void { }\nconst c = await f();\n", DispatchArm::ExecFnSyncRenameAsync),
        (36, "A3+B2+C1", "declare function f(): Promise<number>;\nasync function main(): Promise<void> { }\nconst c = await f();\n", DispatchArm::ExecFnAsyncRenameAsync),
        (38, "A3+B3+C1", "declare function f(): Promise<number>;\ninterface main { x: number; }\nconst c = await f();\n", DispatchArm::ExecNonFnAsync),
        (40, "A3+B4+C1", "declare function f(): Promise<number>;\nfunction __ts_main(): void { }\nconst c = await f();\n", DispatchArm::Collision),

        // ===== Executable mode A6 + C1 =====
        (72, "A6+B0+C1", "declare const p: Promise<number>;\nconst X: number = 1;\nawait p;\n", DispatchArm::ExecNoneAsync),
        (74, "A6+B1+C1", "declare const p: Promise<number>;\nconst X: number = 1;\nfunction main(): void { }\nawait p;\n", DispatchArm::ExecFnSyncRenameAsync),
        (76, "A6+B2+C1", "declare const p: Promise<number>;\nconst X: number = 1;\nasync function main(): Promise<void> { }\nawait p;\n", DispatchArm::ExecFnAsyncRenameAsync),
        (78, "A6+B3+C1", "declare const p: Promise<number>;\nconst X: number = 1;\ninterface main { x: number; }\nawait p;\n", DispatchArm::ExecNonFnAsync),
        (80, "A6+B4+C1", "declare const p: Promise<number>;\nconst X: number = 1;\nfunction __ts_main(): void { }\nawait p;\n", DispatchArm::Collision),

        // ===== Orthogonality-merged Collision (A4 / A5a / A5b + B4) =====
        // INV-5 priority arm absorbs these: the (_, Collision, _) match arm fires
        // regardless of A-axis (control-flow / Empty / Debugger), proving the
        // namespace-reservation invariant supersedes structural A-axis dispatch.
        (49, "A4+B4+C0", "function __ts_main(): void { }\nif (true) { console.log('top'); }\n", DispatchArm::Collision),
        (59, "A5a+B4+C0", "function __ts_main(): void { }\n;\n", DispatchArm::Collision),
        (69, "A5b+B4+C0", "function __ts_main(): void { }\ndebugger;\n", DispatchArm::Collision),
    ];

    // Sanity: every DispatchArm variant is exercised at least once. If a future
    // refactor adds a new variant, this assertion ensures the table is updated.
    for variant in [
        DispatchArm::Collision,
        DispatchArm::LibraryNone,
        DispatchArm::LibraryFnSyncDirect,
        DispatchArm::LibraryFnAsyncDirect,
        DispatchArm::LibraryNonFn,
        DispatchArm::ExecNoneSync,
        DispatchArm::ExecFnSyncRename,
        DispatchArm::ExecFnAsyncRename,
        DispatchArm::ExecNonFnSync,
        DispatchArm::ExecNoneAsync,
        DispatchArm::ExecFnSyncRenameAsync,
        DispatchArm::ExecFnAsyncRenameAsync,
        DispatchArm::ExecNonFnAsync,
    ] {
        assert!(
            cases.iter().any(|(_, _, _, expected)| *expected == variant),
            "DispatchArm::{variant:?} has no representative cell — Rule 9 (a) coverage gap"
        );
    }

    for (cell, axis_summary, src, expected) in cases {
        let module = parse_typescript(src)
            .unwrap_or_else(|e| panic!("cell #{cell} ({axis_summary}): SWC parse failed: {e}"));
        let exec_mode = is_executable_mode(&module);
        let user_main_kind = detect_user_main(&module);
        let await_flag = has_top_level_await(&module);
        let actual = classify_dispatch_arm(exec_mode, user_main_kind, await_flag);
        assert_eq!(
            actual, *expected,
            "cell #{cell} ({axis_summary}): expected {expected:?}, got {actual:?} \
             (is_executable_mode={exec_mode}, user_main_kind={user_main_kind:?}, \
              has_top_level_await={await_flag})"
        );
    }
}

/// Helper test 2: Axis B B1 orthogonality merge structural verify
///
/// `function main()` (B1a function decl) / `const main = () => {}` (B1b const arrow) /
/// `const main = function() {}` (B1c const fn expr) の 3 forms 全てに対し以下を verify:
/// - rename target identifier が同一 `__ts_main` に collapse
/// - main() call site substitute logic が同一 dispatch path を通過
/// - generated Rust output の IR shape (function definition + caller substitute) が
///   3 forms 共通で identical (Decl::Var with init=Arrow vs init=Fn の AST shape 差は
///   transpile 後の `fn __ts_main()` Rust function definition に collapse)
///
/// **Rule 1 (1-4-c) compliance**: orthogonality merge legitimacy の Spec-stage referenced
/// cell symmetry probe を Implementation stage で empirical lock-in。各 (A, C) cell with B1
/// が 3 forms 共通 dispatch を生成することの structural assertion。
#[test]
fn test_axis_b_b1a_b_c_rename_dispatch_symmetric() {
    // Axis A+C fixed at A1+C0 (= cell 13 representative): a top-level
    // `console.log("hi");` ensures executable mode is true (= rename gate
    // fires), and the absence of any top-level await stays in the C0 partition.
    // Each fixture varies ONLY the B1 sub-form (function decl / const arrow /
    // const fn expr); the user main body is `console.log("hi"); main();`
    // (= internal main() recursive call to also exercise the call-substitute
    // path inside the user main body, providing an additional structural
    // probe that the substitute mechanism applies uniformly to all callers).
    //
    // The single user-source-text difference between the three fixtures is
    // limited to the B1 declaration-form keyword(s) ("function" / "=" /
    // "function" with assignment), so any difference in the resulting Rust
    // would imply the dispatch is NOT 3-form symmetric (= would falsify
    // the orthogonality merge claim of Rule 1 (1-4-c) for B-axis B1).
    let body = "console.log(\"hi\");\n  main();\n";
    let trailing = "main();\n";
    let b1a = format!("function main(): void {{\n  {body}}}\n{trailing}",);
    let b1b = format!("const main = (): void => {{\n  {body}}};\n{trailing}",);
    let b1c = format!("const main = function(): void {{\n  {body}}};\n{trailing}",);

    let r_a = transpile(&b1a).expect("B1a transpile must succeed");
    let r_b = transpile(&b1b).expect("B1b transpile must succeed");
    let r_c = transpile(&b1c).expect("B1c transpile must succeed");

    // Structural rename + substitute + synthesis lock-in (= the union of three
    // I-224 mechanisms expected to fire on every B1 form):
    //   (a) Rename gate: user's `function main` (or arrow / fn-expr equivalent)
    //       emits `fn __ts_main` instead of `fn main`.
    //   (b) Call-substitute gate: every `main()` call site (the body's
    //       recursive call AND the top-level trailing call) emits as
    //       `__ts_main()`.
    //   (c) Synthesis (T4-1 wiring): an `ExecFnSyncRename` dispatch arm emits
    //       a `fn main()` body wrapping the captured top-level call, so the
    //       binary entry exists and invokes the renamed user main.
    for (label, source) in [("B1a", &r_a), ("B1b", &r_b), ("B1c", &r_c)] {
        assert!(
            source.contains("fn __ts_main"),
            "{label} (a): expected renamed `fn __ts_main` in output, got:\n{source}"
        );
        assert!(
            source.contains("fn main()"),
            "{label} (c): expected synthesized binary entry `fn main()` (T4-1 \
             ExecFnSyncRename arm) in output, got:\n{source}"
        );
        // (b) Substituted call sites: at least 3 occurrences of `__ts_main()`:
        //   1. inside the user main body's recursive call (body line 1 of fixture)
        //   2. the top-level trailing call captured into synthesized fn main
        //   3. the user main body's own definition `fn __ts_main()` count includes
        //      a parens-less `__ts_main` token; the count below uses `__ts_main()`
        //      with parens to filter out the definition site.
        let occurrences = source.matches("__ts_main()").count();
        assert!(
            occurrences >= 2,
            "{label} (b): expected ≥ 2 `__ts_main()` substituted call sites \
             (recursive + synthesized-main body), got {occurrences} in:\n{source}"
        );
    }

    // Byte-exact symmetry: the three forms must produce IDENTICAL output
    // (= the only differences in user-source-text between the fixtures are
    // structural shape variants of the same B-axis B1 declaration; their
    // post-rename IR collapses to the same `fn __ts_main` definition + the
    // same call-site substitution scheme).
    assert_eq!(
        r_a, r_b,
        "B1a vs B1b output diverges — orthogonality merge violated:\n\
         === B1a ===\n{r_a}\n=== B1b ===\n{r_b}"
    );
    assert_eq!(
        r_b, r_c,
        "B1b vs B1c output diverges — orthogonality merge violated:\n\
         === B1b ===\n{r_b}\n=== B1c ===\n{r_c}"
    );
}

/// Helper test 3: Axis E `pub` modifier preservation rule + orthogonality merge
///
/// E1 form (`export function f()`, `export const X = ...` 等) の入力で以下を verify:
/// - non-`__ts_main` symbols (例: `export function helper()`) は Rust 側 `pub fn helper()` で
///   `pub` modifier preserve (existing path 維持、library export semantic)
/// - **`__ts_main` rename target は private (`fn __ts_main()`、`pub` 不付与)**: INV-5 整合 =
///   transpiler-internal identifier として external API expose されない
/// - `fn main` (synthesized) 自身も private (binary entry point convention)
///
/// **Rule 1 (1-4-c) compliance + INV-5 cross-reference**: Axis E orthogonality merge
/// declaration の structural verify probe。representative reachable cells 11/13/31 から
/// E1 form を probe。
#[test]
fn test_axis_e_export_preserve_symmetric() {
    // Axis E E1 (= export keyword present) probe. Two distinct claims must
    // hold simultaneously:
    //   1. **`pub` modifier preservation** for non-`__ts_main` symbols:
    //      `export function helper()` → `pub fn helper()` survives the
    //      I-224 rewrite path unchanged (= existing `transform_decl`
    //      ExportDecl path with `Visibility::Public` continues to apply).
    //   2. **`pub` modifier drop** for the `__ts_main` rename target:
    //      `export function main()` (= user wrote an exported main, treats
    //      it as public surface) is renamed to `__ts_main` with visibility
    //      forced to `Private` per INV-5 (= the renamed identifier is a
    //      transpiler-internal symbol; exposing it would leak the
    //      synthesis detail).
    //
    // The two claims are independent dimensions of the Axis E orthogonality
    // — they must be probed on the SAME fixture to observe both at once
    // (= regression in either direction breaks the orthogonality merge).
    //
    // Fixture: cell 13 representative (A1 + B1 + C0) in E1 form. The user
    // main + helper export pair captures both symbols.
    let src = "\
export function helper(): number {\n\
  return 7;\n\
}\n\
export function main(): void {\n\
  console.log(\"hi\");\n\
}\n\
main();\n";
    let rust = transpile(src).expect("Axis E E1 fixture must transpile successfully");

    // Claim 1: `pub fn helper()` preserved (= the user's export of `helper`
    // produces a `pub` Rust function; INV-5 does not apply to non-rename
    // identifiers).
    assert!(
        rust.contains("pub fn helper"),
        "Axis E: `pub fn helper` (= export preservation for non-rename symbol) \
         must be present, got:\n{rust}"
    );

    // Claim 2: `__ts_main` rename target is private (= INV-5 compliance).
    // Asserts the negative form (`pub fn __ts_main` must not appear) AND
    // the positive form (the bare `fn __ts_main` declaration must appear).
    assert!(
        !rust.contains("pub fn __ts_main"),
        "Axis E / INV-5: `pub fn __ts_main` must NOT appear — the rename \
         target is transpiler-internal and visibility is forced to Private. \
         got:\n{rust}"
    );
    assert!(
        rust.contains("fn __ts_main"),
        "Axis E: bare `fn __ts_main` declaration must be present (= rename \
         fired and emitted the user's main as private). got:\n{rust}"
    );

    // Cross-axis sanity: the call site is also substituted (= T3-3 gate
    // fired in the same fixture, validating the rename + substitution
    // pair fires uniformly under the `user_main_substitution` flag).
    assert!(
        rust.contains("__ts_main()"),
        "Axis E: substituted `__ts_main()` call site must be present (= \
         the rename-substitute pair is symmetric). got:\n{rust}"
    );
}

/// Helper test 4: Axis A5a × B compositional orthogonality probe
///
/// cell 51 representative (A5a + B0 + C0 = Stmt::Empty silent skip + no user main) と
/// B-axis dispatch leaves の orthogonal composition で cells 53/55/57 の expected output が
/// 一致することを verify:
/// - A5a + B1 (cell 53): silent skip + sync user main directly emit = `fn main { user body }`
/// - A5a + B2 (cell 55): silent skip + async user main directly emit = `#[tokio::main] async fn main`
/// - A5a + B3 (cell 57): silent skip + non-fn preserved (interface) + library mode
/// - A5a + B4 (cell 59): INV-5 collision priority arm 先行 reject (Tier 2 honest error、cell 9 と同 wording)
///
/// **Compositional invariant**: A5a (silent skip) と B-axis dispatch が orthogonal compose
/// する仕様を Implementation 後 empirical lock-in (= matrix Scope 列の "regression lock-in
/// (orthogonality merged)" claim の structural verification)。
#[test]
fn test_axis_a5a_compositional_orthogonality_with_b_axis() {
    // A5a (Stmt::Empty) is a silent-skip top-level item per PRD Design
    // section #3 (= no Rust emission, no executable trigger). The
    // orthogonality merge declaration claims that cells 51 / 53 / 55 / 57
    // / 59 (A5a + B0 / B1 / B2 / B3 / B4) emit identically to their
    // A0-axis equivalents (cells 1 / 3 / 5 / 7 / 9). This probe verifies
    // the **byte-exact equivalence** of each (A5a, Bn) cell with its
    // (A0, Bn) baseline, locking in the merge claim against future
    // refactoring that might silently change the silent-skip semantics
    // (e.g., to "skip but log a warning" or "skip with side-effect").
    //
    // **Cell 59 (A5a + B4 collision)** is verified separately as a
    // negative-path probe: both the (A0, B4) baseline (cell 9) and the
    // (A5a, B4) variant (cell 59) must yield a transpile error with the
    // same INV-5 collision wording (= the namespace lint fires before
    // any A-axis dispatch, regardless of whether `;` precedes the
    // `function __ts_main()` declaration).
    //
    // Each entry = (axis summary, A0 baseline source, A5a variant source).
    let success_cases: &[(&str, &str, &str)] = &[
        // Cell 51 vs Cell 1 (B0): library-mode declarations only emit.
        (
            "B0 (cells 51 vs 1)",
            // A0 baseline: empty / declarations only.
            "function helper(): number { return 7; }\n",
            // A5a variant: `;` prepended (silent skip).
            ";\nfunction helper(): number { return 7; }\n",
        ),
        // Cell 53 vs Cell 3 (B1 sync): `fn main` directly emits user body.
        (
            "B1 (cells 53 vs 3)",
            "function main(): void {\n  console.log(\"hi\");\n}\n",
            ";\nfunction main(): void {\n  console.log(\"hi\");\n}\n",
        ),
        // Cell 55 vs Cell 5 (B2 async): `#[tokio::main] async fn main`.
        (
            "B2 (cells 55 vs 5)",
            "async function main(): Promise<void> {\n  console.log(\"hi\");\n}\n",
            ";\nasync function main(): Promise<void> {\n  console.log(\"hi\");\n}\n",
        ),
        // Cell 57 vs Cell 7 (B3 NonFn): non-fn symbol preserved.
        (
            "B3 (cells 57 vs 7)",
            "interface main {\n  x: number;\n}\n",
            ";\ninterface main {\n  x: number;\n}\n",
        ),
    ];

    for (axis, baseline_src, variant_src) in success_cases {
        let baseline = transpile(baseline_src).unwrap_or_else(|e| {
            panic!("{axis}: A0 baseline must transpile: {e}\nsource:\n{baseline_src}")
        });
        let variant = transpile(variant_src).unwrap_or_else(|e| {
            panic!("{axis}: A5a variant must transpile: {e}\nsource:\n{variant_src}")
        });
        assert_eq!(
            baseline, variant,
            "{axis}: A5a + Bn must produce byte-exact same output as A0 + Bn \
             (= silent-skip orthogonality merge). \n=== A0 baseline ===\n{baseline}\n\
             === A5a variant ===\n{variant}"
        );
    }

    // ===== Cell 59 vs Cell 9 (B4 collision) negative-path probe =====
    // Both must abort with INV-5 namespace collision wording. The lint
    // runs before any A-axis dispatch in `transform_module`, so the `;`
    // prefix has no effect on the rejection.
    let cell9_src = "function __ts_main(): void { }\n";
    let cell59_src = ";\nfunction __ts_main(): void { }\n";
    let cell9_err = transpile(cell9_src).expect_err(
        "cell 9 (A0 + B4 collision): INV-5 namespace lint must reject __ts_main user identifier",
    );
    let cell59_err = transpile(cell59_src).expect_err(
        "cell 59 (A5a + B4 collision): silent-skip prefix must not bypass INV-5 namespace lint",
    );
    let cell9_msg = cell9_err.to_string();
    let cell59_msg = cell59_err.to_string();
    // Both errors must mention the `__ts_main` namespace reservation as
    // the collision source. Substring lock-in is conservative — the exact
    // wording is owned by `scan_for_ts_namespace_collisions` and may
    // evolve, but `__ts_main` must remain in the message.
    assert!(
        cell9_msg.contains("__ts_main"),
        "cell 9: error message must reference __ts_main, got: {cell9_msg}"
    );
    assert!(
        cell59_msg.contains("__ts_main"),
        "cell 59: error message must reference __ts_main, got: {cell59_msg}"
    );
    // Symmetric wording: the `;` silent-skip prefix should produce
    // structurally identical error wording up to the trailing byte
    // offset (which inherently differs because the source position of
    // `__ts_main` shifts by the `;\n` prefix length). Strip the trailing
    // ` at byte <N>` suffix and compare the structural message body.
    let strip_byte_offset = |msg: &str| -> String {
        match msg.rfind(" at byte ") {
            Some(pos) => msg[..pos].to_string(),
            None => msg.to_string(),
        }
    };
    assert_eq!(
        strip_byte_offset(&cell9_msg),
        strip_byte_offset(&cell59_msg),
        "B4 collision priority: cell 9 vs cell 59 error message bodies \
         must be identical modulo byte offset (= silent-skip orthogonality \
         with collision arm; the byte offset inherently shifts by the `;\\n` \
         prefix length and is not part of the structural wording). \n\
         === cell 9 ===\n{cell9_msg}\n=== cell 59 ===\n{cell59_msg}"
    );
}

/// T3-4 generator guard regression lock-in: the B1c arm in
/// `convert_var_decl_module_level` (`Expr::Fn` synthetic FnDecl path) MUST
/// skip generator function expressions (`function*`) to preserve the
/// pre-T3 silent-drop behavior for those forms (= `convert_fn_decl` does
/// not support `Yield` and would otherwise expose previously-hidden
/// unsupported-syntax errors).
///
/// **Lesson source**: T3-4 `/check_job` deep review (2026-05-07) caught a
/// Hono bench regression on `helper/ssg/ssg.ts:203` (`export const
/// fetchRoutesContent = function* <...>`) — pre-T3 the FnExpr init was
/// silently dropped by the legacy `_ => continue` arm; post-T3 the new
/// B1c arm routed it through `convert_fn_decl`, which surfaced the
/// `Yield` body as an unsupported-expression error (= +1 OTHER bench
/// category). The fix narrows the B1c arm to non-generator FnExpr only,
/// restoring Tier-transition Preservation (clean 111 / errors 63).
///
/// This test asserts:
/// - Non-generator FnExpr init produces a renamed-or-named `fn` declaration.
/// - Generator FnExpr init does **not** produce any `fn` declaration
///   (= silent drop preserved).
#[test]
fn test_b1c_fn_expr_generator_guard_preserves_silent_drop() {
    // (1) Non-generator FnExpr init: B1c form for arbitrary name (non-`main`).
    //     The B1c arm routes through `convert_fn_decl` and emits an
    //     `Item::Fn`. (No rename because the binding name isn't "main"
    //     and `user_main_substitution` is false in library mode.)
    let non_gen = transpile("const helper = function(): number { return 7; };\n")
        .expect("non-generator FnExpr init must transpile cleanly");
    assert!(
        non_gen.contains("fn helper"),
        "non-generator FnExpr init must emit `fn helper` (B1c handling); got:\n{non_gen}"
    );

    // (2) Generator FnExpr init: must be silently dropped by the
    //     generator guard. No `fn` declaration emitted; the FnExpr
    //     binding is invisible in the Rust output (= preserves the
    //     pre-T3 `_ => continue` fallback for this shape).
    //
    //     The generator body's `yield` is a separate Tier 3
    //     unsupported-syntax owner (I-016 / future PRD); skipping the
    //     binding entirely avoids surfacing it via the B1c path.
    let gen = transpile("const stream = function* (): Generator<number> { yield 1; };\n")
        .expect("generator FnExpr init must transpile cleanly (silent drop)");
    assert!(
        !gen.contains("fn stream"),
        "generator FnExpr init must NOT emit `fn stream` (= silent drop \
         preserved by generator guard); got:\n{gen}"
    );
    assert!(
        !gen.contains("yield"),
        "generator FnExpr body must NOT leak into output (= silent drop \
         applies to the entire FnExpr); got:\n{gen}"
    );

    // (3) Async non-generator FnExpr: still supported (= async function
    //     expression for B1c async user main case, cell 5 / 15 / etc.).
    //     The guard targets `is_generator` only, not `is_async`.
    let async_non_gen =
        transpile("const fetcher = async function(): Promise<number> { return 7; };\n")
            .expect("async non-generator FnExpr init must transpile cleanly");
    assert!(
        async_non_gen.contains("async fn fetcher") || async_non_gen.contains("fn fetcher"),
        "async non-generator FnExpr init must emit a fn (async or sync); got:\n{async_non_gen}"
    );

    // (4) Async generator FnExpr (`async function*`): also `is_generator=true`,
    //     so the guard fires and the binding is silently dropped (= same
    //     treatment as sync generator). Locks in symmetric guard behavior
    //     for the async-generator AST shape, ensuring a future refactor
    //     that narrows the guard to sync `function*` only does not regress
    //     async-generator handling.
    let async_gen =
        transpile("const asyncStream = async function* (): AsyncGenerator<number> { yield 1; };\n")
            .expect("async generator FnExpr init must transpile cleanly (silent drop)");
    assert!(
        !async_gen.contains("fn asyncStream"),
        "async generator FnExpr init must NOT emit `fn asyncStream` (= silent drop \
         preserved by generator guard, symmetric with sync generator); got:\n{async_gen}"
    );
}
