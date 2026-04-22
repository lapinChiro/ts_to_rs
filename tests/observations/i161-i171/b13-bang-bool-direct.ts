// B13: `!x` where x: boolean — baseline (already correct currently).
function f(x: boolean): boolean { return !x; }
console.log(f(true));   // false
console.log(f(false));  // true
