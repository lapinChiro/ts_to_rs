// Cell 32: A3 + B0 + C1 — Decl::Var with await init only + no user main + top-level await
// Spec: A3 = Decl::Var with side-effect/non-const init (await init partition), B0 = no user main, C1 = top-await
// Ideal Rust: synthesize `#[tokio::main] async fn main() { let v = compute().await; println!(...); }`
// Empirical (TS, ESM mode): module-load awaits the Promise then prints
const value = await Promise.resolve(99);
console.log("got", value);
