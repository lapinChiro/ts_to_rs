//! I-224 Invariants verification tests (Spec stage v6 minor + iteration v7 stub commitment、
//! Rule 8 (8-c) helper test contracts NEW per spec-stage-adversarial-checklist v1.6、
//! I-205 v1.6 self-applied integration pattern 踏襲)。
//!
//! Spec stage iteration v7 で 7 invariants (INV-1〜INV-7) verification method (Rule 8 (8-c))
//! を concrete test contract として `#[test] #[ignore]` stub で author。Implementation Stage
//! T2/T3/T5 で各 stub を fill-in、`#[ignore]` 解除で green-ify。
//!
//! 各 invariant の verification statement (a/b/c/d) は backlog/I-224-top-level-fn-main-mechanism.md
//! `## Invariants` section 参照。
//!
//! **Lesson source (I-205 deep deep review F-deep-deep-2 + I-224 5 rounds adversarial review
//! convergence)**: invariant verification tests を PRD `### Invariants` section の SPEC TEXT
//! のみで record、actual Rust test code 不在 = "deferred verification = unverified claim"
//! compromise を排除する Spec stage convention。本 file は I-224 Spec stage true closure の
//! 最終 artifact (Rule 9 (a) helper test contracts NEW + Rule 8 (8-c) audit symmetry)。

use ts_to_rs::parser::parse_typescript;
use ts_to_rs::transformer::main_synthesis::{detect_user_main, has_top_level_await, UserMainKind};
use ts_to_rs::transpile;

/// INV-1: TS execution order = Rust execution order
///
/// **Property statement (a)**: Cell A != A0 (= top-level execution 存在) の全 cell で、
/// TS module top-level statements の execution order が Rust `fn main()` body 内で
/// **byte-exact preserve** される。Hoisted function declarations は Rust 上で全 fn main
/// 外に配置されるが、user 視点の execution semantic (= top-level stmts 順序通り、`main();`
/// call site も順序通り) は preserve。
///
/// **Verification (c)**: Per-cell E2E fixture で TS / Rust stdout の byte-exact match
/// を verify (TS-3 で fixture 作成、T5/T9 で green 化)。本 stub は INV-1 contract の
/// integration-level lock-in (= matrix in-scope cells の representative subset で
/// transpile + cargo run + tsc/tsx stdout byte-exact match)。
#[test]
#[ignore = "I-224 INV-1 verification stub: Implementation Stage T5/T9 で fill in \
            (cells 11/13/15/16/31/33/35/36/71/73/76 fixtures で TS stdout vs cargo run stdout \
            byte-exact match)"]
fn test_invariant_1_ts_rust_execution_order_byte_exact() {
    let _ = transpile;
    unimplemented!(
        "Spec stage stub、Implementation Stage T5/T9 で fill in: \
         representative in-scope cells で TS source → tsc/tsx stdout vs ts_to_rs → cargo run \
         stdout の byte-exact match を assert (INV-1 source-order preservation invariant)"
    );
}

