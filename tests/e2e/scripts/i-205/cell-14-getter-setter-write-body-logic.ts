class Foo { _n = 0; get x(): number { return this._n; } set x(v: number) { this._n = v + 1; } }
const f = new Foo();
f.x = 5;
console.log(f.x);
