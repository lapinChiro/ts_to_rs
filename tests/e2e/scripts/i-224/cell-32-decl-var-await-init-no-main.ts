// Cell 32: A3 + B0 + C1 — Decl::Var with await init only + no user main + top-level await
// Spec: A3 = Decl::Var with side-effect/non-const init (await init partition), B0 = no user main, C1 = top-await
// Ideal Rust: synthesize `#[tokio::main] async fn main() { let v = getVal(N).await; println!(...); }`
// Empirical (TS, ESM mode): module-load awaits the user-defined async fn then prints
// Iteration v13 fixture rewrite (2026-05-08): user-defined `getVal` instead of `Promise.resolve`.
async function getVal(n: number): Promise<number> { return n; }
const value = await getVal(99);
console.log("got", value);
