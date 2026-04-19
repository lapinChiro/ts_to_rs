// Closure × Loop: closure created inside loop captures loop-var + outer narrow
function f(): number[] {
    let x: number | null = 5;
    const fns: (() => number)[] = [];
    for (let i = 0; i < 3; i++) {
        if (x !== null) {
            fns.push(() => x ?? -1);  // use ?? to avoid narrow-loss error
        }
    }
    return fns.map(fn => fn());
}
console.log(f().join(","));
