// I-161 Cell A-8 (always-truthy LHS): `x &&= y` on Named struct.
// Named structure instance is always truthy in JS → const-fold `x = y;`.

interface P { a: number }

function f(): number {
    let p: P = { a: 1 };
    p &&= { a: 99 };
    return p.a;
}

function main(): void {
    console.log(f()); // 99
}
