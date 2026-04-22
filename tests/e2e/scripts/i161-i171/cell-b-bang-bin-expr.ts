// I-171 Layer 1 Cell B.1.21 (Bin arith operand): `!(x + 1)` on F64.
// Current emission: `!(x + 1.0)` (invalid `!f64`).
// Ideal: `{ let _tmp = x + 1.0; _tmp == 0.0 || _tmp.is_nan() }` or structured equivalent.

function f(x: number): string {
    if (!(x + 1)) return "falsy";
    return "truthy";
}

function main(): void {
    console.log(f(-1)); // falsy (x+1 = 0)
    console.log(f(0));  // truthy
    console.log(f(5));  // truthy
}
