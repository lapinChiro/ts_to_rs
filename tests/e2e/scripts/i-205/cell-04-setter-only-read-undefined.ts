class Box { _v = 0; set x(v: number) { this._v = v; } }
const b = new Box();
b.x = 100;
console.log(b.x);
