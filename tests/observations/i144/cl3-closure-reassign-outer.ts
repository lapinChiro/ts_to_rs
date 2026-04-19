// Closure reassigning outer-narrowed variable (I-142 C-2 scenario)
function f(): number {
    let x: number | null = 5;
    if (x === null) return -1;
    // x: number
    const reset = () => { x = null; };
    // calling reset widens x
    reset();
    return x ?? -99;  // x is now null
}
console.log(f());
