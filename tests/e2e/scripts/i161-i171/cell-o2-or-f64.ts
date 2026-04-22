// I-161 Cell O-2 (F64 `||=`): `x ||= y` on number.
// Ideal: `if x == 0.0 || x.is_nan() { x = y; }`.

function f(init: number): number {
    let x: number = init;
    x ||= 99;
    return x;
}

function main(): void {
    console.log(f(5));      // 5
    console.log(f(0));      // 99
    console.log(f(NaN));    // 99
    console.log(f(-1));     // -1
}
