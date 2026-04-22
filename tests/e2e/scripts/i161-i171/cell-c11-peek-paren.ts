// I-171 Layer 2 Cell C-11 (Paren peek-through): `!(x)` in if-cond.
// Ideal: unwrap Paren and recurse (same as !x).

function f(x: number | null): string {
    if (!(x)) return "none";
    return `ok:${x + 1}`;
}

function main(): void {
    console.log(f(null)); // none
    console.log(f(0));    // none
    console.log(f(5));    // ok:6
}
