// A4: `&&=` / `||=` in expression-context (result is a value).
// JS: `x &&= y` evaluates to the final value of x.
let a: number = 5;
const r1 = (a &&= 3);
console.log(r1, a);  // 3 3

let b: number = 0;
const r2 = (b &&= 3);
console.log(r2, b);  // 0 0 (no assign, result is falsy)

let c: number = 0;
const r3 = (c ||= 3);
console.log(r3, c);  // 3 3 (assign happened)
