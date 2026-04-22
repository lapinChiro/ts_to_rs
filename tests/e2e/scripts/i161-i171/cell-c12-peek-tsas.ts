// I-171 Layer 2 Cell C-12 (TsAs peek-through): `!(x as T)` in if-cond.
// Ideal: peek-through TsAs to inner, use inner's effective type for predicate.

function f(x: number | null): string {
    if (!(x as number | null)) return "none";
    return `ok:${(x as number) + 1}`;
}

function main(): void {
    console.log(f(null)); // none
    console.log(f(0));    // none
    console.log(f(5));    // ok:6
}
