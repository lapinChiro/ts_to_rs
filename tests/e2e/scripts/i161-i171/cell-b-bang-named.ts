// I-171 Layer 1 Cell B-T8 (Named operand): `!x` on non-null interface.
// Ideal: const-fold to `false` (Named instance always truthy).

interface P { a: number }

function f(x: P): boolean {
    return !x;
}

function main(): void {
    console.log(f({ a: 1 })); // false
    console.log(f({ a: 0 })); // false (object ref always truthy)
}
