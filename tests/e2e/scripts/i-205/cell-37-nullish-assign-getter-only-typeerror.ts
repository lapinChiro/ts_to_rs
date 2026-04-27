class Foo { get v(): number | undefined { return undefined; } }
const f = new Foo();
(f as any).v ??= 42;
console.log(f.v);
