class OptCache { _v: string | undefined = "hello"; get v(): string | undefined { return this._v; } }
const c = new OptCache();
console.log(c.v);
