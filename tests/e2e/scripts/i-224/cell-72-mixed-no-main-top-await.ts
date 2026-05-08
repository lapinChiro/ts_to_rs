// Cell 72: A6 + B0 + C1 — mixed top-level (Lit init + side-effect init + Stmt::Expr + await stmt) +
//   no user main + top-level await
// Spec: A6 = mixed, B0 = no user main, C1 = top-await
// Ideal Rust: top-level `const LIT_VAL = 100;` (per-item runtime, Lit init partition) +
//   `#[tokio::main] async fn main() { let n = compute(); let v = getVal(N).await; println!(...); }`
// Empirical (TS, ESM mode): module-load order
// Iteration v13 fixture rewrite (2026-05-08): user-defined `getVal` instead of `Promise.resolve`.
const LIT_VAL = 100;
function compute(): number { return 42; }
async function getVal(n: number): Promise<number> { return n; }
const n = compute();
const value = await getVal(72);
console.log("got", LIT_VAL, n, value);
