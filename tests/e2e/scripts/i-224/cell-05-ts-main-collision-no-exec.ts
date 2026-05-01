// Cell 5: A0 + B4 — User defines `__ts_main` (collision with reserved namespace)、no top-level execution
// Ideal: Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use"
// Current: `fn __ts_main()` directly emit (no reservation detection、本 PRD で reserved 化検出未実装)
function __ts_main(): void { console.log("user defined __ts_main"); }
__ts_main();
