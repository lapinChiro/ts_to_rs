// I-171 Layer 1 Cell B-T7 (Option<Named other> operand): `!x` on nullable interface.
// Ideal: `x.is_none()` (Named always truthy when Some).

interface P { a: number }

function f(x: P | null): boolean {
    return !x;
}

function main(): void {
    console.log(f(null));      // true
    console.log(f({ a: 1 })); // false
    console.log(f({ a: 0 })); // false (object ref always truthy, regardless of a)
}
