// Critical verification: What is the ideal emission for "closure reassign invalidates narrow"?
// Source of truth: tsc compiles, tsx runtime emits specific value.
// cl3b runtime: 1 (null + 1 via JS coercion)
// Rust ideal must produce 1 at runtime.

// Also: without arithmetic, just return x — what does TS allow?
function f(): number | null {
    let x: number | null = 5;
    if (x === null) return null;
    // x: number (narrowed)
    const reset = () => { x = null; };
    reset();
    // TS: x narrowed to number (unsound); return type compatible?
    return x;  // returns null at runtime, but TS sees as number
}
const result = f();
console.log(result);  // null
console.log(typeof result);

// Contrast: no closure, direct reassign
function g(): number | null {
    let x: number | null = 5;
    if (x === null) return null;
    // x: number
    x = null;
    // TS widens x to number | null here
    // return x;  // would error if uncommented: returning number | null as number
    return x;  // TS: x is number | null
}
console.log(g());

// Equality check on potentially-reset var
function h(): boolean {
    let x: number | null = 5;
    if (x === null) return false;
    const reset = () => { x = null; };
    reset();
    // Does TS allow `x === 5` here?
    return x === 5;  // x typed as number, but runtime null
}
console.log(h());
