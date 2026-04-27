class Counter { _n = 0; get value(): number { return this._n; } set value(v: number) { this._n = v; } incrInternal(): void { this.value = this.value + 1; } }
const c = new Counter();
c.incrInternal();
console.log(c.value);
