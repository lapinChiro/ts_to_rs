// Cell 19: A2 + B0 — top-level Decl::Var with Lit init only (library mode、no fn main needed)
// Ideal: top-level `const x: f64 = 0.0;` + no fn main (regression lock-in)
export const x = 0;
export const flag = true;
