export class Box<T> {
  value: T;
  constructor(val: T) {
    this.value = val;
  }
}

export class Pair<A, B> {
  first: A;
  second: B;
  constructor(a: A, b: B) {
    this.first = a;
    this.second = b;
  }
}
