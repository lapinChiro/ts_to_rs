class Counter { _n = 5; get value(): number { return this._n; } set value(v: number) { this._n = v; } incrInternalIncr(): void { this.value++; } }
const c = new Counter();
c.incrInternalIncr();
console.log(c.value);
