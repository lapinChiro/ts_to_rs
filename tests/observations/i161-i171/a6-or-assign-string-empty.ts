// A6: `x ||= y` on String where "" is falsy.
let a: string = "";
a ||= "default";
console.log(a);  // "default"

let b: string = "value";
b ||= "default";
console.log(b);  // "value"
