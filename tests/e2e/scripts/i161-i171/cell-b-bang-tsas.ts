// I-171 Layer 1 Cell B.1.17 (TsAs operand): `!(x as any)`.
// TS: `as any` is type-only, runtime is inner value's truthy.
// Current emission: `!(serde_json::from_x)` — invalid.
// Ideal: peek-through to inner expression + falsy predicate on inner's raw type.

function f(x: number | null): string {
    if (!(x as any)) return "falsy";
    return "truthy";
}

function main(): void {
    console.log(f(null)); // falsy
    console.log(f(0));    // falsy
    console.log(f(5));    // truthy
}
