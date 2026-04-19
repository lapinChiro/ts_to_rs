// I-144 regression lock-in: RC1-RC8 read-context survey under narrow alive.
// Locks in current ideal-output behaviour for each Read Context so the
// CFG-analyzer migration (T3-T6) does not regress any single cluster.
// Each context is exercised once on an `Option<number>` narrowed to `number`.
// TS runtime: see `expected` prints below.

function rc1_arith(x: number | null): number {
    if (x === null) return -1;
    return x + 2;                          // RC1 expect-T arithmetic
}
function rc2_option_read(x: number | null): string {
    // RC2: expect Option<T> (nullish-coalesce RHS keeps Option context)
    return (x ?? 0).toString();
}
function rc4_bool(x: number | null): boolean {
    if (x === null) return false;
    return x > 0;                          // RC4 boolean read
}
function rc5_match(x: number | null): string {
    if (x === null) return "n";
    switch (x) {                           // RC5 match discriminant
        case 1: return "one";
        case 2: return "two";
        default: return "other";
    }
}
function rc6_concat(x: number | null): string {
    if (x === null) return "";
    return "v=" + x;                       // RC6 string concat
}
function rc7_callback(x: number | null): number {
    if (x === null) return -1;
    const arr = [1, 2, 3];
    return arr.reduce((acc, v) => acc + v + x, 0); // RC7 callback body capture
}
function rc8_paren(x: number | null): number {
    if (x === null) return -1;
    return ((x));                          // RC8 paren passthrough
}

function main(): void {
    console.log(rc1_arith(5));
    console.log(rc2_option_read(null));
    console.log(rc4_bool(3));
    console.log(rc5_match(2));
    console.log(rc6_concat(9));
    console.log(rc7_callback(10));
    console.log(rc8_paren(7));
}
