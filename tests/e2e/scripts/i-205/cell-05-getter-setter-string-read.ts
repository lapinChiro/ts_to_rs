class Foo { _name = "alice"; get name(): string { return this._name; } set name(v: string) { this._name = v; } }
const f = new Foo();
console.log(f.name);
