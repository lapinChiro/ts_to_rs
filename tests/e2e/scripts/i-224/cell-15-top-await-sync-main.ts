// Cell 15: A1 + B1 + C1 — top-Stmt::Expr + sync user main + top-level await
// Spec: A1 = Stmt::Expr (main() call), B1 = sync user `main` function decl, C1 = top-await
// Ideal Rust: rename user main → __ts_main; synthesize
//   `#[tokio::main] async fn main() { let v = compute().await; __ts_main(); println!(...); }`
// Empirical (TS, ESM mode): hoisted main() runs in source order interleaved with top-await
const value = await Promise.resolve(10);
function main(): void { console.log("from sync user main"); }
main();
console.log("got", value);
