// I-144 Cell I-025: Option return with multiple exit paths requires per-branch
// implicit `None` insertion. Single-exit (`if ... return x; // fall off`) is
// handled by E5 implicit-None-tail; complex multi-branch case needs E4 match
// exhaustiveness so each non-returning branch emits explicit `None`.
//
// Note: `show` normalises TS runtime `undefined` (implicit `return;` semantics)
// and Rust's `Option<T>::None` to the same string "none" — both represent the
// absence-of-value that fall-off expresses at the signature `number | null`.
// Template literal avoids the `String(v)` callable → synthetic struct issue
// that is orthogonal to I-144 narrowing.
// TS runtime: g(3,4) → 7; g(3,0) → "none"; g(-2,5) → 5; g(-2,0) → "none".

function g(x: number, y: number): number | null {
    if (x > 0) {
        if (y > 0) return x + y;
        // implicit None
    } else {
        if (y > 0) return y;
        // implicit None
    }
    // implicit None
}

function show(v: number | null): string {
    return v == null ? "none" : `${v}`;
}

function main(): void {
    console.log(show(g(3, 4)));
    console.log(show(g(3, 0)));
    console.log(show(g(-2, 5)));
    console.log(show(g(-2, 0)));
}
