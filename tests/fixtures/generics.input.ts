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
