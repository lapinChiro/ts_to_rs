// Cell 11: A1 + B2 — top-level Stmt::Expr + user async main
// Ideal: rename user async main to `__ts_main` + `#[tokio::main] async fn main() { <top-level stmts>; <substituted main() call>; }`
// Current: `#[tokio::main] async fn main()` (user) + `pub fn init()` (never called) — silent semantic change L1
async function main(): Promise<void> { console.log("from async main"); }
console.log("top-level");
main();
