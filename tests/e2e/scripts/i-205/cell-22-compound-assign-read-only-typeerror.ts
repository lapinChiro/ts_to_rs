class Foo { get x(): number { return 10; } }
const f = new Foo();
f.x += 5;
console.log(f.x);
