// Cell 75: A6 + B2 + C0 — mixed top-level + async user main + no top-level await
// Spec: A6 = mixed, B2 = async `main`, C0 = no top-await (Trigger 1 only via FnAsync)
// Ideal Rust: top-level `const LIT_VAL = 100;` + rename user async main → __ts_main + synthesize
//   `#[tokio::main] async fn main() { let n = compute(); println!(...); __ts_main().await; }`
//   (synthesis adds `__ts_main().await` regardless of whether source has explicit await)
// Empirical (TS, ESM mode): module-load + fire-and-forget main() call (no `await` at top-level)
const LIT_VAL = 100;
function compute(): number { return 42; }
const n = compute();
async function main(): Promise<void> { console.log("from async main"); }
console.log("got", LIT_VAL, n);
// fire-and-forget sync call (no `await` keyword) → C0 partition; Promise dropped
// async user main body still executes (synchronous prefix), preserving INV-1 source order
main();
