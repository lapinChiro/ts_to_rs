class Foo { _n: number = 42; get n(): number { return this._n; } }
const f = new Foo();
console.log(f.n);
