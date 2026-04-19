// I-144 Cell T4d: `if (x)` on `number` — TS truthy is "non-zero AND non-NaN".
// Current Rust emission approximates with `if x != 0.0` which misclassifies NaN
// as truthy. Ideal predicate (E10): `x != 0.0 && !x.is_nan()`.
// TS runtime: report("nonzero", 5), report("zero-or-nan", 0),
//             report("nonzero", -1), report("zero-or-nan", NaN).

function report(x: number): string {
    if (x) {
        return "nonzero:" + x;
    }
    return "zero-or-nan";
}

function main(): void {
    console.log(report(5));
    console.log(report(0));
    console.log(report(-1));
    console.log(report(NaN));
}
