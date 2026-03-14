// String literal union
type Direction = "up" | "down" | "left" | "right";

// Numeric literal union
type StatusCode = 200 | 404 | 500;

// Primitive type union
type Value = string | number | boolean;

// Two-type primitive union
type StringOrNumber = string | number;

// Dummy types for type reference union tests
interface Success {}
interface Failure {}
interface MyType {}
interface Response {}
interface Promise<T> {}

// Type reference union
type Result = Success | Failure;

// Mixed keyword and type reference union
type Mixed = string | MyType;

// Generic type reference union
type AsyncResult = Response | Promise<Response>;