/// INV-2: User `main` symbol semantic preservation + multi-call substitution sub-case
///
/// **Property statement (a)**: User-defined `main` symbol (Axis B != B0) は **Rust 側で
/// 参照可能な状態** で preserve される。B1/B2 (function form) → `__ts_main` で rename +
/// 全 user-side `main()` call site を `__ts_main()` (sync) or `__ts_main().await` (async)
/// に substitute。B3 (non-fn symbol) → name preserved (Rust namespace 別)。B4 (collision)
/// → Tier 2 honest reject。
///
/// **Verification (c)**: in-scope cells 13/14/15/16/33/34/35/36/73/74/75/76 fixture で
/// user `main()` call site が `__ts_main()` (or `__ts_main().await`) に substitute される
/// ことを fixture probe + IR token-level test で verify。
///
/// **Multi-call boundary value sub-case (H-7 Fix B)**: cell-31 fixture (A1+B1+C0 with
/// `main(); main();` form) を cell #13 boundary value test fixture として keep、user main
/// の multiple call sites が全 `__ts_main()` に substitute されることを probe。
#[test]
fn test_invariant_2_user_main_symbol_preservation_with_multi_call_subcase() {
    // INV-2 (B1/B2 cells): the user-defined `main` symbol must be renamed
    // to `__ts_main` AND every user-source `main()` call must be substituted
    // to `__ts_main()`. Verified at the IR-text level (= post-transpile
    // Rust string) for the in-scope B1/B2 cells in the C0 partition.
    //
    // **Async substitute (cells 15/16/35/36/75/76)**: T3-3 implements the
    // sync substitute (= rewrite the callee identifier). Async-await
    // wrapping (= adding `.await` after the substituted `__ts_main()`) is
    // T8 work; until then, B2 cells produce `__ts_main()` invocation
    // without `.await`, which compiles correctly inside the synthesized
    // `fn main()` body (T4-1 ExecFnSyncRename / ExecFnAsyncRename arm) but
    // will need T8 to upgrade once `#[tokio::main] async fn main` synthesis
    // lands the `.await` wrapping for B2 cells. This test asserts the
    // **identifier substitution** invariant only — IR shape of the await
    // wrapping is tested separately in T8's INV-3 full-coverage extension.
    //
    // Each entry = (cell #, axis summary, TS source).
    let cells: &[(u32, &str, &str)] = &[
        // ===== Executable A1 + B1 sync user main + C0 =====
        (13, "A1+B1+C0", "function main(): void { }\nmain();\n"),
        (33, "A3+B1+C0", "declare function f(): number;\nfunction main(): void { }\nconst c = f();\nmain();\n"),
        (73, "A6+B1+C0", "const X: number = 1;\nfunction main(): void { }\nmain();\nconsole.log(X);\n"),
        // ===== Executable + B2 async user main + C0 (sync substitute baseline) =====
        (15, "A1+B2+C0", "async function main(): Promise<void> { }\nmain();\n"),
        (35, "A3+B2+C0", "declare function f(): number;\nasync function main(): Promise<void> { }\nconst c = f();\nmain();\n"),
        (75, "A6+B2+C0", "const X: number = 1;\nasync function main(): Promise<void> { }\nmain();\nconsole.log(X);\n"),
    ];

    for (cell, axis_summary, src) in cells {
        let rust = transpile(src)
            .unwrap_or_else(|e| panic!("cell #{cell} ({axis_summary}): transpile failed: {e}"));
        // 1. User main is renamed at the declaration site AND a synthesized
        //    `fn main()` is emitted as the binary entry (T4-1 wiring):
        //    - sync B1 (cells 13 / 33 / 73): `fn __ts_main` exists; the
        //      synthesized `fn main()` (sync, no `#[tokio::main]`) wraps the
        //      captured top-level `main();` call as `__ts_main();`.
        //    - async B2 (cells 15 / 35 / 75): `async fn __ts_main` exists;
        //      the synthesized `#[tokio::main] async fn main()` wraps the
        //      captured top-level call as `__ts_main();` (T8 will upgrade to
        //      `__ts_main().await` once async wrapping lands).
        //    Both: the renamed user main must be present AND the synthesized
        //    binary entry must be present (= INV-1 source-order + dispatch
        //    arm structural compliance).
        assert!(
            rust.contains("fn __ts_main"),
            "cell #{cell} ({axis_summary}): expected `fn __ts_main` declaration, got:\n{rust}"
        );
        assert!(
            rust.contains("fn main()"),
            "cell #{cell} ({axis_summary}): expected synthesized `fn main()` binary \
             entry (T4-1 ExecFnSyncRename / ExecFnAsyncRename arm), got:\n{rust}"
        );
        // 2. Every user-source `main()` call is substituted to `__ts_main()`.
        //    The TS source contains a single `main();` call — the IR must
        //    contain a `__ts_main()` invocation reference. (The original
        //    user-source `main();` call is removed by the substitution; if
        //    a bare `main()` reference leaked through, that would indicate
        //    the substitution gate failed to fire.)
        assert!(
            rust.contains("__ts_main()"),
            "cell #{cell} ({axis_summary}): expected substituted `__ts_main()` call site, got:\n{rust}"
        );
        // Cross-axis sanity: TypeResolver / TypeRegistry layer must classify
        // the user main correctly so the rename gate fires only for B1/B2.
        let module = parse_typescript(src)
            .unwrap_or_else(|e| panic!("cell #{cell} ({axis_summary}): parse failed: {e}"));
        let kind = detect_user_main(&module);
        assert!(
            matches!(kind, UserMainKind::FnSync | UserMainKind::FnAsync),
            "cell #{cell} ({axis_summary}): detect_user_main returned {kind:?}, \
             expected FnSync or FnAsync (= rename gate trigger)"
        );
        // C-axis sanity: these cells are C0 (= no top-level await).
        assert!(
            !has_top_level_await(&module),
            "cell #{cell} ({axis_summary}): unexpected top-level await — fixture corruption"
        );
    }

    // ===== Multi-call boundary value sub-case (cell 31 PRD-named, A3+B1+C0
    // with explicit multi-`main()` calls). Per the PRD H-7 Fix B
    // wording: "user main の multiple call sites が全 `__ts_main()` に
    // substitute される". Counts ≥ 2 substituted occurrences in the
    // post-transpile source; the original user-source `main();` calls have
    // all been rewritten — no bare `main();` invocation must remain. =====
    let multi_call_src = "\
function main(): void {\n\
  console.log(\"a\");\n\
}\n\
main();\n\
console.log(\"between\");\n\
main();\n\
main();\n";
    let multi_call_rust =
        transpile(multi_call_src).expect("multi-call boundary fixture must transpile successfully");
    let substituted = multi_call_rust.matches("__ts_main()").count();
    // 3 user-source `main();` call sites + the user main DEFINITION must
    // produce ≥ 3 `__ts_main()` call-site occurrences (the definition
    // itself contains `fn __ts_main` not `__ts_main()`, so the count
    // captures call sites only).
    assert!(
        substituted >= 3,
        "multi-call boundary: expected ≥ 3 substituted `__ts_main()` call sites, got {substituted} in:\n{multi_call_rust}"
    );
    // No bare `main();` call-site leak (= un-substituted user-source call).
    // Post-rustfmt every body-level call has 4-space indent + semicolon, so a
    // leaked un-substituted call appears as `    main();`. Note: the broader
    // ` main()` substring check (used pre-T4-1) cannot be applied any longer
    // because the synthesized `fn main()` (T4-1 binary entry) declaration
    // legitimately contains ` main()` — checking against the call-site form
    // (with semicolon + indent) keeps the leak detection precise without
    // false positives from the declaration form.
    assert!(
        !multi_call_rust.contains("    main();"),
        "multi-call boundary: bare `main();` call leaked — substitution gate did not \
         fire for every site, got:\n{multi_call_rust}"
    );
}

