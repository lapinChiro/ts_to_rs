// F4: Loop body — narrow per iteration
// Initial: x: number | null = 5
// After narrow in block: loop body re-reads x; does TS reset narrow per iteration?
function f(): number {
    let x: number | null = 5;
    let sum = 0;
    for (let i = 0; i < 3; i++) {
        if (x !== null) {
            sum += x;
            // Reassign to null at last iter
            if (i === 1) x = null;
        }
    }
    return sum;
}
// Loop where narrow is established before body
function g(): number {
    let x: number | null = 5;
    if (x !== null) {
        let out = 0;
        for (let i = 0; i < 3; i++) {
            // TS: does x remain narrowed inside loop?
            out += x;
        }
        return out;
    }
    return -1;
}
console.log(f());
console.log(g());
