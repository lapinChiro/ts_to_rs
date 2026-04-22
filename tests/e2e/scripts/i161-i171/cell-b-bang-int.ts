// I-171 Layer 1 Cell B-T4 (Primitive int operand): `!x` on usize-ish number.
// ts_to_rs emits `usize` for array index contexts. `!<usize>` → `x == 0`.

function f(arr: number[]): boolean {
    return !arr.length;
}

function main(): void {
    console.log(f([]));       // true (length=0 falsy)
    console.log(f([1]));      // false
    console.log(f([1, 2, 3])); // false
}
