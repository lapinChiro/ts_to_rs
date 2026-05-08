// Cell 78: A6 + B3 + C1 — mixed top-level + non-fn `main` + top-level await
// Spec: A6 = mixed, B3 = non-fn (interface), C1 = top-await (Trigger 2 only)
// Ideal Rust: top-level `const LIT_VAL = 100;` + interface preserved + synthesize
//   `#[tokio::main] async fn main() { let n = compute(); let v = getVal(N).await; println!(...); }`
// Empirical (TS, ESM mode): interface erased, top-await + console.log execute
// Iteration v13 fixture rewrite (2026-05-08): user-defined `getVal` instead of `Promise.resolve`.
interface main { id: number; }
async function getVal(n: number): Promise<number> { return n; }
const m: main = { id: 78 };
const LIT_VAL = 100;
function compute(): number { return 42; }
const n = compute();
const value = await getVal(78);
console.log("got", m.id, LIT_VAL, n, value);
