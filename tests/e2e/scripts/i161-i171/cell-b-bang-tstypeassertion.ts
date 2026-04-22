// I-171 Layer 1 Cell B.1.37g (TsTypeAssertion operand, legacy <T>e syntax): peek-through.
// `<number>x` is type-only; runtime = inner value's truthy.
// Ideal: peek-through, apply falsy predicate on inner's effective type.

function f(x: number | null): string {
    // Note: `<T>e` legacy TsTypeAssertion (disallowed in .tsx but valid in .ts)
    if (!(<number | null>x)) return "falsy";
    return "truthy";
}

function main(): void {
    console.log(f(null)); // falsy
    console.log(f(0));    // falsy
    console.log(f(5));    // truthy
}
