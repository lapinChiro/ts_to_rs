// A2: ||= on Option<number> with NO narrow alive.
// JS semantics: `x ||= y` ≡ `if (!x) x = y`.
//   - x=null → falsy → x = 3
//   - x=5    → truthy → no assign
//   - x=0    → falsy → x = 3
let a: number | null = null;
a ||= 3;
console.log(a);

let b: number | null = 5;
b ||= 3;
console.log(b);

let c: number | null = 0;
c ||= 3;
console.log(c);
