// I-171 Layer 2 Cell C-23 (LogicalOr in if-cond): `if (!(x || y)) { ... }`.
// Ideal: De Morgan `<x falsy> && <y falsy>`.

function f(x: number | null, y: string | null): string {
    if (!(x || y)) return "both_falsy";
    return `ok:${x ?? "null"}:${y ?? "null"}`;
}

function main(): void {
    console.log(f(null, null));  // both_falsy
    console.log(f(0, ""));       // both_falsy
    console.log(f(5, null));     // ok:5:null
    console.log(f(null, "a"));   // ok:null:a
    console.log(f(5, "a"));      // ok:5:a
}
