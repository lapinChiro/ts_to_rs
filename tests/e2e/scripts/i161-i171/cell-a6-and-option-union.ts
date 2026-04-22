// I-161 Cell A-6 (Option<synthetic union>): `x &&= y` on `number | string | null`.
// Inside `if (x !== null)`, narrow to `number | string`. RHS string → narrow adjusts.
// Ideal: per-variant truthy match + assign inner.

function f(init: number | string | null): number | string | null {
    let x: number | string | null = init;
    if (x !== null) {
        x &&= "hello";
    }
    return x;
}

function show(v: number | string | null): string {
    if (v === null) return "null";
    return typeof v === "string" ? `"${v}"` : `${v}`;
}

function main(): void {
    console.log(show(f(5)));      // "hello" (5 truthy, assign "hello")
    console.log(show(f("init"))); // "hello"
    console.log(show(f(null)));   // null (skipped by narrow)
    console.log(show(f(0)));      // 0 (falsy, no assign)
    console.log(show(f("")));     // "" (empty string falsy, no assign)
}
