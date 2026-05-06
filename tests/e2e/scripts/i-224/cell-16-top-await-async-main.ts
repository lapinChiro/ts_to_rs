// Cell 16: A1 + B2 + C1 — top-Stmt::Expr + async user main + top-level await
// Spec: A1 = Stmt::Expr (await main() call), B2 = async user `main`, C1 = top-await
// Ideal Rust: rename user async main → __ts_main; synthesize
//   `#[tokio::main] async fn main() { let v = compute().await; __ts_main().await; println!(...); }`
// Empirical (TS, ESM mode): top-await + async user main both await module-load order
const value = await Promise.resolve(20);
async function main(): Promise<void> { console.log("from async user main"); }
await main();
console.log("got", value);
