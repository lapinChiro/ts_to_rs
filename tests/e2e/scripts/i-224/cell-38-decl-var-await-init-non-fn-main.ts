// Cell 38: A3 + B3 + C1 — Decl::Var with await init + non-fn user `main` + top-level await
// Spec: A3 = Decl::Var with await init, B3 = `main` is non-function (interface), C1 = top-await
// Ideal Rust: synthesize `#[tokio::main] async fn main()` (Rust fn namespace) +
//   user interface `main` preserved as Rust type (separate namespace)
// Empirical (TS, ESM mode): interface erased at runtime; only top-await + console.log execute
interface main { id: number; }
const m: main = { id: 33 };
const value = await Promise.resolve(33);
console.log("got", m.id, value);
