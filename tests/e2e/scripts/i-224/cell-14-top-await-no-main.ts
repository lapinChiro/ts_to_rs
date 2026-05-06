// Cell 14: A1 + B0 + C1 — top-Stmt::Expr + no user main + top-level await
// Spec: A1 = top-Stmt::Expr (console.log), B0 = no user main, C1 = top-level await present
// Ideal Rust: synthesize `#[tokio::main] async fn main() { let v = compute().await; println!(...); }`
// Empirical (TS, ESM mode): module-load awaits the Promise then prints
const value = await Promise.resolve(42);
console.log("got", value);
