class Calc { add(a: number, b: number): number { return a + b; } }
const c = new Calc();
const fn = c.add;
console.log(typeof fn);
