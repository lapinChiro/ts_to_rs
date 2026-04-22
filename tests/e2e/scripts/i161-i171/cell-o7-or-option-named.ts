// I-161 Cell O-7 (Option<Named other> `||=`): nullable interface.
// Ideal: `if x.is_none() { x = Some(y); }`.

interface P { a: number }

function f(init: P | null): P {
    let x: P | null = init;
    x ||= { a: 99 };
    return x;
}

function main(): void {
    console.log(f({ a: 1 }).a); // 1
    console.log(f(null).a);     // 99
}
