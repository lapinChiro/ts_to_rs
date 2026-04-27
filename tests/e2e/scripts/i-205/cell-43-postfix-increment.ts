class Counter { _n = 5; get value(): number { return this._n; } set value(v: number) { this._n = v; } }
const c = new Counter();
c.value++;
console.log(c.value);
