// cl3b: Confirm TS narrow behavior after closure reassign
// The question: does TS treat x as number (unsound) or number|null (sound)?
function f(): string {
    let x: number | null = 5;
    if (x === null) return "null";
    // x: number
    const reset = () => { x = null; };
    reset();
    // Is x: number (TS unsound) or number | null (TS sound)?
    // Try using x without `??`:
    const r = x + 1;  // would fail if TS widened
    return String(r);
}
console.log(f());