/// INV-3: Sync / async dispatch consistency (4 sub-case verification)
///
/// **Property statement (a)** (iteration v3 Axis C1 in-scope 反映 wording、iteration v6
/// minor library mode `fn main directly emit` cells 反映): 全 in-scope cell で fn main
/// の sync / async dispatch が以下の trigger 集合のいずれか 1 つ以上が満たされたら
/// `#[tokio::main] async fn main` (= async dispatch)、全て不在なら sync `fn main` で
/// exhaustive + mutually exclusive 決定:
/// - **Trigger 1**: Axis B B2 (= user-defined async function main)
/// - **Trigger 2**: Axis C C1 (= top-level await present)
///
/// **Verification (c)** (iteration v3〜v6 minor で確定の 4 sub-case lists):
/// - **Trigger 1 only** (B2 only、C0): cells 5/15/25/35/55/75 で `#[tokio::main] async fn main` 出力
/// - **Trigger 2 only** (C1 + non-FnAsync): cells 12/14/18/32/34/38/72/74/78 で 同上
/// - **Trigger 1 + 2 combined** (B2 + C1): cells 16/36/76 で 同上 + 重複 attribute 不在
/// - **Sync (no trigger、`fn main` emission cells)**: cells 3/11/13/17/23/31/33/37/71/73/77 で plain `fn main` (or `pub fn main` for E1)
/// - **Edge sub-case (Trigger 2 only with sync user main)**: cells 14/34/74 で `#[tokio::main] async fn main` + sync `__ts_main()` 非 await call wrapping
/// - **Library mode no-fn-main cells (INV-3 scope 外)**: cells 1/7/21/27 = `fn main` 自体 emit しない
///
/// **T2 partial fill-in (C0 cells only)**: verifies that for each in-scope C0
/// cell, the predicate-derived `is_async_required` flag (= `user_main_kind ==
/// FnAsync || has_top_level_await`) matches the expected value per INV-3 (c)
/// sub-case lists (Trigger 1 only / Sync). Trigger 2 and Trigger 1+2 cells
/// (= the C1 cells 12/14/16/18/32/34/36/38/72/74/76/78) require Implementation
/// Stage T8's top-level await synthesis logic before they can be fully verified
/// against IR emission, so they are deferred to T8 fill-in.
///
/// **Library mode no-fn-main cells**: cells 1/7/21/27 do not emit a `fn main`
/// at all (library mode + B0/B3) — INV-3 (a) "fn main の sync/async dispatch"
/// does not apply, so they are intentionally not in this table. Their
/// library-mode classification is locked in by
/// `tests/i224_helper_test.rs::test_dispatch_arm_one_to_one_mapping_per_in_scope_cell`.
#[test]
fn test_invariant_3_sync_async_dispatch_consistency_4_subcases() {
    // Each entry = (cell #, axis summary, TS source, expected is_async_required).
    //
    // is_async_required = (user_main_kind == FnAsync) || has_top_level_await.
    // Per INV-3 (a), this boolean drives `fn main` vs `#[tokio::main] async fn main`
    // emission once T3's `synthesize_fn_main` is integrated.
    let c0_cases: &[(u32, &str, &str, bool)] = &[
        // ---- Trigger 1 only (B2 + C0) → is_async_required=true ----
        (
            5,
            "A0+B2+C0",
            "async function main(): Promise<void> { }\n",
            true,
        ),
        (
            15,
            "A1+B2+C0",
            "async function main(): Promise<void> { }\nconsole.log('hi');\n",
            true,
        ),
        (
            25,
            "A2+B2+C0",
            "const x: number = 0;\nasync function main(): Promise<void> { }\n",
            true,
        ),
        (
            35,
            "A3+B2+C0",
            "declare function f(): number;\n\
             async function main(): Promise<void> { }\nconst c = f();\n",
            true,
        ),
        // Cell 55 (A5a+B2+C0) is orthogonality-merged with cells 5 + 51; A5a Empty
        // is silent in is_executable_mode, so this dispatches to LibraryFnAsyncDirect
        // (= directly emits `#[tokio::main] async fn main`). Same is_async_required=true.
        (
            55,
            "A5a+B2+C0",
            "async function main(): Promise<void> { }\n;\n",
            true,
        ),
        (
            75,
            "A6+B2+C0",
            "const X: number = 1;\n\
             async function main(): Promise<void> { }\nconsole.log(X);\n",
            true,
        ),
        // ---- Sync (no trigger, C0) → is_async_required=false ----
        (3, "A0+B1+C0", "function main(): void { }\n", false),
        (11, "A1+B0+C0", "console.log('hi');\n", false),
        (
            13,
            "A1+B1+C0",
            "function main(): void { }\nconsole.log('hi');\n",
            false,
        ),
        (
            17,
            "A1+B3+C0",
            "interface main { x: number; }\nconsole.log('hi');\n",
            false,
        ),
        (
            23,
            "A2+B1+C0",
            "const x: number = 0;\nfunction main(): void { }\n",
            false,
        ),
        (
            31,
            "A3+B0+C0",
            "declare function f(): number;\nconst c = f();\n",
            false,
        ),
        (
            33,
            "A3+B1+C0",
            "declare function f(): number;\n\
             function main(): void { }\nconst c = f();\n",
            false,
        ),
        (
            37,
            "A3+B3+C0",
            "declare function f(): number;\n\
             interface main { x: number; }\nconst c = f();\n",
            false,
        ),
        (
            71,
            "A6+B0+C0",
            "const X: number = 1;\nconsole.log(X);\n",
            false,
        ),
        (
            73,
            "A6+B1+C0",
            "const X: number = 1;\nfunction main(): void { }\nconsole.log(X);\n",
            false,
        ),
        (
            77,
            "A6+B3+C0",
            "const X: number = 1;\ninterface main { x: number; }\nconsole.log(X);\n",
            false,
        ),
    ];
    for (cell, axis_summary, src, expected_async) in c0_cases {
        let module = parse_typescript(src)
            .unwrap_or_else(|e| panic!("cell #{cell} ({axis_summary}): SWC parse failed: {e}"));
        let user_main_kind = detect_user_main(&module);
        let await_flag = has_top_level_await(&module);
        let is_async_required = matches!(user_main_kind, UserMainKind::FnAsync) || await_flag;
        assert_eq!(
            is_async_required, *expected_async,
            "cell #{cell} ({axis_summary}): is_async_required mismatch \
             (user_main_kind={user_main_kind:?}, has_top_level_await={await_flag})"
        );
    }
}

