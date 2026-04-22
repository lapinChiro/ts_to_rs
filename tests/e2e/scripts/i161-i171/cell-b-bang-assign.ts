// I-171 Layer 1 Cell B.1.33 (Assign operand): `!(x = y)`.
// Assignment expression returns the assigned value. Ideal: tmp-bind + falsy on value type.

function f(init: number, rhs: number): [string, number] {
    let x = init;
    const label = !(x = rhs) ? "falsy" : "truthy";
    return [label, x];
}

function main(): void {
    const [l1, x1] = f(0, 0);
    console.log(`${l1}:${x1}`); // falsy:0
    const [l2, x2] = f(0, 5);
    console.log(`${l2}:${x2}`); // truthy:5
}
