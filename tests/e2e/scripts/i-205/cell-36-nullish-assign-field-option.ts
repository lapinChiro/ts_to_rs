class Cache { v: number | undefined = undefined; }
const c = new Cache();
c.v ??= 42;
console.log(c.v);
