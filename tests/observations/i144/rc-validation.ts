// RC (Read Context) dimension validation
// Verify narrow emission requirements for each RC category

// RC1: Expect-T-value (direct read)
function rc1_return(x: number | null): number {
    if (x === null) return -1;
    return x + 1;  // x read as number (direct T)
}

// RC2: Expect-Option<T> (Option context)
function rc2_nc(x: number | null): number {
    if (x === null) return -1;
    return x ?? 99;  // narrow alive, but ?? expects Option
}

// RC3: Mutation ??=
function rc3_mutation(x: number | null): number {
    if (x === null) return -1;
    // x is narrowed number; ??= is no-op at runtime
    let y: number | null = x;
    y ??= 5;
    return y;
}

// RC4: Boolean truthy
function rc4_truthy(x: number | null): string {
    if (x === null) return "null";
    // Re-check narrowed x in boolean
    if (x) return "truthy";
    return "zero-narrowed";
}

// RC5: Match discriminant (switch)
function rc5_switch(x: string | number | null): string {
    if (x === null) return "null";
    // x: string | number
    switch (typeof x) {
        case "string": return "s:" + x;
        case "number": return "n:" + x;
        default: return "other";
    }
}

// RC6: String template interp
function rc6_template(x: number | null): string {
    if (x === null) return "null";
    return `value: ${x}`;  // narrow x in template
}

// RC7: Callback body capture (narrow visibility)
function rc7_callback(x: number | null): number[] {
    if (x === null) return [];
    // x: number (narrowed); used inside map callback
    return [1, 2, 3].map(i => i + x);
}

// RC8: Passthrough in paren
function rc8_paren(x: number | null): number {
    if (x === null) return -1;
    return (x) + 1;  // paren passthrough
}

console.log(rc1_return(5));
console.log(rc1_return(null));
console.log(rc2_nc(5));
console.log(rc3_mutation(5));
console.log(rc4_truthy(5));
console.log(rc4_truthy(0));
console.log(rc5_switch("a"));
console.log(rc5_switch(3));
console.log(rc6_template(7));
console.log(rc7_callback(10).join(","));
console.log(rc8_paren(5));
