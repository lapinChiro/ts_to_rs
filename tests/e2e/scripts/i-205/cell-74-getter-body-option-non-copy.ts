class Cache { _v: string | undefined = "hello"; get v(): string | undefined { return this._v; } }
const c = new Cache();
console.log(c.v);
