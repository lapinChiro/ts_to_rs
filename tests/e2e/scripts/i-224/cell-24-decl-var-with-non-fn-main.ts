// Cell 24: A3 + B3 — top-level Decl::Var with side-effect init + user `main` non-fn symbol (interface)
// Ideal: synthesize `fn main()` + interface main preserved as Rust type
interface main { kind: string; }
function makePoint(x: number, y: number): { x: number; y: number } { return { x, y }; }
const meta: main = { kind: "point" };
const p = makePoint(1, 2);
console.log(meta.kind, p.x, p.y);
