// Cell 17: A1 + B3 + C1 — top-Stmt::Expr + non-fn user `main` + top-level await
// Spec: A1 = Stmt::Expr, B3 = `main` is non-function (interface here), C1 = top-await
// Ideal Rust: synthesize `#[tokio::main] async fn main()` (Rust fn namespace) +
//   user interface `main` preserved as Rust type (separate namespace, no collision)
// Empirical (TS, ESM mode): interface erased at runtime; only top-await + console.log execute
// Iteration v13 fixture rewrite (2026-05-08): user-defined `getVal` instead of `Promise.resolve`.
interface main { id: number; }
async function getVal(n: number): Promise<number> { return n; }
const m: main = { id: 7 };
const value = await getVal(30);
console.log("got", m.id, value);
