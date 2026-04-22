// I-161 Cell A-{expr context}: `const z = (x &&= y);` — result is final x.
// JS: `x &&= y` evaluates to new x (y if truthy, old x if falsy).

function f(init: number): [number, number] {
    let a = init;
    const r = (a &&= 3);
    return [r, a];
}

function main(): void {
    const [r1, a1] = f(5);
    console.log(`${r1},${a1}`); // "3,3"
    const [r2, a2] = f(0);
    console.log(`${r2},${a2}`); // "0,0" (no assign, result is falsy)
}
