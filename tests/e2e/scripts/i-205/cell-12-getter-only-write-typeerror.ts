class Foo { get x(): number { return 42; } }
const f = new Foo();
f.x = 100;
console.log(f.x);
