// A3: &&= on Option<string> (non-narrowed).
// "" is falsy in JS.
let a: string | null = "hello";
a &&= "world";
console.log(a);

let b: string | null = null;
b &&= "world";
console.log(b);

let c: string | null = "";
c &&= "world";
console.log(c);
