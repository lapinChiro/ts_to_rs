// Cell 77: A6 + B3 + C0 — mixed top-level + non-fn user `main` + no top-level await
// Spec: A6 = mixed, B3 = `main` is non-function (interface), C0 = no top-await (Sync, no trigger)
// Ideal Rust: top-level `const LIT_VAL = 100;` + interface `main` preserved as Rust type +
//   synthesize plain `fn main() { let n = compute(); println!(...); }`
// Empirical (TS, ESM mode): interface erased, only stmts execute
interface main { id: number; }
const m: main = { id: 77 };
const LIT_VAL = 100;
function compute(): number { return 42; }
const n = compute();
console.log("got", m.id, LIT_VAL, n);
