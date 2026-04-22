// I-171 regression lock-in: existing T6-3 `!<Ident>` on Option<F64> early-return
// consolidated match must remain green after this PRD.
// TS: f(null) → "none"; f(0) → "none"; f(5) → "ok:6".

function f(x: number | null): string {
    if (!x) return "none";
    return `ok:${x + 1}`;
}

function main(): void {
    console.log(f(null));
    console.log(f(0));
    console.log(f(5));
}
