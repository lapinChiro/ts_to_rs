class Person { _name = "alice"; get name(): string { return this._name; } }
const p = new Person();
console.log(p.name);
