// Cell 78: A6 + B3 + C1 — mixed top-level + non-fn `main` + top-level await
// Spec: A6 = mixed, B3 = non-fn (interface), C1 = top-await (Trigger 2 only)
// Ideal Rust: top-level `const LIT_VAL = 100;` + interface preserved + synthesize
//   `#[tokio::main] async fn main() { let n = compute(); let v = ....await; println!(...); }`
// Empirical (TS, ESM mode): interface erased, top-await + console.log execute
interface main { id: number; }
const m: main = { id: 78 };
const LIT_VAL = 100;
function compute(): number { return 42; }
const n = compute();
const value = await Promise.resolve(78);
console.log("got", m.id, LIT_VAL, n, value);
