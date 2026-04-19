// F6: Try body — narrow inside try, what about catch?
function f(): number {
    let x: number | null = 5;
    try {
        if (x !== null) {
            // narrow in try
            if (Math.random() < 0) throw new Error("unreachable");
            return x + 1;
        }
        return -1;
    } catch (e) {
        // TS: is x narrowed here?
        // x: number | null (narrow invalidated by throw-possibly-before-narrow)
        return x ?? -99;
    }
}
// Try with assign inside
function g(): number {
    let x: number | null = 5;
    try {
        if (x === null) throw new Error("null");
        // x: number
        x = 10;
        // x: number (narrow preserved via assign)
        return x;
    } catch (e) {
        // x: number | null (assignment during try not observable by catch)
        return x ?? -99;
    }
}
console.log(f());
console.log(g());
