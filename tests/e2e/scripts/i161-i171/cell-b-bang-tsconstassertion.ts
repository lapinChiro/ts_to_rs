// I-171 Layer 1 Cell B.1.37i (TsConstAssertion operand, `e as const`): peek-through.
// `as const` is type-only; runtime = inner value's truthy.
// Ideal: peek-through, apply falsy predicate on inner's effective type.

function f(x: number): string {
    if (!(x as const)) return "falsy";
    return "truthy";
}

function main(): void {
    console.log(f(0)); // falsy
    console.log(f(5)); // truthy
}
