// Cell 5: A0 + B4 — User defines `__ts_main` (collision with reserved namespace), no top-level execution
// Spec: A0 = declarations only (no Stmt::Expr), B4 = `__ts_main` collision, C0 = no top-await
// Ideal: Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use"
// Empirical (TS): tsx executes hoisted decl only, no call site → stdout=(empty), exit_code=0
function __ts_main(): void { console.log("user defined __ts_main"); }
// (no execution statement: pure library form to honor A0 axis、cell-13 が A1 with collision = call-with-execution covers)
