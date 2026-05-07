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
#[ignore = "I-224 helper test stub: Implementation Stage T3 で fill in (3 fixture variants \
            with same Axis A+C, different B1 form、transpile 出力 IR token-level identical assert)"]
fn test_axis_b_b1a_b_c_rename_dispatch_symmetric() {
    let _ = transpile;
    unimplemented!(
        "Spec stage stub、Implementation Stage T3 で fill in: \
         3 fixture variants (B1a function decl / B1b const arrow / B1c const fn expr) を \
         同 Axis A+C で構築、各 transpile 結果が `fn __ts_main()` rename + main() substitute \
         output で identical を assert"
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
#[ignore = "I-224 helper test stub: Implementation Stage T3 で fill in (E1 form fixtures with \
            export keyword、Rust 出力 `pub` modifier 配置を per-symbol assert)"]
fn test_axis_e_export_preserve_symmetric() {
    let _ = transpile;
    unimplemented!(
        "Spec stage stub、Implementation Stage T3 で fill in: \
         E1 form input (export function helper / export function main) で transpile、\
         Rust 出力で `pub fn helper()` preserve かつ `fn __ts_main()` private (no pub) を assert"
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
#[ignore = "I-224 helper test stub: Implementation Stage T3 で fill in (A5a fixture を 4 B variants \
            (B0/B1/B2/B3/B4) で構築、cells 51/53/55/57/59 期待出力との byte-exact match assert)"]
fn test_axis_a5a_compositional_orthogonality_with_b_axis() {
    let _ = transpile;
    unimplemented!(
        "Spec stage stub、Implementation Stage T3 で fill in: \
         cell 51 (A5a + B0) representative fixture + B0/B1/B2/B3/B4 variants 4 件で transpile、\
         cells 51/53/55/57/59 期待 Rust 出力との byte-exact assertion"
    );
}
