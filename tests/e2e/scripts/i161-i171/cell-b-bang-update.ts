// I-171 Layer 1 Cell B.1.36 (Update operand): `!(i++)` — side-effect heavy.
// JS: `i++` returns old value (postfix). `!(i++)` = falsy on old i.
// Ideal: tmp-bind on Update result type (F64) + falsy.

function f(init: number): [string, number] {
    let i = init;
    const label = !(i++) ? "old_falsy" : "old_truthy";
    return [label, i];
}

function main(): void {
    const [l1, i1] = f(0);
    console.log(`${l1}:${i1}`); // old_falsy:1
    const [l2, i2] = f(5);
    console.log(`${l2}:${i2}`); // old_truthy:6
}
