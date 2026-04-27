class Box { _v = 0; set x(v: number) { this._v = v; } }
const b = new Box();
(b as any).x += 5;
console.log((b as any).x);
