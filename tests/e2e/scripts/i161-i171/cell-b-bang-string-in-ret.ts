// I-171 Layer 1 Cell B-T3 (String operand): `!x` on String return.
// Current emission: `!x` on String → E0600.
// Ideal: `x.is_empty()`.

function f(x: string): boolean {
    return !x;
}

function main(): void {
    console.log(f(""));    // true
    console.log(f("abc")); // false
}
