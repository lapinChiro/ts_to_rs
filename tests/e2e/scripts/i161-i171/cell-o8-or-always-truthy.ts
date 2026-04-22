// I-161 Cell O-8 (always-truthy `||=`): LHS always truthy → no-op.
// Ideal: const-fold to empty stmt (no assign).

interface P { a: number }

function f(): number {
    let p: P = { a: 1 };
    p ||= { a: 99 };  // no-op (p always truthy)
    return p.a;
}

function main(): void {
    console.log(f()); // 1
}
