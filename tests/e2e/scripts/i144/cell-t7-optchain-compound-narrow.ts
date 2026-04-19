// I-144 Cell T7: OptChain `x?.v !== undefined` narrows `x` itself to non-null.
// Ideal emission: the then-branch binds x as `{ v: f64 }` (Some arm), and
// `x.v * 2` (no `?` needed) compiles.
// Current emission: narrow event for compound `x?.v !== undefined` is not
// generated → `x.v * 2` either fails to compile (Option) or forces `.unwrap()`.
// TS runtime: g({v:10}) → 20; g(null) → -1; g({v:0}) → -1 (0 is falsy for
// !== undefined? No — 0 !== undefined is true, so narrows. g({v:0}) → 0).

function g(x: { v: number } | null): number {
    if (x?.v !== undefined) {
        return x.v * 2;
    }
    return -1;
}

function main(): void {
    console.log(g({ v: 10 }));
    console.log(g(null));
    console.log(g({ v: 0 }));
}
