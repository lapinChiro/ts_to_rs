class Calc { add(a: number, b: number): number { return a + b; } }
const c = new Calc();
(c as any).add += 1;
console.log(typeof c.add);
