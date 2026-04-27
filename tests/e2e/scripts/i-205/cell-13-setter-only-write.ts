class Box { _v = 0; set x(v: number) { this._v = v * 2; } get _peek(): number { return this._v; } }
const b = new Box();
b.x = 5;
console.log(b._peek);
