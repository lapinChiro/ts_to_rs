// Critical verification: JS null coercion in arithmetic, and cl3b runtime semantic
// null + 1 in JS = 1 (null coerces to 0)
// undefined + 1 in JS = NaN
// We need to know: for I-144 E2 emission on `let r = x + 1` where x was reset to null,
// what value does runtime produce?
console.log(null + 1);           // 1
console.log(undefined + 1);      // NaN
console.log(null + null);        // 0
console.log("null+str=" + (null + "x"));  // nullx (concatenation)

// cl3b re-verify:
function cl3b(): number {
    let x: number | null = 5;
    if (x === null) return -1;
    const reset = () => { x = null; };
    reset();
    // TS types x as `number` here (unsound); runtime x is null
    const r = x + 1;  // JS: null + 1 = 1
    return r;
}
console.log(cl3b());

// What if x is reset to undefined instead?
function cl3c(): number {
    let x: number | undefined = 5;
    if (x === undefined) return -1;
    const reset = () => { x = undefined; };
    reset();
    const r = x + 1;  // JS: undefined + 1 = NaN
    return r;
}
console.log(cl3c());
