// I-171 Layer 1 Cell B-T6 (Option<synthetic union> operand): `!x` on `number | string | null`.
// Ideal: per-variant match with truthy guards inverted.

function f(x: number | string | null): boolean {
    return !x;
}

function main(): void {
    console.log(f(null));  // true
    console.log(f(0));     // true (0 falsy)
    console.log(f(""));    // true (empty string falsy)
    console.log(f(NaN));   // true
    console.log(f(5));     // false
    console.log(f("hi")); // false
}
