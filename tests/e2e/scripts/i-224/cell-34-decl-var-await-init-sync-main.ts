// Cell 34: A3 + B1 + C1 — Decl::Var with await init + sync user main + top-level await
// Spec: A3 = Decl::Var with await init, B1 = sync `main`, C1 = top-await
// Ideal Rust: rename user main → __ts_main; synthesize
//   `#[tokio::main] async fn main() { let v = getVal(N).await; __ts_main(); println!(...); }`
//   (sync __ts_main() called non-await from async fn main, INV-3 (c) edge sub-case)
// Empirical (TS, ESM mode): hoisted main(), interleaved with top-await + console.log
// Iteration v13 fixture rewrite (2026-05-08): user-defined `getVal` instead of `Promise.resolve`.
async function getVal(n: number): Promise<number> { return n; }
const value = await getVal(11);
function main(): void { console.log("from sync user main"); }
main();
console.log("got", value);
