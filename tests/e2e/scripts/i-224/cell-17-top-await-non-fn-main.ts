// Cell 17: A1 + B3 + C1 — top-Stmt::Expr + non-fn user `main` + top-level await
// Spec: A1 = Stmt::Expr, B3 = `main` is non-function (interface here), C1 = top-await
// Ideal Rust: synthesize `#[tokio::main] async fn main()` (Rust fn namespace) +
//   user interface `main` preserved as Rust type (separate namespace, no collision)
// Empirical (TS, ESM mode): interface erased at runtime; only top-await + console.log execute
interface main { id: number; }
const m: main = { id: 7 };
const value = await Promise.resolve(30);
console.log("got", m.id, value);
