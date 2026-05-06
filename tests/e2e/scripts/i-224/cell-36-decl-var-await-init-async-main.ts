// Cell 36: A3 + B2 + C1 — Decl::Var with await init + async user main + top-level await
// Spec: A3 = Decl::Var with await init, B2 = async `main`, C1 = top-await
// Ideal Rust: rename user async main → __ts_main; synthesize
//   `#[tokio::main] async fn main() { let v = compute().await; __ts_main().await; println!(...); }`
//   (Trigger 1 + Trigger 2 combined)
// Empirical (TS, ESM mode): top-await + async user main both await
const value = await Promise.resolve(22);
async function main(): Promise<void> { console.log("from async user main"); }
await main();
console.log("got", value);
