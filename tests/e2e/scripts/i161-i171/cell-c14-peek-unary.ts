// I-171 Layer 2 Cell C-14 (Unary(!) recursion): `!!x` in if-cond.
// Ideal: fold to truthy predicate directly.

function f(x: number | null): string {
    if (!!x) return "truthy";
    return "falsy";
}

function main(): void {
    console.log(f(null)); // falsy
    console.log(f(0));    // falsy
    console.log(f(5));    // truthy
}
