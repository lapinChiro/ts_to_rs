class Foo { _b = true; get b(): boolean { return this._b; } set b(v: boolean) { this._b = v; } }
const f = new Foo();
f.b &&= false;
console.log(f.b);
