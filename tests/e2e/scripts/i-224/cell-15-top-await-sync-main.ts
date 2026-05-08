// Cell 15: A1 + B1 + C1 — top-Stmt::Expr + sync user main + top-level await
// Spec: A1 = Stmt::Expr (main() call), B1 = sync user `main` function decl, C1 = top-await
// Ideal Rust: rename user main → __ts_main; synthesize
//   `#[tokio::main] async fn main() { let v = getVal(N).await; __ts_main(); println!(...); }`
//   (= INV-3 (c) Edge sub-case の matrix # 14 instance: sync user main + top-await cohabitation、
//   async fn main 内で sync `__ts_main()` を非 await call で invoke)
// Empirical (TS, ESM mode): hoisted main() runs in source order interleaved with top-await
// Iteration v13 fixture rewrite (2026-05-08): user-defined `getVal` instead of `Promise.resolve`.
async function getVal(n: number): Promise<number> { return n; }
const value = await getVal(10);
function main(): void { console.log("from sync user main"); }
main();
console.log("got", value);
