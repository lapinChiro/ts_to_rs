// Cell 1: A0 + B0 + C0 — empty / library only (declarations only、no top-level execution)
// Ideal: no fn main、declarations only emit (regression lock-in)
export function add(a: number, b: number): number { return a + b; }
