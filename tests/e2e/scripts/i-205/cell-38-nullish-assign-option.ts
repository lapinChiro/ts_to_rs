class Cache { _v: number | undefined = undefined; get value(): number | undefined { return this._v; } set value(v: number | undefined) { this._v = v; } }
const c = new Cache();
c.value ??= 42;
console.log(c.value);
