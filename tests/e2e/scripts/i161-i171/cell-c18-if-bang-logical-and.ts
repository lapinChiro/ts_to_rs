// I-171 Layer 2 Cell C-18 (LogicalAnd in if-cond): `if (!(x && y)) { ... }`.
// Ideal: De Morgan `!<x truthy> || !<y truthy>`.

function f(x: number | null, y: string | null): string {
    if (!(x && y)) return "at_least_one_falsy";
    return `ok:${x}:${y}`;
}

function main(): void {
    console.log(f(null, "a")); // at_least_one_falsy
    console.log(f(5, null));   // at_least_one_falsy
    console.log(f(0, "a"));    // at_least_one_falsy
    console.log(f(5, ""));     // at_least_one_falsy
    console.log(f(5, "a"));    // ok:5:a
}
