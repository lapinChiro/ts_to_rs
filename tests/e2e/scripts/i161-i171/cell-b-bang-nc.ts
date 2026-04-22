// I-171 Layer 1 Cell B.1.28 (Bin(NullishCoalescing) operand): `!(x ?? d)`.
// Result type of `x ?? d` depends; here both are number so result is number.
// Ideal: tmp-bind + falsy predicate on result type (F64).

function f(x: number | null): string {
    if (!(x ?? 0)) return "falsy";
    return "truthy";
}

function main(): void {
    console.log(f(null));  // falsy (null ?? 0 = 0, !0 = true)
    console.log(f(0));     // falsy (0 ?? 0 = 0)
    console.log(f(5));     // truthy
}
