// I-171 Layer 1 Cell B-T8 (Vec operand): `!arr` on Vec<T>.
// JS: empty/non-empty array is always truthy (object reference).
// Ideal: const-fold to `false`.

function f(arr: number[]): boolean {
    return !arr;
}

function main(): void {
    console.log(f([]));        // false (empty array is truthy object ref)
    console.log(f([1, 2, 3])); // false
}