/// INV-4: `pub fn init` mechanism 廃止 invariant
///
/// **Property statement (a)**: I-224 完了後、ts_to_rs production code 内 (= `src/`,
/// `tools/`, `tests/e2e/rust-runner/`) に `pub fn init(...)` の **function definition**
/// が存在しない (= 全 emission path が `fn main` 統合 or library mode に migration)。
///
/// **Verification (c)**: 本 test は **2 つ独立 verifier** で structural lock-in:
/// 1. `scripts/audit-no-pub-fn-init.sh` を subprocess invoke、`exit=0` を assert
///    (audit script は declaration-shape pattern `^\s*pub\s+fn\s+init\s*[\(<]` で
///    function definition のみ match、doc comment / panic message での `pub fn init`
///    言及は false positive にならない)
/// 2. Rust source 内 `^\s*pub\s+fn\s+init\s*[\(<]` を直接 grep、enforced paths
///    (`src/`, `tools/`, `tests/e2e/rust-runner/`) で hits == 0 を assert
///
/// **Why two verifiers**: audit script の bug / disable / 削除 が起きても、独立 grep が
/// invariant violation を捕捉する。逆も同様。
///
/// **Advisory paths exclusion rationale**: snapshot artefacts under
/// `tests/e2e/scripts/i-205/cell-*.rs` are pre-T4-1 generator output (`pub fn init()`
/// from the legacy mechanism). These are not authoritative — re-running the e2e
/// suite regenerates them under the new `fn main` mechanism. The audit script
/// reports them as advisory hits (no exit=1) and the direct grep below scopes its
/// pattern to the same enforced paths the audit uses.
///
/// **T4-1 + T4-2 + T4-3 fill-in (post production wiring)**: T4-1 retired the
/// `build_init_fn` production helper, T4-2 expanded `transform_module_item`'s `_`
/// arm with Rule 11 (d-1) compliance + Tier 2 wording improvements, and T4-3 fills
/// in this test to lock the invariant in.
#[test]
fn test_invariant_4_no_pub_fn_init_in_codebase_post_t4() {
    use std::process::Command;

    // Locate workspace root (= the directory containing `Cargo.toml` /
    // `scripts/audit-no-pub-fn-init.sh`). `CARGO_MANIFEST_DIR` is set by
    // cargo at compile time and points to the package root, which equals the
    // workspace root for this single-crate workspace.
    let workspace_root = env!("CARGO_MANIFEST_DIR");

    // === Verifier 1: audit script subprocess invoke ===
    let audit_script = format!("{workspace_root}/scripts/audit-no-pub-fn-init.sh");
    let audit_output = Command::new(&audit_script)
        .current_dir(workspace_root)
        .output()
        .unwrap_or_else(|e| panic!("INV-4: failed to spawn audit script `{audit_script}`: {e}"));
    let stdout = String::from_utf8_lossy(&audit_output.stdout);
    let stderr = String::from_utf8_lossy(&audit_output.stderr);
    assert!(
        audit_output.status.success(),
        "INV-4: `audit-no-pub-fn-init.sh` returned non-zero exit (= function-definition \
         leak detected). Status: {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        audit_output.status,
    );
    assert!(
        stdout.contains("OK: 0 hits of `pub fn init`"),
        "INV-4: audit script success status but expected `OK: 0 hits` summary line was \
         not found. stdout:\n{stdout}",
    );

    // === Verifier 2: independent grep (= structural redundancy) ===
    //
    // Walk the enforced paths (= the same set the audit script uses) and search
    // for `^\s*pub\s+fn\s+init\s*[\(<]` matches in `*.rs` files. Use grep -rEn
    // for portability (the harness already requires bash for the audit script,
    // so grep is universally available).
    let enforced_paths = ["src", "tools", "tests/e2e/rust-runner"];
    for path in &enforced_paths {
        let abs = format!("{workspace_root}/{path}");
        if !std::path::Path::new(&abs).exists() {
            // tools/ may be absent; skip silently per the audit script's
            // existence guard.
            continue;
        }
        let grep = Command::new("grep")
            .args([
                "-rEn",
                "--include=*.rs",
                r"^\s*pub\s+fn\s+init\s*[\(<]",
                &abs,
            ])
            .output()
            .unwrap_or_else(|e| panic!("INV-4: failed to spawn grep for `{abs}`: {e}"));
        let hits = String::from_utf8_lossy(&grep.stdout);
        // grep exit code:
        //   0 = matches found (= violation) — must NOT happen.
        //   1 = no matches (= invariant holds) — expected.
        //   2 = error (e.g., bad regex) — fail loudly.
        match grep.status.code() {
            Some(0) => panic!(
                "INV-4: independent grep found `pub fn init(...)` definition(s) in `{path}`:\n{hits}",
            ),
            Some(1) => { /* no matches — invariant holds for this path */ }
            other => panic!(
                "INV-4: grep returned unexpected status {other:?} for `{abs}`. \
                 stdout:\n{hits}\nstderr:\n{}",
                String::from_utf8_lossy(&grep.stderr)
            ),
        }
    }
}

