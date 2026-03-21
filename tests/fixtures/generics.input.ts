export interface Container<T> {
  value: T;
}

export type Pair<A, B> = {
  first: A;
  second: B;
};

export function identity<T>(x: T): T {
  return x;
}

export interface UserList {
  users: Container<string>;
}

export interface Processor<T> {
  process(input: T): T;
}

export interface Bounded<T extends Processor<string>> {
  wrap(item: T): T;
}
