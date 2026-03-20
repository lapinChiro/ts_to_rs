// Union type fallback: function types, tuple types, and other unsupported members

// Case 1: Union with function type member
type StringOrFn = string | ((x: number) => string);

// Case 2: Union with tuple type member
type StringOrTuple = string | [number, string];

// Case 3: Multiple unsupported types should produce distinct variants
type Mixed = string | ((x: number) => void) | [boolean, number];

// Case 4: Supported types only (existing behavior, should not change)
type StringOrNumber = string | number;
