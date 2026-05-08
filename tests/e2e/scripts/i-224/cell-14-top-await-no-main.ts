// Cell 14: A1 + B0 + C1 — top-Stmt::Expr + no user main + top-level await
// Spec: A1 = top-Stmt::Expr (console.log), B0 = no user main, C1 = top-level await present
// Ideal Rust: synthesize `#[tokio::main] async fn main() { let v = getVal(42).await; println!(...); }`
// Empirical (TS, ESM mode): module-load awaits the user-defined async fn then prints.
// Iteration v13 fixture rewrite (2026-05-08): user-defined `getVal` async fn instead of
// `Promise.resolve(N)` (= builtin Promise.resolve runtime integration is a separate
// architectural concern from I-224 fn main mechanism + top-await synthesis dispatch).
async function getVal(n: number): Promise<number> { return n; }
const value = await getVal(42);
console.log("got", value);
