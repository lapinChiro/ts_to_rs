// Generic class patterns

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

// Generic class with method (immutable)
export class Stack<T> {
  items: T[];
  constructor() {
    this.items = [];
  }

  isEmpty(): boolean {
    return this.items.length === 0;
  }
}

// Generic class with type constraint
export class NumberBox<T extends number> {
  value: T;
  constructor(val: T) {
    this.value = val;
  }

  double(): number {
    return this.value * 2;
  }
}

// Multiple type parameters
export class KeyValue<K, V> {
  key: K;
  value: V;
  constructor(key: K, value: V) {
    this.key = key;
    this.value = value;
  }
}
