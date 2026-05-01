// Cell 13: A1 + B4 — top-level Stmt::Expr + user `__ts_main` collision
// Ideal: Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use"
function __ts_main(): void { console.log("user __ts_main"); }
console.log("top-level");
__ts_main();
