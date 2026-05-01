// Cell 21: A3 + B0 — top-level Decl::Var with side-effect init only、no user main
// Note: 本 fixture では class instantiation を avoid して I-162 (constructor synthesis) dependency を切り離し、
// pure function call (`makePoint(...)`) で Decl::Var with non-Lit init を test し B2 architectural concern を独立 verify。
// Ideal: synthesize `fn main() { let p = make_point(1.0, 2.0); println!("{} {}", p.x, p.y); }`
// Current: declaration silently dropped (I-016 silent skip + no fn main = E0601)
function makePoint(x: number, y: number): { x: number; y: number } { return { x, y }; }
const p = makePoint(1, 2);
console.log(p.x, p.y);
