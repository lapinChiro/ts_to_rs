// I-161 Cell O-6 (Option<synthetic union> `||=`): `x ||= y` on `number | string | null`.
// Inside `if (x !== null)`, narrow to `number | string`. Per-variant falsy dispatch.

function f(init: number | string | null, rhs: string): number | string | null {
    let x: number | string | null = init;
    if (x !== null) {
        x ||= rhs;
    }
    return x;
}

function show(v: number | string | null): string {
    if (v === null) return "null";
    return typeof v === "string" ? `"${v}"` : `${v}`;
}

function main(): void {
    console.log(show(f(5, "def")));    // 5 (truthy)
    console.log(show(f(0, "def")));    // "def" (falsy)
    console.log(show(f("hi", "def")));  // "hi"
    console.log(show(f("", "def")));   // "def"
    console.log(show(f(null, "def")));  // null (skipped by narrow)
}