/// INV-5: `__ts_` namespace reservation extension consistency
///
/// **Property statement (a)**: I-154 `__ts_` namespace reservation rule に `__ts_main` が
/// 追加 + 全 user identifier validation path で `__ts_main` を reserved 検出、collision
/// case (= matrix # 9/19/20 + collision-merged cells 29/39/40/49/59/69/79/80) で
/// Tier 2 honest error reject。
///
/// **Verification (c)**: I-154 namespace reservation test 拡張で `__ts_main` reserved
/// verify、collision detection unit test、matrix # 9/19/20 fixture probe、`__ts_main`
/// empirical pre-existing user-code audit (R-4 task = TS-7 で実施完了、codebase + Hono
/// grep `__ts_main` 0 hits 確認) で Tier-transition prerequisite 担保。
///
/// **Collision priority verification (structural)**: Implementation 上、collision detection
/// は `Transformer::transform_module_collecting` の最先頭 (= per-item dispatch + A-axis /
/// C-axis routing 開始前) で `scan_for_ts_namespace_collisions` を call し
/// `unsupported` accumulator に seed する。`transpile` 公開 API は `unsupported.first()`
/// を Err として bail するため、collision を含む source では公開 API 視点で
/// **「最初に報告される Err == collision wording」** が constructive proof。これにより
/// dispatch tree `(_, Collision, _)` arm が A/C 軸 dispatch より先行 reject される (=
/// cells 49/59/69 の A4 control-flow / A5a Empty / A5b Debugger reject よりも collision
/// wording で reject される) 不変性を public API 経由で empirical lock-in する。
///
/// 補強として、cells 49/59/69 (A4 control-flow / A5a Empty / A5b Debugger) では
/// `transpile_collecting` 経由で **alternative reject (`Stmt(If(`、`Stmt(Empty(`、
/// `Stmt(Debugger(` wording) も unsupported list に存在しうる** が collision が
/// `unsupported[0]` に置かれる (= priority arm が先) ことを別途 assert (= non-trivial
/// priority lock-in)。当該 alternative wording は `format_module_item_kind` の
/// `format!("Stmt({stmt:?})")` envelope に依存するため、`Stmt(<Variant>(` 形式で
/// substring match (swc Debug 形式の偶然依存を排除)。
///
/// **Cell 59 forward-compat note**: 現在 (post T1-2 / pre T2/T3) `Stmt::Empty` は
/// `transform_module_item` 末尾の `_ => Err(format_module_item_kind(...))` arm で
/// reject される (alternative wording = `Stmt(Empty(`)。Implementation Stage T2/T3
/// で A5a (Empty) の silent-skip 実装が landed すると alternative wording は
/// 消失する → 本 test の cell 59 alternative-reject 行 (`59 => "Stmt(Empty("`) を
/// その時点で除去 (= intended test evolution、structural drift ではない)。<br/>
/// 本 evolution の trigger は PRD doc Implementation Stage Tasks > T4-2 の
/// `transform_module_item` `_` arm refactor + A5a silent-skip 実装で発火、
/// **本 test を `Stmt(Empty(` 検出から `unsupported len == 1` 検出 (= vacuous
/// priority) に切替** することが ideal implementation。T1-3 時点では現状を
/// structurally lock-in する。
#[test]
fn test_invariant_5_ts_main_namespace_reservation_with_collision_priority() {
    use ts_to_rs::transpile_collecting;

    // Each entry = (matrix cell #, axis summary, TS source). The axis summary documents
    // the (Axis A, Axis B=B4 collision, Axis C) shape so a reader can map a failing case
    // back to the PRD matrix without re-deriving the tuple.
    //
    // **Source construction principle**: every source carries a top-level
    // `function __ts_main()` declaration (= Axis B4 marker), plus an Axis-A-shaped body
    // (declarations only / Stmt::Expr / Decl::Var lit-init / Decl::Var side-effect-init /
    // control-flow / Stmt::Empty / Stmt::Debugger / mixed) and optionally a top-level
    // `await` expression (Axis C1). C1 cells include a `declare const ...: Promise<_>`
    // so SWC parses the await expression in a self-consistent context.
    let cases: &[(u32, &str, &str)] = &[
        // Cell 9: A0 (declarations only / library mode) + B4 + C0
        (
            9,
            "A0+B4+C0 (library-form `__ts_main` collision, no top-level execution)",
            "function __ts_main(): void { console.log('user __ts_main'); }\n",
        ),
        // Cell 19: A1 (top-level Stmt::Expr only) + B4 + C0
        (
            19,
            "A1+B4+C0 (Stmt::Expr top-level + collision)",
            "function __ts_main(): void { console.log('user'); }\n\
             console.log('top-level');\n\
             __ts_main();\n",
        ),
        // Cell 20: A1 + B4 + C1 (top-level await)
        (
            20,
            "A1+B4+C1 (Stmt::Expr top-level + collision + top-level await)",
            "declare const p: Promise<number>;\n\
             function __ts_main(): void { console.log('user'); }\n\
             console.log('top-level');\n\
             await p;\n",
        ),
        // Cell 29: A2 (Decl::Var with literal init only) + B4 + C0
        (
            29,
            "A2+B4+C0 (Lit-init top-level const + collision)",
            "function __ts_main(): void { console.log('user'); }\n\
             const x: number = 0;\n",
        ),
        // Cell 39: A3 (Decl::Var with side-effect / non-const init) + B4 + C0
        (
            39,
            "A3+B4+C0 (side-effect-init top-level const + collision)",
            "declare function fetchSync(): number;\n\
             function __ts_main(): void { console.log('user'); }\n\
             const c: number = fetchSync();\n",
        ),
        // Cell 40: A3 + B4 + C1 (top-level await as init)
        (
            40,
            "A3+B4+C1 (await-init top-level const + collision)",
            "declare function fetchAsync(): Promise<number>;\n\
             function __ts_main(): void { console.log('user'); }\n\
             const c: number = await fetchAsync();\n",
        ),
        // Cell 49: A4 (control-flow at top-level) + B4 + C0
        (
            49,
            "A4+B4+C0 (top-level control-flow + collision; collision precedes A4 reject)",
            "function __ts_main(): void { console.log('user'); }\n\
             if (true) { console.log('top-if'); }\n",
        ),
        // Cell 59: A5a (Stmt::Empty) + B4 + C0
        (
            59,
            "A5a+B4+C0 (top-level empty stmt + collision; collision precedes A5a path)",
            "function __ts_main(): void { console.log('user'); }\n\
             ;\n",
        ),
        // Cell 69: A5b (Stmt::Debugger) + B4 + C0
        (
            69,
            "A5b+B4+C0 (top-level debugger + collision; collision precedes A5b reject)",
            "function __ts_main(): void { console.log('user'); }\n\
             debugger;\n",
        ),
        // Cell 79: A6 (mixed) + B4 + C0
        (
            79,
            "A6+B4+C0 (mixed top-level + collision)",
            "function __ts_main(): void { console.log('user'); }\n\
             const x: number = 0;\n\
             console.log('mixed');\n",
        ),
        // Cell 80: A6 + B4 + C1 (mixed + top-level await)
        (
            80,
            "A6+B4+C1 (mixed top-level + collision + top-level await)",
            "declare const p: Promise<number>;\n\
             function __ts_main(): void { console.log('user'); }\n\
             const x: number = 0;\n\
             console.log('mixed');\n\
             await p;\n",
        ),
    ];

    // Cells whose A-axis dispatch alone (i.e., without collision detection) would
    // produce a non-collision Tier 2 reject in the **current** transformer state
    // (post T3-4 silent-skip of Stmt::Empty, pre T4-2 full _ arm refactor). Used
    // for the structural priority sub-check: collision must be at
    // `unsupported[0]` even when these alternative rejects would otherwise fire.
    //
    // **Cell 59 evolution (T3-4 fix landed)**: T3-4 added `Stmt::Empty` silent
    // skip in `transform_module` / `transform_module_collecting` per PRD Design
    // section #3 ("A5a partition: silent skip per the per-item dispatch table").
    // Cell 59's `;` no longer produces an alternative reject — only the
    // collision is reported. The cell is therefore removed from this list; the
    // primary check (= collision at unsupported[0]) above still verifies the
    // INV-5 priority for cell 59. The `cells_with_alternative_a_axis_reject`
    // entries remaining (49 / 69) are for A4 (control-flow) and A5b (Debugger),
    // which still produce A-axis rejects in the pre-T4-2 state.
    let cells_with_alternative_a_axis_reject: &[u32] = &[49, 69];

    for (cell, axis_summary, src) in cases {
        // (1) Public API: `transpile` must Err with collision wording.
        //     Because `transpile` reports `unsupported.first()`, this transitively
        //     proves collision is at index 0 (= structural priority).
        let abort_result = transpile(src);
        let err = match abort_result {
            Ok(rust) => panic!(
                "cell #{cell} ({axis_summary}): expected Tier 2 honest reject for `__ts_main` \
                 collision, but transpile() returned Ok with rust source:\n{rust}"
            ),
            Err(e) => e,
        };
        let err_msg = format!("{err:#}");
        assert!(
            err_msg.contains("__ts_main") && err_msg.contains("is reserved"),
            "cell #{cell} ({axis_summary}): expected error message to contain `__ts_main` and \
             `is reserved` (collision wording per `check_ts_internal_fn_name_namespace`); got: \
             `{err_msg}`"
        );

        // (2) Collecting API: structural lock-in that `unsupported[0]` is the collision.
        //     This is the primary INV-5 priority statement: collision is seeded into the
        //     accumulator before per-item dispatch runs.
        let (_rust_partial, unsupported) = transpile_collecting(src).unwrap_or_else(|e| {
            panic!("cell #{cell} ({axis_summary}): transpile_collecting failed unexpectedly: {e}")
        });
        let first = unsupported.first().unwrap_or_else(|| {
            panic!(
                "cell #{cell} ({axis_summary}): expected collision in unsupported list, got empty list"
            )
        });
        assert!(
            first.kind.contains("__ts_main") && first.kind.contains("is reserved"),
            "cell #{cell} ({axis_summary}): expected unsupported[0] to be the collision wording \
             (= structural priority of `(_, Collision, _)` arm over A/C-axis dispatch); got \
             unsupported[0].kind = `{}`, full list = {unsupported:?}",
            first.kind,
        );

        // (3) Priority sub-check for cells 49/59/69: their A-axis would otherwise produce
        //     a non-collision reject (in the current state, see the function docstring's
        //     "Cell 59 forward-compat note" for cell 59's planned evolution). We verify
        //     the alternative wording IS present in the unsupported list (proving the
        //     source is non-trivially rejected by A-axis as well), but NOT at index 0
        //     (proving collision wins).
        //
        //     Substring form `Stmt(<Variant>(` matches the `format!("Stmt({stmt:?})")`
        //     envelope produced by `format_module_item_kind` (swc's auto-derived Debug
        //     prints tuple-struct variants as `<Variant>(<inner>)`).
        if cells_with_alternative_a_axis_reject.contains(cell) {
            // T4-2 wording substrings:
            // - Cell 49 (A4 control-flow): the new wording wraps the SWC kind in
            //   parentheses, so the legacy `Stmt(If(` substring still matches as
            //   part of the suffix.
            // - Cell 69 (A5b Debugger): the new wording replaces the SWC kind
            //   prefix with user-facing guidance; we anchor on the leading
            //   backtick-quoted token instead.
            let alternative_wording = match cell {
                49 => "Stmt(If(",
                69 => "`debugger` statement has no Rust equivalent",
                _ => unreachable!(
                    "cells_with_alternative_a_axis_reject must be kept in sync with this match arm"
                ),
            };
            let has_alternative = unsupported
                .iter()
                .any(|u| u.kind.contains(alternative_wording));
            assert!(
                has_alternative,
                "cell #{cell} ({axis_summary}): expected unsupported list to also contain the \
                 A-axis alternative reject wording (substring `{alternative_wording}`) so that \
                 the priority assertion is non-trivial; got unsupported list = {unsupported:?}"
            );
            // The alternative reject must NOT be at index 0 — that slot is reserved for
            // the collision (= INV-5 structural priority).
            assert!(
                !first.kind.contains(alternative_wording),
                "cell #{cell} ({axis_summary}): unsupported[0] unexpectedly contains the \
                 A-axis alternative wording (`{alternative_wording}`) — collision must be \
                 at index 0, not the A-axis reject; got unsupported[0].kind = `{}`",
                first.kind,
            );
        }
    }
}

