// I-144 T6-2 follow-up (I-169) regression lockin: multi-function scope
// isolation (matrix cell #3 / P1).
//
// `f` has a closure that reassigns its local `x`; narrow suppress + coerce
// apply. `g` has the same-named `let x` but NO closure-reassign; narrow
// must fire normally (proper match-shadow on the Option → inner f64).
// Under T6-2 before the I-169 fix, g was silently over-suppressed:
// `x.unwrap_or(0.0) + 1.0` instead of the expected match-shadow form.
//
// TS runtime: f() returns 1 (null + 1 = 1 via JS coerce), g() returns 11
// (no reassign, x stays 10 and narrow is alive).

function f(): number {
    let x: number | null = 5;
    if (x === null) return -1;
    const reset = () => { x = null; };
    reset();
    return x + 1;
}

function g(): number {
    let x: number | null = 10;
    if (x === null) return -2;
    return x + 1;
}

function main(): void {
    console.log(f());
    console.log(g());
}
