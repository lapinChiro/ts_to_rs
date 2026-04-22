// A1: &&= on Option<number> with NO narrow alive.
// Question: does TS evaluate `x &&= y` via JS truthy on Option (Some/None)?
// JS semantics: `x &&= y` ≡ `if (x) x = y`.
//   - x=null → null is falsy → no assign → x stays null
//   - x=5    → 5 is truthy → x = 3
//   - x=0    → 0 is falsy → no assign → x stays 0
//   - x=null (undefined route): null is falsy → no assign
let a: number | null = 5;
a &&= 3;
console.log(a === null ? "null" : a);

let b: number | null = null;
b &&= 3;
console.log(b === null ? "null" : b);

let c: number | null = 0;
c &&= 3;
console.log(c === null ? "null" : c);