/// INV-6: TypeResolver layer unaffected (third-party review R-3 source)
///
/// **Property statement (a)**: 本 PRD の fn main synthesis + user main rename + main()
/// call substitute logic は **TypeResolver layer の type resolution flow に影響しない**。
/// 具体的に: TypeResolver はモジュール内の type binding / expr_type lookup / narrowing
/// 等を処理する pipeline phase であり、本 PRD の identifier rename (`main` → `__ts_main`)
/// は AST transform stage (post-TypeResolver) で完結。TypeResolver 入力 (= `Module` AST)
/// の identifier text を user-defined のまま保持、TypeResolver は `main` を user fn として
/// 既に正しく resolve、本 PRD は post-resolution AST に rename を後付け。
///
/// **Verification (c)**:
/// - 既存 TypeResolver unit tests が本 PRD changes で全 pass
/// - Implementation Stage T2 着手前 empirical probe (`cargo test --lib pipeline::type_resolver::`
///   全 pass、`fn detect_user_main` の input/output で TypeResolver field を touch しない code review)
/// - dispatch logic 内に TypeResolver 呼び出しが新規追加されていないことを Code review
///   (Layer 1 Mechanical) で audit
///
/// **T2 fill-in**: structurally verifies INV-6 (TypeResolver layer unaffected).
///
/// **Verification strategy**: INV-6's two verification methods (per the property's
/// `(c)` clause) compose into a sufficient structural lock-in via method (2):
/// - **(1) Existing TypeResolver tests pass**: the lib build's
///   `pipeline::type_resolver::` test module runs as part of every `cargo test`
///   invocation. If those tests fail, the entire test suite fails — including this
///   test cannot run in a passing state. The structural argument is therefore
///   "this test running to completion in a green suite implies (1)".
///   `cargo test` subprocess invocation from inside a test would deadlock on the
///   build directory lock and recurse on the test runner; the existing global
///   harness already provides the same coverage without that risk.
/// - **(2) main_synthesis.rs has no TypeResolver field references**: this test
///   directly inspects the source content. (2) is the *sufficient* condition for
///   INV-6 — without TypeResolver field references in the I-224-introduced module,
///   no T3/T4 emission code can introduce a TypeResolver-affecting code path.
///   This is the load-bearing structural invariant.
///
/// **Forbidden tokens** are the field / type names that, if present in
/// `main_synthesis.rs`, would constitute a TypeResolver dependency: the
/// `TypeResolver` struct, the `type_registry` / `expr_type` / `narrowing` /
/// `expected_type` access fields, and the `FileTypeResolution` resolution table.
/// Comments mentioning these names in module-level docstrings would also trip the
/// substring match — the test source intentionally uses no such references in
/// `main_synthesis.rs`'s file content (only doc references via Markdown links to
/// other modules, which do not include the forbidden tokens verbatim).
#[test]
fn test_invariant_6_type_resolver_layer_unaffected() {
    // CARGO_MANIFEST_DIR is the workspace root (= where Cargo.toml lives), which
    // is the parent of `src/`.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // `main_synthesis` was originally a single file `main_synthesis.rs`; once the
    // file-line check threshold was reached during T2 implementation, it was split
    // into a directory module (`main_synthesis/mod.rs` for production +
    // `main_synthesis/tests.rs` for unit tests). INV-6's structural verification
    // applies to the production code only — the cfg(test)-gated tests file is not
    // compiled into release builds and cannot affect the TypeResolver layer at
    // runtime, so we audit `mod.rs` exclusively.
    let path = std::path::Path::new(manifest_dir).join("src/transformer/main_synthesis/mod.rs");
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "INV-6 verification (2): failed to read main_synthesis/mod.rs at {}: {e}",
            path.display()
        )
    });

    // Forbidden tokens: TypeResolver-layer fields / types. If any of these appear,
    // main_synthesis.rs has crossed the layer boundary and INV-6 is violated.
    //
    // Note on `expected_type`: `Transformer::convert_expr` reads the expected-type
    // map from `FileTypeResolution` indirectly via the TransformContext; the field
    // is never accessed by name from `main_synthesis.rs` (we only call `convert_expr`
    // / `convert_var_decl` on `&mut Transformer`). Forbidding `expected_type` as a
    // raw substring catches accidental direct access.
    // Use **specific** TypeResolver-layer identifiers (struct names, field names,
    // sibling-module names) rather than generic English words. Substring matching
    // is the simplest robust mechanism, but a token like `"narrowing"` would
    // produce false positives whenever the module uses the English word
    // "narrowing" / "narrow" in unrelated contexts (e.g., a doc comment about
    // "narrowing a Lit match"). Replacing it with the concrete TypeResolver
    // field name `narrowed_type` and sibling module `narrowing_analyzer`
    // preserves the structural intent while eliminating the false-positive class.
    let forbidden_tokens: &[&str] = &[
        "TypeResolver",
        "type_resolver",
        "FileTypeResolution",
        "type_resolution", // TransformContext field accessing FileTypeResolution
        "type_registry",
        "expr_type",
        "narrowed_type",      // TypeResolver narrowing data
        "narrowing_analyzer", // TypeResolver sibling module
        "expected_type",
    ];
    for token in forbidden_tokens {
        assert!(
            !source.contains(token),
            "INV-6 verification (2): main_synthesis.rs unexpectedly contains TypeResolver-layer \
             token `{token}` — this introduces a layer-crossing dependency that violates \
             INV-6's structural separation. Either remove the reference or revise INV-6 (a)."
        );
    }
}

