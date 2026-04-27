class Base {
  _n: number = 42;
  get x(): number { return this._n; }
}
class Sub extends Base {}
const s = new Sub();
console.log(s.x);
