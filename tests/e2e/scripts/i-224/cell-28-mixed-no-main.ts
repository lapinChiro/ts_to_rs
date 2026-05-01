// Cell 28: A6 + B0 — mixed top-level (Stmt::Expr + Decl::Var with side-effect init + Decl::Var with Lit init)、no user main
// Ideal: synthesize `fn main() { let n = compute(...); println!(...); ... }` (source order preserve)、Lit init は top-level const として preserve
function compute(): number { return 42; }
const LIT_VAL = 100;  // A2 = Lit init、library mode emit (top-level const)
const n = compute();  // A3 = side-effect init、fn main 内 capture
console.log(LIT_VAL, n);  // A1 = Stmt::Expr、fn main 内 capture
