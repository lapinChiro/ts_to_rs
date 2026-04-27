class Foo { _b = false; get b(): boolean { return this._b; } set b(v: boolean) { this._b = v; } }
const f = new Foo();
f.b ||= true;
console.log(f.b);
