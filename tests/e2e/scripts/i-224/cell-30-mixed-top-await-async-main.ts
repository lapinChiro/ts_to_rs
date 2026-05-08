// Cell 30: A6 + B2 + C1 — mixed top-level (Decl::Var lit + Decl::Var side-effect + Stmt::Expr) +
//   async user main + top-level await
// Spec: A6 = mixed top-level items, B2 = async user `main`, C1 = top-await
// Ideal Rust: rename user async main → __ts_main; synthesize
//   `#[tokio::main] async fn main() { let LIT_VAL = 100;  /* moved out as Rust top-level const */
//      let n = compute_sync(); let v = compute_async().await; println!(...); __ts_main().await; }`
//   (lit → top-level const preserved; side-effect init + Stmt::Expr captured into fn main body)
// Empirical (TS, ESM mode): module-load order interleaves all
// Iteration v13 fixture rewrite (2026-05-08): user-defined `getVal` instead of `Promise.resolve`.
const LIT_VAL = 100;  // A2-like (lit init), library mode candidate
function compute(): number { return 42; }
const n = compute();  // A3 side-effect init
async function main(): Promise<void> { console.log("from async main"); }
async function getVal(n: number): Promise<number> { return n; }
const value = await getVal(50);  // A3 with await init (also covers C1)
console.log("got", LIT_VAL, n, value);
await main();
