// I-161 Cell A-2 (F64 narrow): `x &&= y` inside `if (x !== null)` scope.
// Reproduction of I-144 R4 cell that was deferred to this PRD.
// Current emission: `if let Some(x) = x { x = x && 3.0; return x; }` (E0308).
// Ideal: `if let Some(x) = x { if x != 0.0 && !x.is_nan() { x = 3.0; } return x; }`.
// TS runtime: f() → 3; g() → 5.

function f(): number | null {
    let x: number | null = 5;
    if (x !== null) {
        x &&= 3;
        return x;
    }
    return -1;
}

function g(): number | null {
    let x: number | null = null;
    x ??= 10;
    x &&= 5;
    return x;
}

function show(v: number | null): string {
    return v === null ? "null" : `${v}`;
}

function main(): void {
    console.log(show(f()));
    console.log(show(g()));
}
