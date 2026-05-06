// Cell 34: A3 + B1 + C1 — Decl::Var with await init + sync user main + top-level await
// Spec: A3 = Decl::Var with await init, B1 = sync `main`, C1 = top-await
// Ideal Rust: rename user main → __ts_main; synthesize
//   `#[tokio::main] async fn main() { let v = compute().await; __ts_main(); println!(...); }`
//   (sync __ts_main() called non-await from async fn main, INV-3 (c) edge sub-case)
// Empirical (TS, ESM mode): hoisted main(), interleaved with top-await + console.log
const value = await Promise.resolve(11);
function main(): void { console.log("from sync user main"); }
main();
console.log("got", value);
