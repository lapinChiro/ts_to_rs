class Profile { _name: string = "alice"; get name(): string { return this._name; } }
const p = new Profile();
console.log(p.name);
