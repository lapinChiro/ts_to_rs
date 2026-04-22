// I-171 Layer 1 Cell B.1.30 (Cond ternary operand): `!(c ? a : b)`.
// Result type is union of branches (here number/number). Ideal: tmp-bind + falsy.

function f(c: boolean, a: number, b: number): string {
    if (!(c ? a : b)) return "falsy";
    return "truthy";
}

function main(): void {
    console.log(f(true, 0, 5));   // falsy (picked a=0)
    console.log(f(false, 5, 0));  // falsy (picked b=0)
    console.log(f(true, 5, 0));   // truthy
    console.log(f(false, 0, 5));  // truthy
}
