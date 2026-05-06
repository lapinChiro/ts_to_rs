// Cell 18: A1 + B4 + C1 — top-Stmt::Expr + `__ts_main` collision + top-level await
// Spec: A1 = Stmt::Expr (call to user-defined __ts_main), B4 = collision, C1 = top-await
// Ideal Rust: Tier 2 honest error reclassify "`__ts_main` is reserved for transpiler-internal use;
//   user must rename to avoid collision" (same wording as cells 5/13)
// Empirical (TS, ESM mode): user `function __ts_main` is a valid identifier; tsx executes
//   await + user call in source order
function __ts_main(): void { console.log("user collision __ts_main"); }
const value = await Promise.resolve(40);
__ts_main();
console.log("got", value);
