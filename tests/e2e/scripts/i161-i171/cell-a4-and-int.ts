// I-161 Cell A-4 (Primitive int — bigint/i128): `x &&= y` on bigint LHS.
// TS `bigint` → Rust IR `Primitive(I128)`. Only user-facing path that reliably
// produces a Primitive(int) LHS is bigint syntax; arr.length gives f64 LHS
// because `let len: number = arr.length` binds len as number (F64).
// JS: 0n &&= 99n → 0n (0n falsy); 5n &&= 99n → 99n.
// Ideal: `if x != 0 { x = 99; }`.

function f(init: bigint): bigint {
    let x: bigint = init;
    x &&= 99n;
    return x;
}

function main(): void {
    console.log(f(5n));  // 99
    console.log(f(0n));  // 0
}