/// INV-7: `pub fn init` mechanism 廃止の external API audit (third-party review R-2 source)
///
/// **Property statement (a)**: `pub fn init` mechanism 廃止は ts_to_rs の generated Rust
/// code の external API breaking change である (= user / downstream test が generated Rust
/// 上 `init()` を call する case が存在すれば compile fail)。本 PRD で codebase + Hono +
/// 既存 e2e test 全体で `init()` call site の empirical audit を完了し、breaking change
/// の実 reachability を 0 件に確定。
///
/// **Verification (c)**:
/// - Codebase grep: `grep -rn '\binit\s*(' src/ tests/ tools/` で ts_to_rs side の
///   `init()` call site enumerate (TS-7 で実施済 = 0 件)
/// - Hono codebase grep: `grep -rn '\binit\s*(' /tmp/hono*` で 3rd party `init()` call site
///   enumerate (Implementation Stage T5 で Hono bench Tier-transition compliance verify
///   時に 0 件 confirm 予定)
/// - e2e test runner: `tests/e2e_test.rs` 内で generated Rust の `init()` を expect する
///   logic 検出 (= 0 件、TS-7 で確認済)
///
/// **Pre-Implementation Audit Findings (TS-7、本 PRD doc embed 済)**: codebase + Hono
/// grep で 0 hits = INV-7 reachability prerequisite 満たす。本 stub は Implementation T5
/// で post-T4 state での再確認 (= `pub fn init` 廃止後も新 break が発生していないこと)
/// を fill-in 時に assert。
#[test]
#[ignore = "I-224 INV-7 verification stub: Implementation Stage T5 で fill in \
            (post-T4 state で `init()` call site 0 件 + Hono bench Tier-transition compliance \
            confirm)"]
fn test_invariant_7_pub_fn_init_external_api_audit_post_t4() {
    let _ = transpile;
    unimplemented!(
        "Spec stage stub、Implementation Stage T5 で fill in: \
         post-T4 state (= build_init_fn helper 削除済 + e2e snapshots regenerated) で \
         `grep -rn '\\binit\\s*(' tests/e2e/rust-runner/ src/ tools/` 0 hits を \
         subprocess で assert + Hono bench Tier-transition compliance result classification \
         が `Improvement` or `Preservation` であることを Hono bench output 経由で verify"
    );
}
