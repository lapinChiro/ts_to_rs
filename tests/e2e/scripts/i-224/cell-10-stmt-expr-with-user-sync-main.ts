// Cell 10: A1 + B1 — top-level Stmt::Expr + user sync main
// Ideal: rename user main to `__ts_main` + synthesize `fn main()` with top-level stmts in source order + substitute main() call
// Current: `fn main()` (user) + `pub fn init()` (top-level、never called) — silent semantic change L1
// TS execution order (ECMAScript spec): top-level statements run in source order; `function main` is hoisted but `main();` call site preserves source order
// Expected stdout: "top-level\nfrom main\n"
function main(): void { console.log("from main"); }
console.log("top-level");
main();
