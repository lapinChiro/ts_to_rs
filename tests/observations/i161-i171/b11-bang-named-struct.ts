// B11: `!x` where x is Named (always truthy in JS, any object reference is truthy).
interface P { a: number }
function f(x: P): boolean { return !x; }
console.log(f({ a: 1 }));  // false (x is an object, truthy)
