class Foo { get x(): number { return 42; } }
const f = new Foo();
const {x} = f;
console.log(x);
