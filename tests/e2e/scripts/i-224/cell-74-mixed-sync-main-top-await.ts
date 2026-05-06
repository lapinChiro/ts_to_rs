// Cell 74: A6 + B1 + C1 — mixed top-level + sync user main + top-level await
// Spec: A6 = mixed, B1 = sync `main`, C1 = top-await (INV-3 (c) edge sub-case = sync user main + top-await)
// Ideal Rust: top-level `const LIT_VAL = 100;` + rename user main → __ts_main + synthesize
//   `#[tokio::main] async fn main() { let n = compute(); let v = ....await; __ts_main(); println!(...); }`
//   (sync __ts_main() called non-await from async fn main)
// Empirical (TS, ESM mode): hoisted main() interleaves with top-await + Stmt::Expr in source order
const LIT_VAL = 100;
function compute(): number { return 42; }
const n = compute();
function main(): void { console.log("from sync user main"); }
const value = await Promise.resolve(74);
main();
console.log("got", LIT_VAL, n, value);
