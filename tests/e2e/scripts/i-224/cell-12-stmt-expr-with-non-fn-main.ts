// Cell 12: A1 + B3 — top-level Stmt::Expr + user `main` non-fn symbol (interface)
// Rust namespace 別 (interface = type position、fn main = value position) のため衝突なし
// Ideal: synthesize `fn main() { ... }` + interface `main` preserved as Rust type
interface main { value: number; }
const v: main = { value: 42 };
console.log(v.value);
