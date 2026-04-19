// I-144 Cell I-024: truthy narrow complex case — union `string | number | null`
// undergoes truthy complement (`!x`) + typeof split + post-narrow compound
// arithmetic on the `number` branch. CFG analyzer must track:
//   1. `!x` early-return narrows x from `string | number | null` to
//      `string | number` (non-empty / non-zero) afterwards.
//   2. `typeof x === "string"` narrows to `string`; `else` branch is `number`.
//   3. `x += 1` on the number branch must not reset narrow (R2a preserves).
// TS runtime: f("hi") → "s:hi"; f(3) → "n:4"; f(0) → "none"; f("") → "none";
// f(null) → "none".

function f(x: string | number | null): string {
    if (!x) return "none";
    if (typeof x === "string") {
        return "s:" + x;
    }
    x += 1;
    return "n:" + x;
}

function main(): void {
    console.log(f("hi"));
    console.log(f(3));
    console.log(f(0));
    console.log(f(""));
    console.log(f(null));
}
