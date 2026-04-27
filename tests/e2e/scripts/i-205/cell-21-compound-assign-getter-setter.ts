class Counter { _n = 10; get value(): number { return this._n; } set value(v: number) { this._n = v; } }
const c = new Counter();
c.value += 5;
console.log(c.value);
