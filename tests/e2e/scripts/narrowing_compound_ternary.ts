// E2E: compound ternary narrowing with null checks (I-214)

function compoundNullTernary(x: string | null, y: string | null): string {
    return x !== null && y !== null ? x + " " + y : "fallback";
}

function main(): void {
    console.log(compoundNullTernary("hello", "world"));
    console.log(compoundNullTernary("hello", null));
    console.log(compoundNullTernary(null, "world"));
    console.log(compoundNullTernary(null, null));
}
