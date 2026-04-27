class Base { _n = 10; get x(): number { return this._n; } set x(v: number) { this._n = v; } }
class Sub extends Base {}
const s = new Sub();
s.x += 5;
console.log(s.x);
