class Cnt { static _v = 10; static get x(): number { return Cnt._v; } static set x(v: number) { Cnt._v = v; } }
Cnt.x += 5;
console.log(Cnt.x);
