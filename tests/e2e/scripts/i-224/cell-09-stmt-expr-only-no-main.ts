// Cell 9: A1 + B0 — top-level Stmt::Expr only、no user main
// Ideal: synthesize `fn main() { println!("hello world"); }`
// Current: `pub fn init() { println!("hello world"); }` only、no `fn main` (E0601 compile fail)
console.log("hello world");
