// I-144 Cell #14: `??=` followed by linear `x = null` reset. Current emission
// surfaces `UnsupportedSyntaxError("nullish-assign with narrowing-reset")`.
// Ideal emission post-I-144: E2a `x.get_or_insert_with(|| 0.0);` (Option kept),
// so `x = None` remains valid and the return reflects runtime null.
// TS runtime: narrowingReset(null) → null (x is None after reset);
// narrowingReset(7) → null (x set to 7 via ??= is then overwritten to null).

function narrowingReset(x: number | null): number | null {
    x ??= 0;
    x = null;
    return x;
}

function show(v: number | null): string {
    // Template literal avoids the `String(v)` callable → synthetic-struct
    // issue that is orthogonal to I-144 narrowing (same pattern as
    // cell-i025-option-return-implicit-none-complex).
    return v === null ? "null" : `${v}`;
}

function main(): void {
    console.log(show(narrowingReset(null)));
    console.log(show(narrowingReset(7)));
}
