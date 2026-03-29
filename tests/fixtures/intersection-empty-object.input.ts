// Empty object removal: { x: number } & {} → struct
type WithEmpty = { x: number } & {};

// Identity mapped type: { [K in keyof T]: T[K] } & {} → type alias T
type Simplify<T> = { [K in keyof T]: T[K] } & {};

// Non-identity mapped type (value type is not T[K]) — becomes type alias
type NonIdentity<T> = { [K in keyof T]: string } & {};

// All empty: {} & {} → empty struct
type AllEmpty = {} & {};

// Parenthesized unwrap: (({ x: number })) & {} → struct
type Parenthesized = ({ x: number }) & {};

// Existing behavior preserved: two object literals
type Preserved = { a: string } & { b: number };
