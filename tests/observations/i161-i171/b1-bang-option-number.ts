// B1: `!x` where x: Option<number> in expression context.
// JS: `!x` - null falsy, 0 falsy, NaN falsy, other truthy.
function check(x: number | null): boolean { return !x; }
console.log(check(null));  // true
console.log(check(0));     // true
console.log(check(NaN));   // true
console.log(check(5));     // false
